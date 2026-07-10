use std::{collections::HashMap, sync::Arc};

use aioduct::{
    RequestBuilderSend, TokioClient,
    runtime::{ConnectorSend, RuntimePoll},
};
use axum::http::{HeaderValue, StatusCode, header};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use url::Url;

use crate::{
    db::Db,
    error::AppError,
    gemini_code_assist::{
        DEFAULT_GEMINI_CODE_ASSIST_ENDPOINT, GOOGLE_OAUTH_TOKEN_ENDPOINT,
        STORED_ANTIGRAVITY_CREDENTIAL_TYPE, antigravity_metadata_platform,
        antigravity_oauth_client_id, antigravity_oauth_client_secret,
        gemini_code_assist_authorization, gemini_code_assist_method_url, is_antigravity_oauth_auth,
    },
    models::{
        AntigravityOAuthStartRequest, AntigravityOAuthStartResponse, CreateProviderAccountRequest,
        GeminiAccountModel, GeminiAccountModelsResponse, GeminiAccountQuota,
        GeminiAccountQuotaBucket, GeminiAccountQuotaGroup, GeminiAccountQuotaResponse,
        GeminiAccountQuotaSummary, GeminiAccountTier, ProviderAccount,
        UpdateProviderAccountRequest,
    },
};

const GOOGLE_OAUTH_AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const OAUTH_ATTEMPT_MINUTES: i64 = 10;
const ANTIGRAVITY_VERSION_ENV: &str = "TT_ANTIGRAVITY_VERSION";
const ANTIGRAVITY_USER_AGENT_ENV: &str = "TT_ANTIGRAVITY_USER_AGENT";
const DEFAULT_ANTIGRAVITY_VERSION: &str = "2.2.1";
const OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];

#[derive(Clone, Default)]
pub struct AntigravityOAuthStore {
    attempts: Arc<Mutex<HashMap<String, AntigravityOAuthAttempt>>>,
}

#[derive(Debug, Clone)]
struct AntigravityOAuthAttempt {
    verifier: String,
    redirect_uri: String,
    opener_origin: String,
    account_id: Option<String>,
    name: String,
    priority: i32,
    created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct AntigravityOAuthOutcome {
    pub opener_origin: Option<String>,
    pub account_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Clone)]
struct AvailableModel {
    id: String,
    display_name: String,
    remaining_fraction: Option<f64>,
    reset_time: Option<DateTime<Utc>>,
}

pub async fn begin_antigravity_oauth(
    store: &AntigravityOAuthStore,
    input: AntigravityOAuthStartRequest,
) -> Result<AntigravityOAuthStartResponse, AppError> {
    antigravity_oauth_client_secret()?;
    let (redirect_uri, opener_origin) = validate_redirect_uri(&input.redirect_uri)?;

    let state = random_url_safe(32);
    let verifier = random_url_safe(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let created_at = Utc::now();
    let expires_at = created_at + Duration::minutes(OAUTH_ATTEMPT_MINUTES);
    let attempt = AntigravityOAuthAttempt {
        verifier,
        redirect_uri: redirect_uri.clone(),
        opener_origin,
        account_id: input.account_id.filter(|value| !value.trim().is_empty()),
        name: input.name.trim().to_string(),
        priority: input.priority,
        created_at,
    };

    let mut attempts = store.attempts.lock().await;
    attempts.retain(|_, attempt| {
        attempt.created_at + Duration::minutes(OAUTH_ATTEMPT_MINUTES) > created_at
    });
    attempts.insert(state.clone(), attempt);
    drop(attempts);

    let mut authorization_url = Url::parse(GOOGLE_OAUTH_AUTHORIZATION_ENDPOINT)
        .map_err(|error| AppError::Internal(format!("invalid Google OAuth URL: {error}")))?;
    authorization_url
        .query_pairs_mut()
        .append_pair("client_id", &antigravity_oauth_client_id())
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", &OAUTH_SCOPES.join(" "))
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");

    Ok(AntigravityOAuthStartResponse {
        authorization_url: authorization_url.to_string(),
        expires_at,
    })
}

pub async fn complete_antigravity_oauth(
    store: &AntigravityOAuthStore,
    db: &Db,
    http: &TokioClient,
    state: Option<&str>,
    code: Option<&str>,
    oauth_error: Option<&str>,
    oauth_error_description: Option<&str>,
) -> AntigravityOAuthOutcome {
    let Some(state) = state.filter(|value| !value.trim().is_empty()) else {
        return AntigravityOAuthOutcome {
            opener_origin: None,
            account_id: None,
            error: Some("missing OAuth state".to_string()),
        };
    };
    let attempt = store.attempts.lock().await.remove(state);
    let Some(attempt) = attempt else {
        return AntigravityOAuthOutcome {
            opener_origin: None,
            account_id: None,
            error: Some("OAuth state is invalid, expired, or already used".to_string()),
        };
    };
    let opener_origin = Some(attempt.opener_origin.clone());

    if attempt.created_at + Duration::minutes(OAUTH_ATTEMPT_MINUTES) <= Utc::now() {
        return AntigravityOAuthOutcome {
            opener_origin,
            account_id: None,
            error: Some("OAuth login expired; start the sign-in again".to_string()),
        };
    }
    if let Some(error) = oauth_error {
        let description = oauth_error_description.unwrap_or(error);
        return AntigravityOAuthOutcome {
            opener_origin,
            account_id: None,
            error: Some(format!("Google sign-in failed: {description}")),
        };
    }
    let Some(code) = code.filter(|value| !value.trim().is_empty()) else {
        return AntigravityOAuthOutcome {
            opener_origin,
            account_id: None,
            error: Some("Google did not return an authorization code".to_string()),
        };
    };

    match finish_antigravity_oauth(db, http, code, &attempt).await {
        Ok(account) => AntigravityOAuthOutcome {
            opener_origin,
            account_id: Some(account.id),
            error: None,
        },
        Err(error) => AntigravityOAuthOutcome {
            opener_origin,
            account_id: None,
            error: Some(error.to_string()),
        },
    }
}

async fn finish_antigravity_oauth(
    db: &Db,
    http: &TokioClient,
    code: &str,
    attempt: &AntigravityOAuthAttempt,
) -> Result<ProviderAccount, AppError> {
    let client_id = antigravity_oauth_client_id();
    let client_secret = antigravity_oauth_client_secret()?;
    let response = http
        .post(GOOGLE_OAUTH_TOKEN_ENDPOINT)?
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", attempt.redirect_uri.as_str()),
            ("code_verifier", attempt.verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|error| AppError::Upstream(error.into()))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        return Err(oauth_upstream_error(
            "Antigravity OAuth token exchange",
            status,
            &bytes,
        ));
    }
    let tokens: GoogleTokenResponse = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::Internal(format!("invalid Antigravity OAuth token response: {error}"))
    })?;
    let refresh = tokens.refresh_token.ok_or_else(|| {
        AppError::Unauthorized(
            "Antigravity OAuth did not return a refresh token; revoke access and sign in again"
                .to_string(),
        )
    })?;
    let expires = Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000;
    let credential = serde_json::to_string(&json!({
        "type": STORED_ANTIGRAVITY_CREDENTIAL_TYPE,
        "refresh": refresh,
        "access": tokens.access_token,
        "expires": expires,
        "endpoint": DEFAULT_GEMINI_CODE_ASSIST_ENDPOINT,
        "clientId": client_id,
    }))
    .map_err(|error| AppError::Internal(format!("serialize Antigravity credential: {error}")))?;

    let account = if let Some(account_id) = attempt.account_id.as_deref() {
        db.get_provider_account(account_id)
            .await?
            .ok_or_else(|| AppError::NotFound("provider account not found".to_string()))?;
        db.update_provider_account(
            account_id,
            UpdateProviderAccountRequest {
                name: (!attempt.name.is_empty()).then(|| attempt.name.clone()),
                provider: Some("gemini".to_string()),
                base_url: Some(DEFAULT_GEMINI_CODE_ASSIST_ENDPOINT.to_string()),
                auth_mode: Some("antigravity-oauth".to_string()),
                wire_api: Some("gemini-generate-content".to_string()),
                api_key: Some(credential),
                is_active: Some(true),
                priority: Some(attempt.priority),
            },
        )
        .await?
    } else {
        db.create_provider_account(CreateProviderAccountRequest {
            name: if attempt.name.is_empty() {
                "Gemini Account".to_string()
            } else {
                attempt.name.clone()
            },
            provider: "gemini".to_string(),
            base_url: DEFAULT_GEMINI_CODE_ASSIST_ENDPOINT.to_string(),
            auth_mode: "antigravity-oauth".to_string(),
            wire_api: "gemini-generate-content".to_string(),
            api_key: credential,
            is_active: true,
            priority: attempt.priority,
        })
        .await?
    };
    db.mark_provider_result(&account.id, "healthy", None)
        .await?;
    Ok(account)
}

pub async fn gemini_account_models(
    db: &Db,
    http: &TokioClient,
    account_id: &str,
) -> Result<GeminiAccountModelsResponse, AppError> {
    let account = gemini_account_record(db, account_id).await?;
    let authorization = gemini_code_assist_authorization(db, http, &account).await?;
    let available = fetch_available_models(
        http,
        &authorization.endpoint,
        &authorization.access_token,
        authorization.project.as_deref(),
    )
    .await?;
    let models = available
        .into_iter()
        .map(|model| GeminiAccountModel {
            id: model.id,
            display_name: model.display_name,
        })
        .collect();

    Ok(GeminiAccountModelsResponse {
        account_id: account.account.id,
        project: authorization.project,
        endpoint: authorization.endpoint,
        models,
    })
}

pub async fn gemini_account_quota(
    db: &Db,
    http: &TokioClient,
    account_id: &str,
) -> Result<GeminiAccountQuotaResponse, AppError> {
    let account = gemini_account_record(db, account_id).await?;
    let authorization = gemini_code_assist_authorization(db, http, &account).await?;
    let available = fetch_available_models(
        http,
        &authorization.endpoint,
        &authorization.access_token,
        authorization.project.as_deref(),
    )
    .await?;
    let quotas = available
        .into_iter()
        .map(|model| GeminiAccountQuota {
            model_id: model.id,
            remaining_fraction: model.remaining_fraction,
            reset_time: model.reset_time,
        })
        .collect();
    let (current_tier, paid_tier) =
        load_code_assist_tiers(http, &authorization.endpoint, &authorization.access_token)
            .await
            .unwrap_or((None, None));
    let (quota_summary, quota_summary_error) = match post_code_assist_json(
        http,
        &authorization.endpoint,
        "retrieveUserQuotaSummary",
        &authorization.access_token,
        &project_body(authorization.project.as_deref()),
    )
    .await
    {
        Ok(value) => (Some(parse_quota_summary(&value)), None),
        Err(error) => (None, Some(error.to_string())),
    };

    Ok(GeminiAccountQuotaResponse {
        account_id: account.account.id,
        auth_mode: account.account.auth_mode,
        project: authorization.project,
        endpoint: authorization.endpoint,
        quota_source: "fetchAvailableModels".to_string(),
        current_tier,
        paid_tier,
        quotas,
        quota_summary,
        quota_summary_error,
    })
}

async fn gemini_account_record(
    db: &Db,
    account_id: &str,
) -> Result<crate::models::ProviderAccountRecord, AppError> {
    let account = db
        .get_provider_account_record(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound("provider account not found".to_string()))?;
    if account.account.provider != "gemini"
        || !is_antigravity_oauth_auth(&account.account.auth_mode)
    {
        return Err(AppError::BadRequest(
            "provider account does not use Antigravity OAuth".to_string(),
        ));
    }
    Ok(account)
}

async fn fetch_available_models(
    http: &TokioClient,
    endpoint: &str,
    access_token: &str,
    project: Option<&str>,
) -> Result<Vec<AvailableModel>, AppError> {
    let value = post_code_assist_json(
        http,
        endpoint,
        "fetchAvailableModels",
        access_token,
        &project_body(project),
    )
    .await?;
    let mut models = parse_antigravity_models(&value);
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

async fn load_code_assist_tiers(
    http: &TokioClient,
    endpoint: &str,
    access_token: &str,
) -> Result<(Option<GeminiAccountTier>, Option<GeminiAccountTier>), AppError> {
    let metadata = json!({
        "ideType": "ANTIGRAVITY",
        "platform": antigravity_metadata_platform(),
        "pluginType": "GEMINI"
    });
    let value = post_code_assist_json(
        http,
        endpoint,
        "loadCodeAssist",
        access_token,
        &json!({ "metadata": metadata }),
    )
    .await?;
    Ok((
        parse_tier(value.get("currentTier")),
        parse_tier(value.get("paidTier")),
    ))
}

async fn post_code_assist_json(
    http: &TokioClient,
    endpoint: &str,
    method: &str,
    access_token: &str,
    body: &Value,
) -> Result<Value, AppError> {
    let url = gemini_code_assist_method_url(endpoint, method);
    let body = serde_json::to_vec(body).map_err(|error| {
        AppError::Internal(format!("serialize Gemini account request: {error}"))
    })?;
    let request = http
        .post(&url)?
        .bearer_auth(access_token)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(body);
    let request = apply_antigravity_headers(request)?;
    let response = request
        .send()
        .await
        .map_err(|error| AppError::Upstream(error.into()))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        return Err(oauth_upstream_error(
            &format!("Gemini account {method}"),
            status,
            &bytes,
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::Internal(format!("invalid Gemini account response: {error}")))
}

pub fn apply_antigravity_headers<'a, R, C>(
    request: RequestBuilderSend<'a, R, C>,
) -> Result<RequestBuilderSend<'a, R, C>, AppError>
where
    R: RuntimePoll,
    C: ConnectorSend,
{
    let user_agent = HeaderValue::from_str(&antigravity_user_agent())
        .map_err(|error| AppError::Internal(format!("invalid Antigravity user agent: {error}")))?;
    Ok(request.header(header::USER_AGENT, user_agent))
}

fn parse_antigravity_models(value: &Value) -> Vec<AvailableModel> {
    match value.get("models") {
        Some(Value::Object(models)) => models
            .iter()
            .map(|(id, value)| available_model(id, value))
            .collect(),
        Some(Value::Array(models)) => models
            .iter()
            .filter_map(|value| {
                let id = string_at(value, &["id", "name", "modelId"])?;
                Some(available_model(&id, value))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn available_model(id: &str, value: &Value) -> AvailableModel {
    let quota = value.get("quotaInfo").unwrap_or(value);
    AvailableModel {
        id: id.to_string(),
        display_name: string_at(value, &["displayName", "display_name", "name"])
            .unwrap_or_else(|| id.to_string()),
        remaining_fraction: number_at(quota, &["remainingFraction", "remaining_fraction"]),
        reset_time: time_at(quota, &["resetTime", "reset_time"]),
    }
}

fn parse_tier(value: Option<&Value>) -> Option<GeminiAccountTier> {
    let value = value?;
    Some(GeminiAccountTier {
        id: string_at(value, &["id"]).unwrap_or_default(),
        name: string_at(value, &["name"]).unwrap_or_default(),
        description: string_at(value, &["description"]).unwrap_or_default(),
    })
}

fn parse_quota_summary(value: &Value) -> GeminiAccountQuotaSummary {
    GeminiAccountQuotaSummary {
        description: string_at(value, &["description"]),
        buckets: value
            .get("buckets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(parse_quota_bucket)
            .collect(),
        groups: value
            .get("groups")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|group| GeminiAccountQuotaGroup {
                display_name: string_at(group, &["displayName", "display_name"])
                    .unwrap_or_default(),
                description: string_at(group, &["description"]),
                buckets: group
                    .get("buckets")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(parse_quota_bucket)
                    .collect(),
            })
            .collect(),
    }
}

fn parse_quota_bucket(value: &Value) -> GeminiAccountQuotaBucket {
    GeminiAccountQuotaBucket {
        bucket_id: string_at(value, &["bucketId", "bucket_id"]).unwrap_or_default(),
        display_name: string_at(value, &["displayName", "display_name"]).unwrap_or_default(),
        description: string_at(value, &["description"]),
        window: string_at(value, &["window"]),
        remaining_fraction: number_at(value, &["remainingFraction", "remaining_fraction"]),
        remaining_amount: number_at(value, &["remainingAmount", "remaining_amount"]),
        disabled: bool_at(value, &["disabled"]),
        reset_time: time_at(value, &["resetTime", "reset_time"]),
    }
}

fn string_at(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn number_at(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
}

fn bool_at(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

fn time_at(value: &Value, keys: &[&str]) -> Option<DateTime<Utc>> {
    keys.iter().find_map(|key| {
        DateTime::parse_from_rfc3339(value.get(*key)?.as_str()?)
            .ok()
            .map(|value| value.with_timezone(&Utc))
    })
}

fn project_body(project: Option<&str>) -> Value {
    project
        .filter(|value| !value.trim().is_empty())
        .map_or_else(|| json!({}), |project| json!({ "project": project }))
}

fn validate_redirect_uri(value: &str) -> Result<(String, String), AppError> {
    let url = Url::parse(value)
        .map_err(|error| AppError::BadRequest(format!("invalid OAuth redirect URI: {error}")))?;
    if !matches!(url.scheme(), "http" | "https")
        || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
        || url.path() != "/oauth-callback"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::BadRequest(
            "Antigravity OAuth redirect URI must be a loopback URL ending in /oauth-callback"
                .to_string(),
        ));
    }
    Ok((url.to_string(), url.origin().ascii_serialization()))
}

fn oauth_upstream_error(context: &str, status: StatusCode, bytes: &[u8]) -> AppError {
    let message = serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.get("error_description"))
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).chars().take(500).collect());
    let message = format!("{context} failed: {} {}", status.as_u16(), message.trim());
    if matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        AppError::Unauthorized(message)
    } else {
        AppError::Internal(message)
    }
}

fn antigravity_user_agent() -> String {
    if let Ok(value) = std::env::var(ANTIGRAVITY_USER_AGENT_ENV)
        && !value.trim().is_empty()
    {
        return value.trim().to_string();
    }
    let version = std::env::var(ANTIGRAVITY_VERSION_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_ANTIGRAVITY_VERSION.to_string());
    format!(
        "antigravity/{version} {}",
        antigravity_user_agent_platform()
    )
}

fn antigravity_user_agent_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin/arm64",
        ("macos", "x86_64") => "darwin/amd64",
        ("linux", "aarch64") => "linux/arm64",
        ("linux", "x86_64") => "linux/amd64",
        ("windows", "x86_64") => "windows/amd64",
        _ => "unknown/unknown",
    }
}

fn random_url_safe(size: usize) -> String {
    let mut bytes = vec![0_u8; size];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_antigravity_model_map() {
        let models = parse_antigravity_models(&json!({
            "models": {
                "gemini-3.1-pro-high": {
                    "displayName": "Gemini 3.1 Pro (High)",
                    "quotaInfo": {
                        "remainingFraction": 0.75,
                        "resetTime": "2026-07-11T02:41:18Z"
                    }
                }
            }
        }));

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gemini-3.1-pro-high");
        assert_eq!(models[0].display_name, "Gemini 3.1 Pro (High)");
        assert_eq!(models[0].remaining_fraction, Some(0.75));
        assert!(models[0].reset_time.is_some());
    }

    #[test]
    fn parses_antigravity_quota_summary() {
        let summary = parse_quota_summary(&json!({
            "description": "Account limits",
            "groups": [{
                "displayName": "Gemini Models",
                "description": "Gemini Flash and Pro",
                "buckets": [{
                    "bucketId": "gemini-5h",
                    "displayName": "Five Hour Limit",
                    "remainingFraction": 0.75,
                    "resetTime": "2026-07-10T09:53:57Z"
                }]
            }]
        }));

        assert_eq!(summary.description.as_deref(), Some("Account limits"));
        assert_eq!(summary.groups.len(), 1);
        assert_eq!(summary.groups[0].buckets.len(), 1);
        assert_eq!(summary.groups[0].buckets[0].bucket_id, "gemini-5h");
        assert_eq!(summary.groups[0].buckets[0].remaining_fraction, Some(0.75));
        assert!(summary.groups[0].buckets[0].reset_time.is_some());
    }

    #[test]
    fn rejects_non_loopback_redirects() {
        let error = validate_redirect_uri("https://example.com/oauth-callback")
            .expect_err("reject remote callback");

        assert!(error.to_string().contains("loopback"));
    }

    #[test]
    fn accepts_local_callback() {
        let (redirect, origin) =
            validate_redirect_uri("http://127.0.0.1:3000/oauth-callback").expect("valid callback");

        assert_eq!(redirect, "http://127.0.0.1:3000/oauth-callback");
        assert_eq!(origin, "http://127.0.0.1:3000");
    }
}
