use std::{
    collections::HashMap,
    net::IpAddr,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, anyhow, bail};
use axum::{
    Router,
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::{Request, Response, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use axum_server::tls_rustls::RustlsConfig;
use chrono::{DateTime, TimeDelta, Utc};
use http_body_util::{BodyExt, Full};
use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, BodyWrapper, BytesResponse, ChallengeType,
    Identifier, NewAccount, NewOrder, OrderStatus, RetryPolicy,
};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, sync::watch, time::sleep};
use uuid::Uuid;

use crate::config::AcmeHttp01Config;

const SHORTLIVED_PROFILE: &str = "shortlived";
const ACCOUNT_FILE: &str = "account.json";
const ACCOUNT_METADATA_FILE: &str = "account-metadata.json";
const CERT_FILE: &str = "fullchain.pem";
const KEY_FILE: &str = "privkey.pem";
const METADATA_FILE: &str = "metadata.json";

#[derive(Clone, Default)]
pub struct ChallengeStore {
    entries: Arc<RwLock<HashMap<String, String>>>,
}

impl ChallengeStore {
    async fn insert(&self, token: String, key_authorization: String) {
        self.entries.write().await.insert(token, key_authorization);
    }

    async fn remove(&self, token: &str) {
        self.entries.write().await.remove(token);
    }

    pub async fn get(&self, token: &str) -> Option<String> {
        self.entries.read().await.get(token).cloned()
    }
}

#[derive(Clone)]
struct AcmeHttpState {
    challenges: ChallengeStore,
}

pub fn http01_router(challenges: ChallengeStore) -> Router {
    Router::new()
        .route(
            "/.well-known/acme-challenge/{token}",
            get(serve_http01_challenge),
        )
        .fallback(get(handle_http01_fallback))
        .with_state(AcmeHttpState { challenges })
}

async fn serve_http01_challenge(
    State(state): State<AcmeHttpState>,
    AxumPath(token): AxumPath<String>,
) -> impl IntoResponse {
    match state.challenges.get(&token).await {
        Some(value) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            value,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_http01_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AcmeIdentifier {
    Dns(String),
    Ip(IpAddr),
}

impl AcmeIdentifier {
    fn as_order_identifier(&self) -> Identifier {
        match self {
            Self::Dns(name) => Identifier::Dns(name.clone()),
            Self::Ip(addr) => Identifier::Ip(*addr),
        }
    }

    fn profile(&self) -> Option<&'static str> {
        match self {
            Self::Dns(_) => None,
            Self::Ip(_) => Some(SHORTLIVED_PROFILE),
        }
    }
}

pub(crate) fn parse_acme_identifier(value: &str) -> anyhow::Result<AcmeIdentifier> {
    let value = value.trim();
    if value.is_empty() {
        bail!("ACME identifier must not be empty");
    }
    if value.contains("://") || value.contains('/') {
        bail!("ACME identifier must be a domain name or IP address, not a URL");
    }
    Ok(value
        .parse::<IpAddr>()
        .map(AcmeIdentifier::Ip)
        .unwrap_or_else(|_| AcmeIdentifier::Dns(value.to_string())))
}

#[derive(Debug, Clone)]
pub struct ManagedCertificate {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub info: CertificateInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    pub identifier: String,
    pub directory_url: String,
    pub profile: Option<String>,
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountMetadata {
    directory_url: String,
    email: String,
    updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AcmeManager {
    config: Arc<AcmeHttp01Config>,
    challenges: ChallengeStore,
    http: aioduct::TokioClient,
}

impl AcmeManager {
    pub fn new(
        config: Arc<AcmeHttp01Config>,
        challenges: ChallengeStore,
        http: aioduct::TokioClient,
    ) -> Self {
        Self {
            config,
            challenges,
            http,
        }
    }

    pub async fn prepare_certificate(&self) -> anyhow::Result<ManagedCertificate> {
        self.ensure_cert_dir().await?;
        if let Some(certificate) = self.load_certificate().await? {
            if certificate.info.not_after > Utc::now() {
                tracing::info!(
                    identifier = certificate.info.identifier,
                    not_after = %certificate.info.not_after,
                    "using existing ACME certificate"
                );
                return Ok(certificate);
            }
        }

        self.issue_certificate().await
    }

    pub fn spawn_renewal(
        self,
        tls_config: RustlsConfig,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let delay = match self.load_certificate().await {
                    Ok(Some(certificate)) => renewal_delay(&certificate.info, Utc::now()),
                    Ok(None) => Duration::from_secs(0),
                    Err(error) => {
                        tracing::warn!(%error, "failed to inspect ACME certificate for renewal");
                        Duration::from_secs(60)
                    }
                };

                tokio::select! {
                    _ = sleep(delay) => {}
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                        continue;
                    }
                }

                match self.issue_certificate().await {
                    Ok(certificate) => {
                        if let Err(error) = tls_config
                            .reload_from_pem_file(&certificate.cert_path, &certificate.key_path)
                            .await
                        {
                            tracing::error!(%error, "renewed ACME certificate could not be loaded");
                        } else {
                            tracing::info!(
                                identifier = certificate.info.identifier,
                                not_after = %certificate.info.not_after,
                                "reloaded renewed ACME certificate"
                            );
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, "ACME certificate renewal failed");
                        tokio::select! {
                            _ = sleep(Duration::from_secs(300)) => {}
                            changed = shutdown_rx.changed() => {
                                if changed.is_err() || *shutdown_rx.borrow() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    async fn issue_certificate(&self) -> anyhow::Result<ManagedCertificate> {
        self.ensure_cert_dir().await?;
        let raw_identifier = self.config.identifier.as_str();
        let identifier = parse_acme_identifier(raw_identifier)?;
        let profile = identifier.profile();
        let account = self.account().await?;
        let order_identifier = identifier.as_order_identifier();
        let identifiers = [order_identifier];
        let mut new_order = NewOrder::new(&identifiers);
        if let Some(profile) = profile {
            new_order = new_order.profile(profile);
        }
        let mut order = account
            .new_order(&new_order)
            .await
            .context("create ACME order")?;
        let mut challenge_tokens = Vec::new();
        let result = async {
            self.complete_http01_authorizations(&mut order, &mut challenge_tokens)
                .await?;
            let retry = RetryPolicy::new().timeout(Duration::from_secs(120));
            let status = order
                .poll_ready(&retry)
                .await
                .context("poll ACME order readiness")?;
            if status != OrderStatus::Ready {
                bail!("ACME order did not become ready: {status:?}");
            }
            let private_key_pem = order.finalize().await.context("finalize ACME order")?;
            let cert_chain_pem = order
                .poll_certificate(&retry)
                .await
                .context("poll ACME certificate")?;
            anyhow::Ok((cert_chain_pem, private_key_pem))
        }
        .await;

        for token in &challenge_tokens {
            self.challenges.remove(token).await;
        }

        let (cert_chain_pem, private_key_pem) = result?;
        let info = parse_certificate_info(
            cert_chain_pem.as_bytes(),
            raw_identifier.to_string(),
            self.config.directory_url.clone(),
            profile.map(ToOwned::to_owned),
        )?;
        self.write_certificate(&cert_chain_pem, &private_key_pem, &info)
            .await?;
        tracing::info!(
            identifier = info.identifier,
            profile = info.profile.as_deref().unwrap_or("default"),
            not_after = %info.not_after,
            "issued ACME certificate"
        );
        self.load_certificate()
            .await?
            .context("issued ACME certificate was not readable")
    }

    async fn complete_http01_authorizations(
        &self,
        order: &mut instant_acme::Order,
        challenge_tokens: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        let mut authorizations = order.authorizations();
        while let Some(result) = authorizations.next().await {
            let mut authorization = result.context("load ACME authorization")?;
            match authorization.status {
                AuthorizationStatus::Valid => continue,
                AuthorizationStatus::Pending => {}
                other => bail!("ACME authorization is not pending: {other:?}"),
            }

            let mut challenge = authorization
                .challenge(ChallengeType::Http01)
                .ok_or_else(|| anyhow!("ACME authorization did not include http-01 challenge"))?;
            let token = challenge.token.clone();
            let key_authorization = challenge.key_authorization().as_str().to_string();
            self.challenges
                .insert(token.clone(), key_authorization)
                .await;
            challenge_tokens.push(token);
            challenge
                .set_ready()
                .await
                .context("mark ACME http-01 challenge ready")?;
        }
        Ok(())
    }

    async fn account(&self) -> anyhow::Result<Account> {
        let account_path = self.account_path();
        if account_path.exists() && self.account_metadata_matches().await? {
            let data = tokio::fs::read(&account_path)
                .await
                .with_context(|| format!("read {}", account_path.display()))?;
            let credentials: AccountCredentials =
                serde_json::from_slice(&data).context("parse ACME account credentials")?;
            return instant_acme::Account::builder_with_http(Box::new(AioductAcmeHttpClient {
                client: self.http.clone(),
            }))
            .from_credentials(credentials)
            .await
            .context("restore ACME account");
        }

        let contact = format!("mailto:{}", self.config.email.as_str());
        let contacts = [contact.as_str()];
        let (account, credentials) =
            instant_acme::Account::builder_with_http(Box::new(AioductAcmeHttpClient {
                client: self.http.clone(),
            }))
            .create(
                &NewAccount {
                    contact: &contacts,
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                self.config.directory_url.clone(),
                None,
            )
            .await
            .context("create ACME account")?;
        let credentials_json =
            serde_json::to_vec_pretty(&credentials).context("serialize ACME account")?;
        write_atomic(&account_path, &credentials_json, 0o600).await?;
        let account_metadata = AccountMetadata {
            directory_url: self.config.directory_url.clone(),
            email: self.config.email.clone(),
            updated_at: Utc::now(),
        };
        let metadata_json = serde_json::to_vec_pretty(&account_metadata)
            .context("serialize ACME account metadata")?;
        write_atomic(&self.account_metadata_path(), &metadata_json, 0o600).await?;
        Ok(account)
    }

    async fn load_certificate(&self) -> anyhow::Result<Option<ManagedCertificate>> {
        let cert_path = self.cert_path();
        let key_path = self.key_path();
        if !cert_path.exists() || !key_path.exists() {
            return Ok(None);
        }
        let expected_identifier = self.config.identifier.as_str();
        let expected_profile = parse_acme_identifier(expected_identifier)?
            .profile()
            .map(ToOwned::to_owned);
        let Some(metadata) = self.load_metadata().await? else {
            tracing::warn!("existing ACME certificate is missing metadata; reissuing");
            return Ok(None);
        };
        if metadata.identifier != expected_identifier
            || metadata.directory_url != self.config.directory_url
            || metadata.profile != expected_profile
        {
            tracing::warn!(
                "existing ACME certificate metadata does not match current config; reissuing"
            );
            return Ok(None);
        }
        let cert_pem = tokio::fs::read(&cert_path)
            .await
            .with_context(|| format!("read {}", cert_path.display()))?;
        let info = parse_certificate_info(
            &cert_pem,
            expected_identifier.to_string(),
            self.config.directory_url.clone(),
            expected_profile,
        )?;
        Ok(Some(ManagedCertificate {
            cert_path,
            key_path,
            info,
        }))
    }

    async fn write_certificate(
        &self,
        cert_chain_pem: &str,
        private_key_pem: &str,
        info: &CertificateInfo,
    ) -> anyhow::Result<()> {
        write_atomic(&self.cert_path(), cert_chain_pem.as_bytes(), 0o600).await?;
        write_atomic(&self.key_path(), private_key_pem.as_bytes(), 0o600).await?;
        let metadata = serde_json::to_vec_pretty(info).context("serialize ACME metadata")?;
        write_atomic(&self.metadata_path(), &metadata, 0o600).await?;
        Ok(())
    }

    async fn ensure_cert_dir(&self) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&self.config.cert_dir)
            .await
            .with_context(|| format!("create {}", self.config.cert_dir.display()))?;
        set_permissions(&self.config.cert_dir, 0o700).await?;
        Ok(())
    }

    fn account_path(&self) -> PathBuf {
        self.config.cert_dir.join(ACCOUNT_FILE)
    }

    fn account_metadata_path(&self) -> PathBuf {
        self.config.cert_dir.join(ACCOUNT_METADATA_FILE)
    }

    pub fn cert_path(&self) -> PathBuf {
        self.config.cert_dir.join(CERT_FILE)
    }

    pub fn key_path(&self) -> PathBuf {
        self.config.cert_dir.join(KEY_FILE)
    }

    fn metadata_path(&self) -> PathBuf {
        self.config.cert_dir.join(METADATA_FILE)
    }

    async fn load_metadata(&self) -> anyhow::Result<Option<CertificateInfo>> {
        let metadata_path = self.metadata_path();
        if !metadata_path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&metadata_path)
            .await
            .with_context(|| format!("read {}", metadata_path.display()))?;
        let metadata = serde_json::from_slice(&data).context("parse ACME certificate metadata")?;
        Ok(Some(metadata))
    }

    async fn account_metadata_matches(&self) -> anyhow::Result<bool> {
        let metadata_path = self.account_metadata_path();
        if !metadata_path.exists() {
            return Ok(false);
        }
        let data = tokio::fs::read(&metadata_path)
            .await
            .with_context(|| format!("read {}", metadata_path.display()))?;
        let metadata: AccountMetadata =
            serde_json::from_slice(&data).context("parse ACME account metadata")?;
        Ok(metadata.directory_url == self.config.directory_url
            && metadata.email == self.config.email)
    }
}

#[derive(Clone)]
struct AioductAcmeHttpClient {
    client: aioduct::TokioClient,
}

impl instant_acme::HttpClient for AioductAcmeHttpClient {
    fn request(
        &self,
        req: Request<BodyWrapper<Bytes>>,
    ) -> Pin<Box<dyn Future<Output = Result<BytesResponse, instant_acme::Error>> + Send>> {
        let client = self.client.clone();
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let uri = parts.uri.to_string();
            let body = body
                .collect()
                .await
                .map_err(|error| instant_acme::Error::Other(Box::new(error)))?
                .to_bytes();
            let mut request = client
                .request(parts.method, &uri)
                .map_err(|error| instant_acme::Error::Other(Box::new(error)))?
                .body(body);
            for (name, value) in parts.headers {
                if let Some(name) = name {
                    request = request.header(name, value);
                }
            }
            let response = request
                .send()
                .await
                .map_err(|error| instant_acme::Error::Other(Box::new(error)))?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .bytes()
                .await
                .map_err(|error| instant_acme::Error::Other(Box::new(error)))?;
            let mut response = Response::new(Full::new(body));
            *response.status_mut() = status;
            *response.headers_mut() = headers;
            Ok(BytesResponse::from(response))
        })
    }
}

pub fn parse_certificate_info(
    cert_pem: &[u8],
    identifier: String,
    directory_url: String,
    profile: Option<String>,
) -> anyhow::Result<CertificateInfo> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem)
        .map_err(|error| anyhow!("parse certificate PEM: {error}"))?;
    let (_, certificate) = x509_parser::parse_x509_certificate(&pem.contents)
        .map_err(|error| anyhow!("parse X.509 certificate: {error}"))?;
    let validity = certificate.validity();
    let not_before = DateTime::from_timestamp(validity.not_before.timestamp(), 0)
        .context("certificate not_before is out of range")?;
    let not_after = DateTime::from_timestamp(validity.not_after.timestamp(), 0)
        .context("certificate not_after is out of range")?;
    Ok(CertificateInfo {
        identifier,
        directory_url,
        profile,
        not_before,
        not_after,
        updated_at: Utc::now(),
    })
}

pub(crate) fn renewal_delay(info: &CertificateInfo, now: DateTime<Utc>) -> Duration {
    let renew_at = if info.profile.as_deref() == Some(SHORTLIVED_PROFILE) {
        let lifetime = (info.not_after - info.not_before).num_seconds().max(0);
        info.not_before + TimeDelta::seconds(lifetime / 2)
    } else {
        info.not_after - TimeDelta::days(30)
    };
    if renew_at <= now {
        Duration::from_secs(0)
    } else {
        (renew_at - now).to_std().unwrap_or_default()
    }
}

async fn write_atomic(path: &Path, data: &[u8], mode: u32) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("create {}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let temp_path = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    tokio::fs::write(&temp_path, data)
        .await
        .with_context(|| format!("write {}", temp_path.display()))?;
    set_permissions(&temp_path, mode).await?;
    tokio::fs::rename(&temp_path, path)
        .await
        .with_context(|| format!("rename {} to {}", temp_path.display(), path.display()))?;
    Ok(())
}

async fn set_permissions(path: &Path, mode: u32) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = std::fs::Permissions::from_mode(mode);
        tokio::fs::set_permissions(path, permissions)
            .await
            .with_context(|| format!("set permissions on {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager(cert_dir: PathBuf) -> AcmeManager {
        let config = Arc::new(AcmeHttp01Config {
            identifier: "token.example.com".to_string(),
            email: "ops@example.com".to_string(),
            http_bind_addr: "127.0.0.1:80".parse().unwrap(),
            cert_dir,
            directory_url: "https://example.test/acme".to_string(),
        });
        let http = aioduct::TokioClient::builder()
            .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
            .build()
            .unwrap();
        AcmeManager::new(config, ChallengeStore::default(), http)
    }

    fn unique_cert_dir() -> PathBuf {
        std::env::temp_dir().join(format!("token-toxication-acme-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn parses_ip_identifier_for_shortlived_profile() {
        let identifier = parse_acme_identifier("127.0.0.1").unwrap();
        assert_eq!(identifier, AcmeIdentifier::Ip("127.0.0.1".parse().unwrap()));
        assert_eq!(identifier.profile(), Some("shortlived"));
    }

    #[test]
    fn parses_dns_identifier_without_profile() {
        let identifier = parse_acme_identifier("token.example.com").unwrap();
        assert_eq!(
            identifier,
            AcmeIdentifier::Dns("token.example.com".to_string())
        );
        assert_eq!(identifier.profile(), None);
    }

    #[test]
    fn rejects_url_identifier() {
        let error = parse_acme_identifier("https://token.example.com")
            .unwrap_err()
            .to_string();
        assert!(error.contains("not a URL"));
    }

    #[tokio::test]
    async fn http01_fallback_returns_not_found() {
        assert_eq!(handle_http01_fallback().await, StatusCode::NOT_FOUND);
    }

    #[test]
    fn renewal_for_shortlived_uses_half_lifetime() {
        let now = Utc::now();
        let info = CertificateInfo {
            identifier: "127.0.0.1".to_string(),
            directory_url: "https://example.test/acme".to_string(),
            profile: Some("shortlived".to_string()),
            not_before: now,
            not_after: now + TimeDelta::hours(160),
            updated_at: now,
        };
        assert_eq!(renewal_delay(&info, now).as_secs(), 80 * 60 * 60);
    }

    #[tokio::test]
    async fn challenge_store_returns_inserted_key_authorization() {
        let store = ChallengeStore::default();
        store
            .insert("token".to_string(), "token.thumbprint".to_string())
            .await;
        assert_eq!(
            store.get("token").await.as_deref(),
            Some("token.thumbprint")
        );
        store.remove("token").await;
        assert!(store.get("token").await.is_none());
    }

    #[tokio::test]
    async fn certificate_metadata_mismatch_triggers_reissue() {
        let cert_dir = unique_cert_dir();
        let manager = test_manager(cert_dir.clone());
        manager.ensure_cert_dir().await.unwrap();

        let key = rcgen::KeyPair::generate().unwrap();
        let params = rcgen::CertificateParams::new(vec!["token.example.com".to_string()]).unwrap();
        let cert = params.self_signed(&key).unwrap();
        write_atomic(&manager.cert_path(), cert.pem().as_bytes(), 0o600)
            .await
            .unwrap();
        write_atomic(&manager.key_path(), key.serialize_pem().as_bytes(), 0o600)
            .await
            .unwrap();

        let now = Utc::now();
        let metadata = CertificateInfo {
            identifier: "other.example.com".to_string(),
            directory_url: "https://example.test/acme".to_string(),
            profile: None,
            not_before: now,
            not_after: now + TimeDelta::days(1),
            updated_at: now,
        };
        let metadata_json = serde_json::to_vec_pretty(&metadata).unwrap();
        write_atomic(&manager.metadata_path(), &metadata_json, 0o600)
            .await
            .unwrap();

        assert!(manager.load_certificate().await.unwrap().is_none());
        let _ = tokio::fs::remove_dir_all(cert_dir).await;
    }

    #[test]
    fn parses_certificate_validity_from_pem() {
        let key = rcgen::KeyPair::generate().unwrap();
        let params = rcgen::CertificateParams::new(vec!["token.example.com".to_string()]).unwrap();
        let cert = params.self_signed(&key).unwrap();
        let info = parse_certificate_info(
            cert.pem().as_bytes(),
            "token.example.com".to_string(),
            "https://example.test/acme".to_string(),
            None,
        )
        .unwrap();
        assert_eq!(info.identifier, "token.example.com");
        assert!(info.not_after > info.not_before);
    }
}
