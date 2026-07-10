use aioduct::TokioClient;
use axum::http::{HeaderValue, StatusCode, header};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{db::Db, error::AppError, models::ProviderAccountRecord};

pub(crate) const DEFAULT_GEMINI_CODE_ASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com";
pub(crate) const GOOGLE_OAUTH_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
pub(crate) const ANTIGRAVITY_OAUTH_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
pub(crate) const ANTIGRAVITY_OAUTH_CLIENT_ID_ENV: &str = "TT_ANTIGRAVITY_OAUTH_CLIENT_ID";
pub(crate) const ANTIGRAVITY_OAUTH_CLIENT_SECRET_ENV: &str = "TT_ANTIGRAVITY_OAUTH_CLIENT_SECRET";
const REFRESH_SAFETY_MARGIN_MS: i64 = 30_000;
pub(crate) const STORED_ANTIGRAVITY_CREDENTIAL_TYPE: &str = "token-toxication-antigravity-oauth-v1";
const INVALID_ANTIGRAVITY_CREDENTIAL: &str =
    "Antigravity OAuth credential is invalid; reconnect the provider account";

#[derive(Debug, Clone)]
pub struct GeminiCodeAssistAuthorization {
    pub access_token: String,
    pub project: Option<String>,
    pub endpoint: String,
}

#[derive(Debug, Clone)]
struct GeminiCodeAssistCredential {
    refresh: String,
    access: Option<String>,
    expires: Option<i64>,
    project: Option<String>,
    endpoint: Option<String>,
    client_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct StoredAntigravityCredential {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    refresh: String,
    #[serde(default)]
    access: Option<String>,
    #[serde(default)]
    expires: Option<i64>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default, rename = "clientId")]
    client_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct LoadCodeAssistResponse {
    #[serde(default, rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<String>,
}

pub fn is_antigravity_oauth_auth(auth_mode: &str) -> bool {
    auth_mode == "antigravity-oauth"
}

pub fn gemini_code_assist_endpoint(base_url: &str) -> String {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return DEFAULT_GEMINI_CODE_ASSIST_ENDPOINT.to_string();
    }
    base_url.to_string()
}

pub async fn gemini_code_assist_authorization(
    db: &Db,
    http: &TokioClient,
    account: &ProviderAccountRecord,
) -> Result<GeminiCodeAssistAuthorization, AppError> {
    let mut credential = parse_gemini_code_assist_credential(&account.api_key)?;
    let endpoint = credential
        .endpoint
        .clone()
        .unwrap_or_else(|| gemini_code_assist_endpoint(&account.account.base_url));
    let mut changed = false;

    if credential
        .access
        .as_deref()
        .is_none_or(|access| access.trim().is_empty())
        || credential_is_expired(credential.expires)
    {
        let tokens = refresh_gemini_code_assist_token(http, &credential).await?;
        credential.access = Some(tokens.access_token.clone());
        if let Some(refresh) = tokens
            .refresh_token
            .filter(|refresh| !refresh.trim().is_empty())
        {
            credential.refresh = refresh;
        }
        credential.expires =
            Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000);
        changed = true;
    }

    let access_token = credential.access.clone().unwrap_or_default();
    if credential.project.is_none()
        && let Some(project) =
            discover_gemini_code_assist_project(http, &endpoint, &access_token).await?
    {
        credential.project = Some(project);
        changed = true;
    }
    if changed {
        db.update_provider_account_secret(&account.account.id, &serialize_credential(&credential))
            .await?;
    }

    Ok(GeminiCodeAssistAuthorization {
        access_token,
        project: credential.project,
        endpoint,
    })
}

pub fn build_code_assist_request(
    request: &Value,
    model: &str,
    project: Option<&str>,
    session_id: Option<&str>,
) -> Value {
    let mut inner = Map::new();
    for key in [
        "contents",
        "systemInstruction",
        "cachedContent",
        "tools",
        "toolConfig",
        "labels",
        "safetySettings",
        "generationConfig",
    ] {
        if let Some(value) = request.get(key) {
            let value = if key == "contents" {
                normalize_code_assist_contents(value)
            } else {
                value.clone()
            };
            inner.insert(key.to_string(), value);
        }
    }
    let session_id = session_id
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(antigravity_session_id);
    inner.insert("sessionId".to_string(), Value::String(session_id));

    let mut outer = Map::new();
    outer.insert("model".to_string(), Value::String(model.to_string()));
    if let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        outer.insert("project".to_string(), Value::String(project.to_string()));
    }
    outer.insert(
        "requestId".to_string(),
        Value::String(format!("agent-{}", Uuid::new_v4())),
    );
    outer.insert(
        "requestType".to_string(),
        Value::String("agent".to_string()),
    );
    outer.insert(
        "userAgent".to_string(),
        Value::String("antigravity".to_string()),
    );
    outer.insert("request".to_string(), Value::Object(inner));
    Value::Object(outer)
}

fn antigravity_session_id() -> String {
    let value = Uuid::new_v4().as_u128() % 9_000_000_000_000_000_000_u128;
    format!("-{value}")
}

fn normalize_code_assist_contents(value: &Value) -> Value {
    match value {
        Value::Array(contents) => {
            Value::Array(contents.iter().map(normalize_code_assist_content).collect())
        }
        Value::Object(_) => normalize_code_assist_content(value),
        _ => value.clone(),
    }
}

fn normalize_code_assist_content(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object
            .entry("role".to_string())
            .or_insert_with(|| Value::String("user".to_string()));
    }
    value
}

pub fn unwrap_code_assist_response_bytes(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        AppError::Internal(format!("invalid Gemini upstream response: {error}"))
    })?;
    let unwrapped = unwrap_code_assist_response_value(value);
    serde_json::to_vec(&unwrapped)
        .map_err(|error| AppError::Internal(format!("serialize Gemini response: {error}")))
}

pub fn unwrap_code_assist_sse_data(data: &str) -> String {
    match serde_json::from_str::<Value>(data) {
        Ok(value) => unwrap_code_assist_response_value(value).to_string(),
        Err(_) => data.to_string(),
    }
}

fn unwrap_code_assist_response_value(value: Value) -> Value {
    let trace_id = value
        .get("traceId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let Some(mut response) = value.get("response").cloned() else {
        return value;
    };
    if let (Some(trace_id), Some(object)) = (trace_id, response.as_object_mut()) {
        object
            .entry("responseId".to_string())
            .or_insert(Value::String(trace_id));
    }
    response
}

fn parse_gemini_code_assist_credential(
    value: &str,
) -> Result<GeminiCodeAssistCredential, AppError> {
    let value = value.trim();
    let stored: StoredAntigravityCredential = serde_json::from_str(value)
        .map_err(|_| AppError::BadRequest(INVALID_ANTIGRAVITY_CREDENTIAL.into()))?;
    if stored.r#type != STORED_ANTIGRAVITY_CREDENTIAL_TYPE || stored.refresh.trim().is_empty() {
        return Err(AppError::BadRequest(INVALID_ANTIGRAVITY_CREDENTIAL.into()));
    }
    Ok(GeminiCodeAssistCredential {
        refresh: stored.refresh,
        access: stored.access,
        expires: normalize_expires(stored.expires),
        project: stored.project.filter(|value| !value.trim().is_empty()),
        endpoint: stored
            .endpoint
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim_end_matches('/').to_string()),
        client_id: oauth_client_candidate(stored.client_id, ANTIGRAVITY_OAUTH_CLIENT_ID_ENV),
    })
}

fn oauth_client_candidate(provided: Option<String>, env_name: &str) -> Option<String> {
    provided
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var(env_name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

pub(crate) fn antigravity_oauth_client_id() -> String {
    oauth_client_candidate(None, ANTIGRAVITY_OAUTH_CLIENT_ID_ENV)
        .unwrap_or_else(|| ANTIGRAVITY_OAUTH_CLIENT_ID.to_string())
}

pub(crate) fn antigravity_oauth_client_secret() -> Result<String, AppError> {
    oauth_client_candidate(None, ANTIGRAVITY_OAUTH_CLIENT_SECRET_ENV).ok_or_else(|| {
        AppError::BadRequest(format!(
            "Antigravity OAuth client_secret is required; set {ANTIGRAVITY_OAUTH_CLIENT_SECRET_ENV} on the backend"
        ))
    })
}

pub(crate) fn antigravity_metadata_platform() -> &'static str {
    antigravity_metadata_platform_for_target(std::env::consts::OS, std::env::consts::ARCH)
}

fn antigravity_metadata_platform_for_target(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("macos", "aarch64") => "DARWIN_ARM64",
        ("macos", "x86_64") => "DARWIN_AMD64",
        ("linux", "aarch64") => "LINUX_ARM64",
        ("linux", "x86_64") => "LINUX_AMD64",
        ("windows", "x86_64") => "WINDOWS_AMD64",
        _ => "PLATFORM_UNSPECIFIED",
    }
}

async fn refresh_gemini_code_assist_token(
    http: &TokioClient,
    credential: &GeminiCodeAssistCredential,
) -> Result<GoogleTokenResponse, AppError> {
    let client_id = credential
        .client_id
        .clone()
        .unwrap_or_else(antigravity_oauth_client_id);
    let client_secret = antigravity_oauth_client_secret()?;
    let response = http
        .post(GOOGLE_OAUTH_TOKEN_ENDPOINT)?
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", credential.refresh.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
        ])
        .send()
        .await
        .map_err(|error| AppError::Upstream(error.into()))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes);
        let message = format!(
            "Gemini account token refresh failed: {} {}",
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
        .map_err(|error| AppError::Internal(format!("invalid Gemini token response: {error}")))
}

pub(crate) async fn discover_gemini_code_assist_project(
    http: &TokioClient,
    endpoint: &str,
    access_token: &str,
) -> Result<Option<String>, AppError> {
    if access_token.trim().is_empty() {
        return Ok(None);
    }
    let url = gemini_code_assist_method_url(endpoint, "loadCodeAssist");
    let metadata = json!({
        "ideType": "ANTIGRAVITY",
        "platform": antigravity_metadata_platform(),
        "pluginType": "GEMINI"
    });
    let body = serde_json::to_vec(&json!({
        "metadata": metadata
    }))
    .map_err(|error| AppError::Internal(format!("serialize Gemini setup request: {error}")))?;
    let response = http
        .post(&url)?
        .bearer_auth(access_token)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(body)
        .send()
        .await
        .map_err(|error| AppError::Upstream(error.into()))?;
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes);
        let message = format!(
            "Gemini account setup check failed: {} {}",
            status.as_u16(),
            body.trim()
        );
        return if matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            Err(AppError::Unauthorized(message))
        } else {
            Ok(None)
        };
    }
    let load: LoadCodeAssistResponse = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::Internal(format!("invalid Gemini setup response: {error}")))?;
    Ok(load
        .cloudaicompanion_project
        .filter(|project| !project.trim().is_empty()))
}

pub(crate) fn gemini_code_assist_method_url(endpoint: &str, method: &str) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    let endpoint = endpoint
        .strip_suffix("/v1internal")
        .unwrap_or(endpoint)
        .trim_end_matches('/');
    format!("{endpoint}/v1internal:{method}")
}

fn serialize_credential(credential: &GeminiCodeAssistCredential) -> String {
    let stored = StoredAntigravityCredential {
        r#type: STORED_ANTIGRAVITY_CREDENTIAL_TYPE.to_string(),
        refresh: credential.refresh.clone(),
        access: credential.access.clone(),
        expires: credential.expires,
        project: credential.project.clone(),
        endpoint: credential.endpoint.clone(),
        client_id: credential.client_id.clone(),
    };
    serde_json::to_string(&stored).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_antigravity_stored_credential() {
        let credential = parse_gemini_code_assist_credential(
            r#"{"type":"token-toxication-antigravity-oauth-v1","refresh":"refresh-antigravity","access":"access-antigravity","expires":4102444800000}"#,
        )
        .expect("parse");

        assert_eq!(credential.refresh, "refresh-antigravity");
        assert_eq!(credential.access.as_deref(), Some("access-antigravity"));
    }

    #[test]
    fn rejects_non_antigravity_credentials() {
        let error = parse_gemini_code_assist_credential(
            r#"{"type":"unsupported-oauth-v1","refresh":"refresh"}"#,
        )
        .expect_err("non-Antigravity credential must be rejected");

        assert!(error.to_string().contains("reconnect"));
    }

    #[test]
    fn maps_antigravity_metadata_platforms() {
        assert_eq!(
            antigravity_metadata_platform_for_target("macos", "aarch64"),
            "DARWIN_ARM64"
        );
        assert_eq!(
            antigravity_metadata_platform_for_target("macos", "x86_64"),
            "DARWIN_AMD64"
        );
        assert_eq!(
            antigravity_metadata_platform_for_target("linux", "aarch64"),
            "LINUX_ARM64"
        );
        assert_eq!(
            antigravity_metadata_platform_for_target("linux", "x86_64"),
            "LINUX_AMD64"
        );
        assert_eq!(
            antigravity_metadata_platform_for_target("windows", "x86_64"),
            "WINDOWS_AMD64"
        );
        assert_eq!(
            antigravity_metadata_platform_for_target("freebsd", "aarch64"),
            "PLATFORM_UNSPECIFIED"
        );
    }

    #[test]
    fn builds_antigravity_generate_content_request() {
        let request = build_code_assist_request(
            &json!({
                "contents": [{"parts": [{"text": "hello"}]}],
                "generationConfig": {"temperature": 0.2},
                "systemInstruction": {"parts": [{"text": "be concise"}]}
            }),
            "gemini-2.5-flash",
            Some("project-1"),
            Some("session-1"),
        );

        assert_eq!(request["model"], "gemini-2.5-flash");
        assert_eq!(request["project"], "project-1");
        assert_eq!(request["requestType"], "agent");
        assert_eq!(request["userAgent"], "antigravity");
        assert!(request["requestId"].as_str().unwrap().starts_with("agent-"));
        assert_eq!(request["request"]["contents"][0]["role"], "user");
        assert_eq!(request["request"]["generationConfig"]["temperature"], 0.2);
        assert_eq!(request["request"]["sessionId"], "session-1");
    }

    #[test]
    fn preserves_existing_code_assist_content_roles() {
        let request = build_code_assist_request(
            &json!({
                "contents": [
                    {"role": "model", "parts": [{"text": "previous"}]},
                    {"parts": [{"text": "next"}]}
                ]
            }),
            "gemini-2.5-flash",
            None,
            None,
        );

        assert_eq!(request["request"]["contents"][0]["role"], "model");
        assert_eq!(request["request"]["contents"][1]["role"], "user");
    }

    #[test]
    fn unwraps_code_assist_generate_content_response() {
        let bytes = serde_json::to_vec(&json!({
            "traceId": "trace-1",
            "response": {
                "candidates": [{"content": {"parts": [{"text": "connected"}]}}],
                "usageMetadata": {"promptTokenCount": 1, "candidatesTokenCount": 2}
            }
        }))
        .unwrap();

        let unwrapped: Value =
            serde_json::from_slice(&unwrap_code_assist_response_bytes(&bytes).unwrap()).unwrap();
        assert_eq!(unwrapped["responseId"], "trace-1");
        assert_eq!(
            unwrapped["candidates"][0]["content"]["parts"][0]["text"],
            "connected"
        );
    }
}
