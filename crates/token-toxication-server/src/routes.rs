use std::time::Instant;

use aioduct::{
    RequestBuilderSend,
    runtime::{ConnectorSend, RuntimePoll},
};
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri, header},
    middleware,
    response::Response,
    routing::{get, patch, post},
};
use chrono::Utc;
use futures_util::stream;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{extract_api_key, generate_secret, login, logout, me, require_admin},
    error::AppError,
    models::{
        AnthropicModel, AnthropicModelListResponse, ApiKeyListResponse, ApiKeyRecord,
        ApiKeyResponse, CreateApiKeyRequest, CreateApiKeyResponse, CreateProviderAccountRequest,
        Dashboard, HealthResponse, MetricsResponse, OpenAiModel, OpenAiModelListResponse,
        ProviderAccountListResponse, ProviderAccountResponse, RequestLog, RequestLogListResponse,
        UpdateApiKeyRequest, UpdateProviderAccountRequest,
    },
};

const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

pub fn admin_routes(state: AppState) -> Router<AppState> {
    let protected = Router::new()
        .route("/auth/me", get(me))
        .route("/auth/logout", post(logout))
        .route("/dashboard", get(admin_dashboard))
        .route("/api-keys", get(list_api_keys).post(create_api_key))
        .route(
            "/api-keys/{id}",
            patch(update_api_key).delete(delete_api_key),
        )
        .route(
            "/provider-accounts",
            get(list_provider_accounts).post(create_provider_account),
        )
        .route(
            "/provider-accounts/{id}",
            patch(update_provider_account).delete(delete_provider_account),
        )
        .route("/request-logs", get(list_request_logs))
        .route_layer(middleware::from_fn_with_state(state, require_admin));

    Router::new()
        .route("/auth/login", post(login))
        .merge(protected)
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "token-toxication".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: (Utc::now() - state.started_at).num_seconds(),
        timestamp: Utc::now(),
    })
}

pub async fn metrics(State(state): State<AppState>) -> Result<Json<MetricsResponse>, AppError> {
    let dashboard = state.db.dashboard().await?;
    Ok(Json(MetricsResponse {
        active_api_keys: dashboard.active_api_keys,
        total_api_keys: dashboard.total_api_keys,
        healthy_accounts: dashboard.healthy_accounts,
        total_accounts: dashboard.total_accounts,
        usage: dashboard.usage,
        timestamp: Utc::now(),
    }))
}

pub async fn relay_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Result<Response, AppError> {
    relay_json_endpoint(state, headers, uri, body, WireApi::AnthropicMessages).await
}

pub async fn relay_openai_responses(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Result<Response, AppError> {
    relay_json_endpoint(state, headers, uri, body, WireApi::OpenAiResponses).await
}

pub async fn relay_openai_chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Result<Response, AppError> {
    relay_json_endpoint(state, headers, uri, body, WireApi::OpenAiChat).await
}

pub async fn list_openai_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<OpenAiModelListResponse>, AppError> {
    authenticate_relay_api_key(&state, &headers, uri.query()).await?;
    Ok(Json(OpenAiModelListResponse {
        object: "list".to_string(),
        data: openai_models(&state).await?,
    }))
}

pub async fn get_openai_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Path(model): Path<String>,
) -> Result<Json<OpenAiModel>, AppError> {
    authenticate_relay_api_key(&state, &headers, uri.query()).await?;
    let model = openai_models(&state)
        .await?
        .into_iter()
        .find(|entry| entry.id == model)
        .ok_or_else(|| AppError::NotFound("model not found".into()))?;
    Ok(Json(model))
}

pub async fn list_anthropic_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<AnthropicModelListResponse>, AppError> {
    authenticate_relay_api_key(&state, &headers, uri.query()).await?;
    let data = anthropic_models(&state).await?;
    Ok(Json(AnthropicModelListResponse {
        first_id: data.first().map(|model| model.id.clone()),
        last_id: data.last().map(|model| model.id.clone()),
        data,
        has_more: false,
    }))
}

pub async fn get_anthropic_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Path(model): Path<String>,
) -> Result<Json<AnthropicModel>, AppError> {
    authenticate_relay_api_key(&state, &headers, uri.query()).await?;
    let model = anthropic_models(&state)
        .await?
        .into_iter()
        .find(|entry| entry.id == model)
        .ok_or_else(|| AppError::NotFound("model not found".into()))?;
    Ok(Json(model))
}

#[derive(Debug, Clone, Copy)]
enum WireApi {
    AnthropicMessages,
    OpenAiChat,
    OpenAiResponses,
}

impl WireApi {
    fn account_value(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic-messages",
            Self::OpenAiChat => "openai-chat",
            Self::OpenAiResponses => "openai-responses",
        }
    }

    fn upstream_path(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "/v1/messages",
            Self::OpenAiChat => "/chat/completions",
            Self::OpenAiResponses => "/v1/responses",
        }
    }

    fn public_path(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "/anthropic/v1/messages",
            Self::OpenAiChat => "/openai/v1/chat/completions",
            Self::OpenAiResponses => "/openai/v1/responses",
        }
    }

    fn validate(self, value: &Value) -> Result<(), AppError> {
        match self {
            Self::AnthropicMessages | Self::OpenAiChat => validate_messages_request(value),
            Self::OpenAiResponses => validate_responses_request(value),
        }
    }
}

async fn relay_json_endpoint(
    state: AppState,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
    wire_api: WireApi,
) -> Result<Response, AppError> {
    let started = Instant::now();
    let api_key = authenticate_relay_api_key(&state, &headers, uri.query()).await?;

    let request_json: Value = serde_json::from_slice(&body)
        .map_err(|error| AppError::BadRequest(format!("invalid JSON body: {error}")))?;
    wire_api.validate(&request_json)?;
    let model = request_json
        .get("model")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let api_key_id = api_key.view.id;

    let account = state
        .db
        .select_provider_account_for_wire(wire_api.account_value(), model.as_deref())
        .await?
        .ok_or_else(|| AppError::Forbidden("no active provider account is available".into()))?;

    let upstream_url = upstream_url(&account.account.base_url, wire_api.upstream_path());
    let request = match state.http.post(&upstream_url) {
        Ok(request) => request
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(body),
        Err(error) => {
            record_upstream_failure(
                &state,
                &account.account.id,
                api_key_id.clone(),
                wire_api,
                model.clone(),
                started,
                error.to_string(),
            )
            .await?;
            return Err(AppError::Upstream(error));
        }
    };
    let request = apply_protocol_headers(request, wire_api, &headers);
    let request = match apply_provider_auth(request, &account.account.auth_mode, &account.api_key) {
        Ok(request) => request,
        Err(error) => {
            record_upstream_failure(
                &state,
                &account.account.id,
                api_key_id.clone(),
                wire_api,
                model.clone(),
                started,
                error.to_string(),
            )
            .await?;
            return Err(error);
        }
    };

    let upstream = request.send().await;
    match upstream {
        Ok(response) => {
            let status = response.status();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .cloned()
                .unwrap_or_else(|| HeaderValue::from_static("application/json"));
            let is_stream = request_json
                .get("stream")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            if is_stream {
                state
                    .db
                    .mark_provider_result(
                        &account.account.id,
                        provider_status_for(status),
                        error_for_status(status).as_deref(),
                    )
                    .await?;
                state
                    .db
                    .insert_request_log(RequestLog {
                        id: Uuid::new_v4().to_string(),
                        api_key_id: api_key_id.clone(),
                        provider_account_id: Some(account.account.id.clone()),
                        method: "POST".to_string(),
                        path: wire_api.public_path().to_string(),
                        model: model.clone(),
                        status_code: status.as_u16(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost_usd: 0.0,
                        created_at: Utc::now(),
                        error: error_for_status(status),
                    })
                    .await?;
                let body_stream =
                    stream::unfold(response.into_bytes_stream(), |mut body| async move {
                        body.next().await.map(|chunk| {
                            let chunk = chunk.map_err(std::io::Error::other);
                            (chunk, body)
                        })
                    });
                let body = Body::from_stream(body_stream);
                let mut relay = Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(body)
                    .map_err(|error| AppError::Internal(error.to_string()))?;
                relay.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, no-transform"),
                );
                Ok(relay)
            } else {
                let bytes = response.bytes().await?;
                let usage = parse_usage(&bytes);
                state
                    .db
                    .mark_provider_result(
                        &account.account.id,
                        provider_status_for(status),
                        error_for_status(status).as_deref(),
                    )
                    .await?;
                state
                    .db
                    .insert_request_log(RequestLog {
                        id: Uuid::new_v4().to_string(),
                        api_key_id: api_key_id.clone(),
                        provider_account_id: Some(account.account.id.clone()),
                        method: "POST".to_string(),
                        path: wire_api.public_path().to_string(),
                        model: model.clone(),
                        status_code: status.as_u16(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: usage.0,
                        output_tokens: usage.1,
                        cost_usd: 0.0,
                        created_at: Utc::now(),
                        error: error_for_status(status),
                    })
                    .await?;
                let mut relay = Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(bytes))
                    .map_err(|error| AppError::Internal(error.to_string()))?;
                relay
                    .headers_mut()
                    .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                Ok(relay)
            }
        }
        Err(error) => {
            record_upstream_failure(
                &state,
                &account.account.id,
                api_key_id,
                wire_api,
                model,
                started,
                error.to_string(),
            )
            .await?;
            Err(AppError::Upstream(error.into()))
        }
    }
}

async fn record_upstream_failure(
    state: &AppState,
    account_id: &str,
    api_key_id: String,
    wire_api: WireApi,
    model: Option<String>,
    started: Instant,
    error: String,
) -> Result<(), AppError> {
    state
        .db
        .mark_provider_result(account_id, "degraded", Some(&error))
        .await?;
    state
        .db
        .insert_request_log(RequestLog {
            id: Uuid::new_v4().to_string(),
            api_key_id,
            provider_account_id: Some(account_id.to_string()),
            method: "POST".to_string(),
            path: wire_api.public_path().to_string(),
            model,
            status_code: StatusCode::BAD_GATEWAY.as_u16(),
            latency_ms: started.elapsed().as_millis() as u64,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            created_at: Utc::now(),
            error: Some(error),
        })
        .await?;
    Ok(())
}

pub async fn admin_dashboard(State(state): State<AppState>) -> Result<Json<Dashboard>, AppError> {
    Ok(Json(state.db.dashboard().await?))
}

pub async fn list_api_keys(
    State(state): State<AppState>,
) -> Result<Json<ApiKeyListResponse>, AppError> {
    Ok(Json(ApiKeyListResponse {
        data: state.db.list_api_keys().await?,
    }))
}

pub async fn create_api_key(
    State(state): State<AppState>,
    Json(input): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("API key name is required".into()));
    }
    let secret = generate_secret(&state.config.api_key_prefix);
    let key = state.db.create_api_key(input, &secret).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse { key, secret }),
    ))
}

pub async fn update_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    let key = state
        .db
        .update_api_key(&id, input)
        .await
        .map_err(map_not_found)?;
    Ok(Json(ApiKeyResponse { data: key }))
}

pub async fn delete_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if !state.db.delete_api_key(&id).await? {
        return Err(AppError::NotFound("API key not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_provider_accounts(
    State(state): State<AppState>,
) -> Result<Json<ProviderAccountListResponse>, AppError> {
    Ok(Json(ProviderAccountListResponse {
        data: state.db.list_provider_accounts().await?,
    }))
}

pub async fn create_provider_account(
    State(state): State<AppState>,
    Json(input): Json<CreateProviderAccountRequest>,
) -> Result<(StatusCode, Json<ProviderAccountResponse>), AppError> {
    if input.name.trim().is_empty()
        || input.base_url.trim().is_empty()
        || input.api_key.trim().is_empty()
    {
        return Err(AppError::BadRequest(
            "provider account name, base URL, and API key are required".into(),
        ));
    }
    let account = state.db.create_provider_account(input).await?;
    Ok((
        StatusCode::CREATED,
        Json(ProviderAccountResponse { data: account }),
    ))
}

pub async fn update_provider_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProviderAccountRequest>,
) -> Result<Json<ProviderAccountResponse>, AppError> {
    if input
        .base_url
        .as_deref()
        .is_some_and(|base_url| base_url.trim().is_empty())
    {
        return Err(AppError::BadRequest(
            "provider account base URL cannot be empty".into(),
        ));
    }
    let account = state
        .db
        .update_provider_account(&id, input)
        .await
        .map_err(map_not_found)?;
    Ok(Json(ProviderAccountResponse { data: account }))
}

pub async fn delete_provider_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    state.db.delete_provider_account(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsQuery {
    limit: Option<u32>,
}

pub async fn list_request_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<RequestLogListResponse>, AppError> {
    Ok(Json(RequestLogListResponse {
        data: state
            .db
            .list_request_logs(query.limit.unwrap_or(50).min(200))
            .await?,
    }))
}

fn validate_messages_request(value: &Value) -> Result<(), AppError> {
    if !value.is_object() {
        return Err(AppError::BadRequest(
            "request body must be an object".into(),
        ));
    }
    match value.get("messages").and_then(Value::as_array) {
        Some(messages) if !messages.is_empty() => Ok(()),
        _ => Err(AppError::BadRequest(
            "missing or invalid messages array".into(),
        )),
    }
}

fn validate_responses_request(value: &Value) -> Result<(), AppError> {
    if !value.is_object() {
        return Err(AppError::BadRequest(
            "request body must be an object".into(),
        ));
    }
    if value.get("input").is_some()
        || value.get("messages").is_some()
        || value.get("prompt").is_some()
    {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "missing Responses API input field".into(),
        ))
    }
}

fn apply_protocol_headers<'a, R, C>(
    mut request: RequestBuilderSend<'a, R, C>,
    wire_api: WireApi,
    headers: &HeaderMap,
) -> RequestBuilderSend<'a, R, C>
where
    R: RuntimePoll,
    C: ConnectorSend,
{
    if matches!(wire_api, WireApi::AnthropicMessages) {
        request = request.header(
            HeaderName::from_static("anthropic-version"),
            forwarded_anthropic_version(headers),
        );
        if let Some(beta) = headers.get("anthropic-beta") {
            request = request.header(HeaderName::from_static("anthropic-beta"), beta.clone());
        }
    }
    request
}

fn apply_provider_auth<'a, R, C>(
    request: RequestBuilderSend<'a, R, C>,
    auth_mode: &str,
    api_key: &str,
) -> Result<RequestBuilderSend<'a, R, C>, AppError>
where
    R: RuntimePoll,
    C: ConnectorSend,
{
    if auth_mode == "bearer" {
        Ok(request.bearer_auth(api_key))
    } else {
        let api_key = HeaderValue::from_str(api_key)
            .map_err(|error| AppError::Internal(format!("invalid provider API key: {error}")))?;
        Ok(request.header(HeaderName::from_static("x-api-key"), api_key))
    }
}

fn upstream_url(base_url: &str, path: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if let Some(rest) = path.strip_prefix("v1/")
        && base_url.ends_with("/v1")
    {
        format!("{base_url}/{rest}")
    } else {
        format!("{base_url}/{path}")
    }
}

fn forwarded_anthropic_version(headers: &HeaderMap) -> HeaderValue {
    headers
        .get("anthropic-version")
        .filter(|value| {
            value
                .to_str()
                .is_ok_and(|version| !version.trim().is_empty())
        })
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static(DEFAULT_ANTHROPIC_VERSION))
}

fn provider_status_for(status: StatusCode) -> &'static str {
    if status.is_success() {
        "healthy"
    } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        "blocked"
    } else {
        "degraded"
    }
}

fn error_for_status(status: StatusCode) -> Option<String> {
    (!status.is_success()).then(|| format!("upstream returned {}", status.as_u16()))
}

fn parse_usage(bytes: &[u8]) -> (u64, u64) {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return (0, 0);
    };
    let Some(usage) = value.get("usage") else {
        return (0, 0);
    };
    let input = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    (input, output)
}

async fn authenticate_relay_api_key(
    state: &AppState,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<ApiKeyRecord, AppError> {
    let api_key = extract_api_key(headers, query)
        .ok_or_else(|| AppError::Unauthorized("missing API key".into()))?;
    if !api_key.starts_with(&state.config.api_key_prefix) {
        return Err(AppError::Unauthorized("invalid API key prefix".into()));
    }
    state
        .db
        .validate_api_key(&api_key)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid or inactive API key".into()))
}

async fn openai_models(state: &AppState) -> Result<Vec<OpenAiModel>, AppError> {
    let models = state
        .db
        .list_model_catalog(&["openai-chat", "openai-responses"])
        .await?;
    Ok(models
        .into_iter()
        .map(|model| OpenAiModel {
            id: model.id,
            object: "model".to_string(),
            created: model.created_at.timestamp(),
            owned_by: model.provider,
        })
        .collect())
}

async fn anthropic_models(state: &AppState) -> Result<Vec<AnthropicModel>, AppError> {
    let models = state.db.list_model_catalog(&["anthropic-messages"]).await?;
    Ok(models
        .into_iter()
        .map(|model| AnthropicModel {
            r#type: "model".to_string(),
            display_name: model.id.clone(),
            id: model.id,
            created_at: model.created_at,
        })
        .collect())
}

fn map_not_found(error: rusqlite::Error) -> AppError {
    match error {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("record not found".into()),
        error => AppError::Database(error),
    }
}
