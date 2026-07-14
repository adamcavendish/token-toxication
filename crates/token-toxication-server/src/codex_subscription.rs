use std::time::Duration;

use aioduct::TokioClient;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::{
    db::Db,
    error::AppError,
    models::{
        CodexAccountCredits, CodexAccountQuotaLimit, CodexAccountQuotaResponse,
        CodexAccountQuotaWindow, CodexAccountSpendControl, CodexAccountSpendControlLimit,
        ProviderAccountRecord,
    },
};

const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const DEFAULT_CODEX_ISSUER: &str = "https://auth.openai.com";
const CODEX_QUOTA_TIMEOUT: Duration = Duration::from_secs(15);
const REFRESH_SAFETY_MARGIN_MS: i64 = 30_000;
const STORED_CODEX_CREDENTIAL_TYPE: &str = "token-toxication-codex-oauth-v1";
const RAW_REFRESH_TOKEN_HELP: &str = "Codex subscription expects a raw refresh token. \
Use `jq -r '.tokens.refresh_token' ~/.codex/auth.json` for Codex CLI or \
`jq -r '.openai.refresh' ~/.local/share/opencode/auth.json` for opencode.";

#[derive(Debug, Clone)]
pub struct CodexSubscriptionAuthorization {
    pub access_token: String,
    pub account_id: Option<String>,
    pub endpoint: String,
}

#[derive(Debug, Clone)]
struct CodexSubscriptionCredential {
    refresh: String,
    access: Option<String>,
    expires: Option<i64>,
    account_id: Option<String>,
    issuer: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredCodexSubscriptionCredential {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    refresh: String,
    #[serde(default)]
    access: Option<String>,
    #[serde(default)]
    expires: Option<i64>,
    #[serde(default, rename = "accountId")]
    account_id: Option<String>,
    #[serde(default)]
    issuer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: String,
    access_token: String,
    refresh_token: String,
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexUsagePayload {
    plan_type: Option<String>,
    rate_limit: Option<CodexUsageRateLimit>,
    credits: Option<CodexUsageCredits>,
    spend_control: Option<CodexUsageSpendControl>,
    additional_rate_limits: Option<Vec<CodexUsageAdditionalRateLimit>>,
    rate_limit_reached_type: Option<Value>,
    rate_limit_reset_credits: Option<CodexUsageResetCredits>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageRateLimit {
    allowed: Option<bool>,
    limit_reached: Option<bool>,
    primary_window: Option<CodexUsageWindow>,
    secondary_window: Option<CodexUsageWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<i64>,
    reset_after_seconds: Option<i64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageAdditionalRateLimit {
    limit_name: Option<String>,
    metered_feature: Option<String>,
    rate_limit: Option<CodexUsageRateLimit>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageCredits {
    has_credits: Option<bool>,
    unlimited: Option<bool>,
    balance: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageSpendControl {
    reached: Option<bool>,
    individual_limit: Option<CodexUsageSpendControlLimit>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageSpendControlLimit {
    source: Option<String>,
    limit: Option<Value>,
    used: Option<Value>,
    remaining: Option<Value>,
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    reset_after_seconds: Option<i64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageResetCredits {
    available_count: Option<i64>,
}

pub fn is_codex_subscription_auth(auth_mode: &str) -> bool {
    auth_mode == "codex-oauth"
}

pub fn codex_subscription_endpoint(base_url: &str) -> Result<String, AppError> {
    let mut url = parse_codex_base_url(base_url)?;
    let path = url.path().trim_end_matches('/').to_string();
    if path.ends_with("/responses") {
        url.set_path(&path);
        return Ok(url.to_string());
    }

    Ok(append_codex_path(
        codex_account_api_base_url(url),
        "codex/responses",
    ))
}

pub fn codex_quota_endpoint(base_url: &str) -> Result<String, AppError> {
    Ok(append_codex_path(
        codex_account_api_base_url(parse_codex_base_url(base_url)?),
        "wham/usage",
    ))
}

pub fn canonicalize_legacy_codex_base_url(base_url: &str) -> Option<String> {
    let url = parse_codex_base_url(base_url).ok()?;
    let path = url.path().trim_end_matches('/');
    if !["/codex/responses", "/codex"]
        .into_iter()
        .any(|suffix| path.ends_with(suffix))
    {
        return None;
    }

    Some(
        codex_account_api_base_url(url)
            .to_string()
            .trim_end_matches('/')
            .to_string(),
    )
}

fn parse_codex_base_url(base_url: &str) -> Result<Url, AppError> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(AppError::BadRequest(
            "Codex account API base URL is required".to_string(),
        ));
    }
    let mut url = Url::parse(base_url).map_err(|error| {
        AppError::BadRequest(format!("invalid Codex account API base URL: {error}"))
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.cannot_be_a_base() {
        return Err(AppError::BadRequest(
            "Codex account API base URL must use http or https".to_string(),
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn codex_account_api_base_url(mut url: Url) -> Url {
    let path = url.path().trim_end_matches('/');
    let path = ["/codex/responses", "/codex", "/responses"]
        .into_iter()
        .find_map(|suffix| path.strip_suffix(suffix))
        .unwrap_or(path)
        .to_string();
    url.set_path(if path.is_empty() { "/" } else { &path });
    url
}

fn append_codex_path(mut base_url: Url, path: &str) -> String {
    let prefix = base_url.path().trim_end_matches('/');
    base_url.set_path(&format!("{prefix}/{}", path.trim_start_matches('/')));
    base_url.to_string()
}

pub async fn codex_subscription_authorization(
    db: &Db,
    http: &TokioClient,
    account: &ProviderAccountRecord,
) -> Result<CodexSubscriptionAuthorization, AppError> {
    let endpoint = codex_subscription_endpoint(&account.account.base_url)?;
    let mut credential = parse_codex_subscription_credential(&account.api_key)?;
    if credential
        .access
        .as_deref()
        .is_some_and(|access| !access.trim().is_empty())
        && !credential_is_expired(credential.expires)
    {
        return Ok(CodexSubscriptionAuthorization {
            access_token: credential.access.unwrap_or_default(),
            account_id: credential.account_id,
            endpoint,
        });
    }

    let tokens = refresh_codex_subscription_token(http, &credential).await?;
    credential.access = Some(tokens.access_token.clone());
    credential.refresh = tokens.refresh_token.clone();
    credential.expires =
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000);
    credential.account_id = extract_account_id(&tokens).or(credential.account_id);
    db.update_provider_account_secret(&account.account.id, &serialize_credential(&credential))
        .await?;

    Ok(CodexSubscriptionAuthorization {
        access_token: tokens.access_token,
        account_id: credential.account_id,
        endpoint,
    })
}

pub async fn codex_account_quota(
    db: &Db,
    http: &TokioClient,
    account_id: &str,
) -> Result<CodexAccountQuotaResponse, AppError> {
    let account = codex_account_record(db, account_id).await?;
    let authorization = codex_subscription_authorization(db, http, &account).await?;
    let endpoint = codex_quota_endpoint(&account.account.base_url)?;
    let payload = fetch_codex_usage(
        http,
        &endpoint,
        &authorization.access_token,
        authorization.account_id.as_deref(),
        CODEX_QUOTA_TIMEOUT,
    )
    .await?;

    Ok(codex_quota_response(
        account.account.id,
        account.account.auth_mode,
        endpoint,
        payload,
        Utc::now(),
    ))
}

async fn codex_account_record(
    db: &Db,
    account_id: &str,
) -> Result<ProviderAccountRecord, AppError> {
    let account = db
        .get_provider_account_record(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound("provider account not found".to_string()))?;
    if !is_codex_subscription_auth(&account.account.auth_mode) {
        return Err(AppError::BadRequest(
            "provider account does not use Codex OAuth".to_string(),
        ));
    }
    Ok(account)
}

async fn fetch_codex_usage(
    http: &TokioClient,
    endpoint: &str,
    access_token: &str,
    account_id: Option<&str>,
    timeout: Duration,
) -> Result<CodexUsagePayload, AppError> {
    let mut request = http
        .get(endpoint)?
        .bearer_auth(access_token)
        .header(header::ACCEPT, HeaderValue::from_static("application/json"))
        .header(header::USER_AGENT, HeaderValue::from_static("codex-cli"))
        .timeout(timeout);
    if let Some(account_id) = account_id.filter(|value| !value.trim().is_empty()) {
        let account_id = HeaderValue::from_str(account_id).map_err(|error| {
            AppError::Internal(format!("invalid Codex account id header: {error}"))
        })?;
        request = request.header(HeaderName::from_static("chatgpt-account-id"), account_id);
    }

    let response = request
        .send()
        .await
        .map_err(|error| AppError::Upstream(error.into()))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        return Err(codex_quota_upstream_error(status, &bytes));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::Internal(format!("invalid Codex quota response: {error}")))
}

fn codex_quota_response(
    account_id: String,
    auth_mode: String,
    endpoint: String,
    payload: CodexUsagePayload,
    now: DateTime<Utc>,
) -> CodexAccountQuotaResponse {
    let mut limits = Vec::new();
    if let Some(rate_limit) = payload.rate_limit {
        limits.push(codex_quota_limit(
            "codex".to_string(),
            "Codex".to_string(),
            rate_limit,
            now,
        ));
    }
    for (index, additional) in payload
        .additional_rate_limits
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        let limit_id = non_empty(additional.metered_feature)
            .or_else(|| non_empty(additional.limit_name.clone()))
            .unwrap_or_else(|| format!("additional-{}", index + 1));
        let display_name = non_empty(additional.limit_name).unwrap_or_else(|| limit_id.clone());
        let rate_limit = additional.rate_limit.unwrap_or(CodexUsageRateLimit {
            allowed: None,
            limit_reached: None,
            primary_window: None,
            secondary_window: None,
        });
        limits.push(codex_quota_limit(limit_id, display_name, rate_limit, now));
    }

    CodexAccountQuotaResponse {
        account_id,
        auth_mode,
        endpoint,
        plan_type: non_empty(payload.plan_type),
        limits,
        credits: payload.credits.map(|credits| CodexAccountCredits {
            has_credits: credits.has_credits,
            unlimited: credits.unlimited,
            balance: credits.balance.and_then(value_string),
        }),
        spend_control: payload.spend_control.map(|spend| CodexAccountSpendControl {
            reached: spend.reached,
            individual_limit: spend
                .individual_limit
                .map(|limit| CodexAccountSpendControlLimit {
                    source: non_empty(limit.source),
                    limit: limit.limit.and_then(value_string),
                    used: limit.used.and_then(value_string),
                    remaining: limit.remaining.and_then(value_string),
                    used_percent: normalized_percent(limit.used_percent),
                    remaining_percent: normalized_percent(limit.remaining_percent),
                    reset_after_seconds: non_negative(limit.reset_after_seconds),
                    reset_at: reset_time(limit.reset_at, limit.reset_after_seconds, now),
                }),
        }),
        rate_limit_reached_type: reached_type(payload.rate_limit_reached_type),
        reset_credits_available_count: payload
            .rate_limit_reset_credits
            .and_then(|credits| credits.available_count),
    }
}

fn codex_quota_limit(
    limit_id: String,
    display_name: String,
    rate_limit: CodexUsageRateLimit,
    now: DateTime<Utc>,
) -> CodexAccountQuotaLimit {
    CodexAccountQuotaLimit {
        limit_id,
        display_name,
        allowed: rate_limit.allowed,
        limit_reached: rate_limit.limit_reached,
        primary_window: rate_limit
            .primary_window
            .map(|window| codex_quota_window(window, now)),
        secondary_window: rate_limit
            .secondary_window
            .map(|window| codex_quota_window(window, now)),
    }
}

fn codex_quota_window(window: CodexUsageWindow, now: DateTime<Utc>) -> CodexAccountQuotaWindow {
    CodexAccountQuotaWindow {
        used_percent: normalized_percent(window.used_percent),
        limit_window_seconds: positive(window.limit_window_seconds),
        reset_after_seconds: non_negative(window.reset_after_seconds),
        reset_at: reset_time(window.reset_at, window.reset_after_seconds, now),
    }
}

fn codex_quota_upstream_error(status: StatusCode, bytes: &[u8]) -> AppError {
    let message = serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .or_else(|| value.get("error_description"))
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).chars().take(500).collect());
    let message = format!(
        "Codex quota query failed: {} {}",
        status.as_u16(),
        message.trim()
    );
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        AppError::Unauthorized(message)
    } else {
        AppError::Internal(message)
    }
}

fn reset_time(
    reset_at: Option<i64>,
    reset_after_seconds: Option<i64>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    reset_at
        .filter(|value| *value > 0)
        .and_then(|value| DateTime::from_timestamp(value, 0))
        .or_else(|| {
            non_negative(reset_after_seconds)
                .and_then(|seconds| now.checked_add_signed(chrono::Duration::seconds(seconds)))
        })
}

fn reached_type(value: Option<Value>) -> Option<String> {
    value.and_then(|value| {
        value.as_str().map(ToOwned::to_owned).or_else(|| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
    })
}

fn value_string(value: Value) -> Option<String> {
    match value {
        Value::String(value) => non_empty(Some(value)),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalized_percent(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 100.0))
}

fn positive(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

fn non_negative(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value >= 0)
}

fn parse_codex_subscription_credential(
    value: &str,
) -> Result<CodexSubscriptionCredential, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(
            "Codex subscription refresh token is required".into(),
        ));
    }
    if !value.starts_with('{') {
        return Ok(CodexSubscriptionCredential {
            refresh: value.to_string(),
            access: None,
            expires: None,
            account_id: None,
            issuer: DEFAULT_CODEX_ISSUER.to_string(),
        });
    }

    let stored: StoredCodexSubscriptionCredential = serde_json::from_str(value)
        .map_err(|_| AppError::BadRequest(RAW_REFRESH_TOKEN_HELP.into()))?;
    if stored.r#type != STORED_CODEX_CREDENTIAL_TYPE {
        return Err(AppError::BadRequest(RAW_REFRESH_TOKEN_HELP.into()));
    }
    if stored.refresh.trim().is_empty() {
        return Err(AppError::BadRequest(
            "stored Codex subscription credential is missing a refresh token".into(),
        ));
    }
    Ok(CodexSubscriptionCredential {
        refresh: stored.refresh,
        access: stored.access,
        expires: normalize_expires(stored.expires),
        account_id: stored.account_id,
        issuer: stored
            .issuer
            .filter(|issuer| !issuer.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_CODEX_ISSUER.to_string())
            .trim_end_matches('/')
            .to_string(),
    })
}

async fn refresh_codex_subscription_token(
    http: &TokioClient,
    credential: &CodexSubscriptionCredential,
) -> Result<TokenResponse, AppError> {
    let token_url = format!("{}/oauth/token", credential.issuer.trim_end_matches('/'));
    let response = http
        .post(&token_url)?
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", credential.refresh.as_str()),
            ("client_id", CODEX_CLIENT_ID),
        ])
        .send()
        .await
        .map_err(|error| AppError::Upstream(error.into()))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes);
        let message = format!(
            "Codex token refresh failed: {} {}",
            status.as_u16(),
            body.trim()
        );
        return if matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            Err(AppError::Unauthorized(message))
        } else {
            Err(AppError::Internal(message))
        };
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::Internal(format!("invalid Codex token response: {error}")))
}

fn serialize_credential(credential: &CodexSubscriptionCredential) -> String {
    let stored = StoredCodexSubscriptionCredential {
        r#type: STORED_CODEX_CREDENTIAL_TYPE.to_string(),
        refresh: credential.refresh.clone(),
        access: credential.access.clone(),
        expires: credential.expires,
        account_id: credential.account_id.clone(),
        issuer: (credential.issuer != DEFAULT_CODEX_ISSUER).then(|| credential.issuer.clone()),
    };
    serde_json::to_string(&stored).unwrap_or_else(|_| credential.refresh.clone())
}

fn credential_is_expired(expires: Option<i64>) -> bool {
    let Some(expires) = expires else {
        return true;
    };
    Utc::now().timestamp_millis() + REFRESH_SAFETY_MARGIN_MS >= expires
}

fn normalize_expires(value: Option<i64>) -> Option<i64> {
    value.map(|expires| {
        if expires > 10_000_000_000 {
            expires
        } else {
            expires * 1000
        }
    })
}

fn extract_account_id(tokens: &TokenResponse) -> Option<String> {
    extract_account_id_from_jwt(&tokens.id_token)
        .or_else(|| extract_account_id_from_jwt(&tokens.access_token))
}

fn extract_account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("chatgpt_account_id")
        .and_then(Value::as_str)
        .or_else(|| {
            claims
                .get("https://api.openai.com/auth")
                .and_then(|auth| auth.get("chatgpt_account_id"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            claims
                .get("organizations")
                .and_then(Value::as_array)
                .and_then(|organizations| organizations.first())
                .and_then(|organization| organization.get("id"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{Json, Router, http::HeaderMap, routing::get};
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;

    fn jwt(claims: Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
        format!("{header}.{payload}.sig")
    }

    #[test]
    fn raw_value_is_treated_as_refresh_token() {
        let credential = parse_codex_subscription_credential("refresh-old").expect("parse");

        assert_eq!(credential.refresh, "refresh-old");
        assert!(credential.access.is_none());
        assert_eq!(credential.issuer, DEFAULT_CODEX_ISSUER);
    }

    #[test]
    fn rejects_opencode_oauth_json() {
        let error = parse_codex_subscription_credential(
            r#"{
                "type": "oauth",
                "refresh": "refresh-old",
                "access": "access-old",
                "expires": 4102444800000,
                "accountId": "acc-123"
            }"#,
        )
        .expect_err("reject opencode credential object");

        assert!(error.to_string().contains("raw refresh token"));
    }

    #[test]
    fn parses_stored_token_cache() {
        let credential = parse_codex_subscription_credential(
            r#"{
                "type": "token-toxication-codex-oauth-v1",
                "refresh": "refresh-old",
                "access": "access-old",
                "expires": 4102444800000,
                "accountId": "acc-456"
            }"#,
        )
        .expect("parse");

        assert_eq!(credential.refresh, "refresh-old");
        assert_eq!(credential.access.as_deref(), Some("access-old"));
        assert_eq!(credential.expires, Some(4102444800000));
        assert_eq!(credential.account_id.as_deref(), Some("acc-456"));
    }

    #[test]
    fn extracts_account_id_from_jwt_claims() {
        let token = jwt(json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc-nested"
            }
        }));

        assert_eq!(
            extract_account_id_from_jwt(&token).as_deref(),
            Some("acc-nested")
        );
    }

    #[test]
    fn codex_endpoints_use_account_api_root_and_accept_legacy_urls() {
        let root = "https://relay.example/backend-api";
        assert_eq!(
            codex_subscription_endpoint(root).expect("responses endpoint"),
            "https://relay.example/backend-api/codex/responses"
        );
        assert_eq!(
            codex_quota_endpoint(root).expect("quota endpoint"),
            "https://relay.example/backend-api/wham/usage"
        );

        for legacy in [
            "https://relay.example/backend-api/codex",
            "https://relay.example/backend-api/codex/responses",
        ] {
            assert_eq!(
                codex_subscription_endpoint(legacy).expect("legacy responses endpoint"),
                "https://relay.example/backend-api/codex/responses"
            );
            assert_eq!(
                codex_quota_endpoint(legacy).expect("legacy quota endpoint"),
                "https://relay.example/backend-api/wham/usage"
            );
        }
    }

    #[test]
    fn codex_endpoints_preserve_relay_path_prefix() {
        assert_eq!(
            codex_subscription_endpoint("https://relay.example/tenant/backend-api")
                .expect("responses endpoint"),
            "https://relay.example/tenant/backend-api/codex/responses"
        );
        assert_eq!(
            codex_quota_endpoint("https://relay.example/tenant/backend-api/codex")
                .expect("quota endpoint"),
            "https://relay.example/tenant/backend-api/wham/usage"
        );
        assert!(codex_quota_endpoint("").is_err());
    }

    #[test]
    fn canonicalizes_only_legacy_codex_base_urls() {
        for legacy in [
            "https://relay.example/backend-api/codex",
            "https://relay.example/backend-api/codex/responses/",
        ] {
            assert_eq!(
                canonicalize_legacy_codex_base_url(legacy).as_deref(),
                Some("https://relay.example/backend-api")
            );
        }
        assert_eq!(
            canonicalize_legacy_codex_base_url("https://relay.example/codex").as_deref(),
            Some("https://relay.example")
        );
        assert_eq!(
            canonicalize_legacy_codex_base_url("https://relay.example/backend-api"),
            None
        );
        assert_eq!(
            canonicalize_legacy_codex_base_url("https://relay.example/backend-api/responses"),
            None
        );
        assert_eq!(canonicalize_legacy_codex_base_url("not a URL"), None);
    }

    #[test]
    fn parses_complete_codex_quota_payload() {
        let now = DateTime::parse_from_rfc3339("2026-07-14T00:00:00Z")
            .expect("time")
            .with_timezone(&Utc);
        let payload: CodexUsagePayload = serde_json::from_value(json!({
            "plan_type": "pro",
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": {
                    "used_percent": 24,
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 60,
                    "reset_at": 1800000000
                },
                "secondary_window": {
                    "used_percent": 101,
                    "limit_window_seconds": 2592000,
                    "reset_after_seconds": 120,
                    "reset_at": 0
                }
            },
            "additional_rate_limits": [{
                "limit_name": "GPT-5.3-Codex-Spark",
                "metered_feature": "codex_bengalfox",
                "rate_limit": {
                    "allowed": true,
                    "limit_reached": false,
                    "primary_window": {
                        "used_percent": 17,
                        "limit_window_seconds": 18000,
                        "reset_after_seconds": 30,
                        "reset_at": 1800000030
                    }
                }
            }],
            "credits": {
                "has_credits": true,
                "unlimited": false,
                "balance": 12.5
            },
            "spend_control": {
                "reached": false,
                "individual_limit": {
                    "source": "workspace",
                    "limit": "100",
                    "used": "25",
                    "remaining": "75",
                    "used_percent": 25,
                    "remaining_percent": 75,
                    "reset_after_seconds": 3600,
                    "reset_at": 1800003600
                }
            },
            "rate_limit_reached_type": {
                "type": "workspace_member_usage_limit_reached"
            },
            "rate_limit_reset_credits": {
                "available_count": 2
            }
        }))
        .expect("payload");

        let quota = codex_quota_response(
            "provider-1".to_string(),
            "codex-oauth".to_string(),
            "https://relay.example/backend-api/wham/usage".to_string(),
            payload,
            now,
        );

        assert_eq!(quota.plan_type.as_deref(), Some("pro"));
        assert_eq!(quota.limits.len(), 2);
        assert_eq!(quota.limits[0].limit_id, "codex");
        assert_eq!(
            quota.limits[0]
                .secondary_window
                .as_ref()
                .and_then(|window| window.limit_window_seconds),
            Some(2_592_000)
        );
        assert_eq!(
            quota.limits[0]
                .secondary_window
                .as_ref()
                .and_then(|window| window.used_percent),
            Some(100.0)
        );
        assert_eq!(
            quota.limits[0]
                .secondary_window
                .as_ref()
                .and_then(|window| window.reset_at),
            now.checked_add_signed(chrono::Duration::seconds(120))
        );
        assert_eq!(quota.limits[1].limit_id, "codex_bengalfox");
        assert_eq!(quota.limits[1].display_name, "GPT-5.3-Codex-Spark");
        assert_eq!(
            quota
                .credits
                .as_ref()
                .and_then(|credits| credits.balance.as_deref()),
            Some("12.5")
        );
        assert_eq!(
            quota
                .spend_control
                .as_ref()
                .and_then(|spend| spend.individual_limit.as_ref())
                .and_then(|limit| limit.remaining.as_deref()),
            Some("75")
        );
        assert_eq!(
            quota.rate_limit_reached_type.as_deref(),
            Some("workspace_member_usage_limit_reached")
        );
        assert_eq!(quota.reset_credits_available_count, Some(2));
    }

    #[test]
    fn preserves_additional_only_quota_payloads() {
        let payload: CodexUsagePayload = serde_json::from_value(json!({
            "plan_type": "pro",
            "rate_limit": null,
            "additional_rate_limits": [{
                "limit_name": "GPT-5.3-Codex-Spark",
                "metered_feature": "codex_bengalfox",
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 5,
                        "limit_window_seconds": 18000,
                        "reset_at": 1800000000
                    }
                }
            }]
        }))
        .expect("payload");

        let quota = codex_quota_response(
            "provider-1".to_string(),
            "codex-oauth".to_string(),
            "https://relay.example/backend-api/wham/usage".to_string(),
            payload,
            Utc::now(),
        );

        assert_eq!(quota.limits.len(), 1);
        assert_eq!(quota.limits[0].limit_id, "codex_bengalfox");
    }

    #[test]
    fn maps_codex_quota_auth_and_relay_errors() {
        let auth = codex_quota_upstream_error(
            StatusCode::FORBIDDEN,
            br#"{"error":{"message":"expired"}}"#,
        );
        assert_eq!(auth.status(), StatusCode::UNAUTHORIZED);
        assert!(auth.to_string().contains("expired"));

        let relay = codex_quota_upstream_error(StatusCode::NOT_FOUND, b"not relayed");
        assert_eq!(relay.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(relay.to_string().contains("404 not relayed"));
    }

    #[tokio::test]
    async fn quota_query_uses_the_relay_url_and_selected_account_headers() {
        let seen_headers = Arc::new(Mutex::new(None));
        let handler_headers = Arc::clone(&seen_headers);
        let app = Router::new().route(
            "/tenant/backend-api/wham/usage",
            get(move |headers: HeaderMap| {
                let handler_headers = Arc::clone(&handler_headers);
                async move {
                    *handler_headers.lock().expect("headers lock") = Some(headers);
                    Json(json!({ "plan_type": "plus", "rate_limit": null }))
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        let http = test_http_client();
        let endpoint = format!("http://{address}/tenant/backend-api/wham/usage");

        let payload = fetch_codex_usage(
            &http,
            &endpoint,
            "access-1",
            Some("account-1"),
            Duration::from_secs(2),
        )
        .await
        .expect("quota response");
        server.abort();

        assert_eq!(payload.plan_type.as_deref(), Some("plus"));
        let headers = seen_headers
            .lock()
            .expect("headers lock")
            .take()
            .expect("captured headers");
        assert_eq!(
            headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer access-1")
        );
        assert_eq!(
            headers
                .get("chatgpt-account-id")
                .and_then(|value| value.to_str().ok()),
            Some("account-1")
        );
        assert_eq!(
            headers
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some("codex-cli")
        );
    }

    #[tokio::test]
    async fn quota_query_has_a_bounded_timeout() {
        let app = Router::new().route(
            "/backend-api/wham/usage",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Json(json!({ "plan_type": "plus" }))
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        let http = test_http_client();
        let endpoint = format!("http://{address}/backend-api/wham/usage");

        let error = fetch_codex_usage(
            &http,
            &endpoint,
            "access-1",
            None,
            Duration::from_millis(25),
        )
        .await
        .expect_err("request should time out");
        server.abort();

        assert!(matches!(error, AppError::Upstream(_)));
    }

    fn test_http_client() -> TokioClient {
        TokioClient::builder()
            .tls(aioduct::tls::RustlsConnector::with_webpki_roots())
            .user_agent("token-toxication-test")
            .timeout(Duration::from_secs(5))
            .build()
            .expect("HTTP client")
    }
}
