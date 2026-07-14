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
    response::{Html, Response},
    routing::{get, patch, post},
};
use chrono::Utc;
use futures_util::stream;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppState,
    antigravity_oauth::{
        apply_antigravity_headers, begin_antigravity_oauth, complete_antigravity_oauth,
        gemini_account_models, gemini_account_quota,
    },
    auth::{extract_api_key, generate_secret, login, logout, me, require_admin},
    codex_subscription::{
        CodexSubscriptionAuthorization, codex_account_quota, codex_subscription_authorization,
        is_codex_subscription_auth,
    },
    error::AppError,
    gemini_code_assist::{
        build_code_assist_request, gemini_code_assist_authorization, gemini_code_assist_endpoint,
        is_antigravity_oauth_auth, unwrap_code_assist_response_bytes, unwrap_code_assist_sse_data,
    },
    models::{
        AnthropicModel, AnthropicModelListResponse, AntigravityOAuthStartRequest,
        AntigravityOAuthStartResponse, ApiKeyListResponse, ApiKeyRecord, ApiKeyResponse,
        CodexAccountQuotaResponse, CreateApiKeyRequest, CreateApiKeyResponse,
        CreateModelCatalogEntryRequest, CreateProviderAccountRequest,
        CreateProviderModelRouteRequest, Dashboard, GeminiAccountModelsResponse,
        GeminiAccountQuotaResponse, GeminiModel, GeminiModelListResponse, HealthResponse,
        MetricsResponse, ModelCatalogEntryResponse, ModelCatalogListResponse, OpenAiModel,
        OpenAiModelListResponse, ProviderAccountListResponse, ProviderAccountResponse,
        ProviderModelRouteListResponse, ProviderModelRouteResponse, ProviderPresetListResponse,
        RequestLog, RequestLogListResponse, RequestSummary, UpdateApiKeyRequest,
        UpdateModelCatalogEntryRequest, UpdateProviderAccountRequest,
        UpdateProviderModelRouteRequest,
    },
    provider_catalog::provider_presets,
    routing::{RouteFailure, classify_response_failure, classify_transport_failure},
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
        .route(
            "/provider-accounts/{id}/gemini/models",
            get(get_gemini_account_models),
        )
        .route(
            "/provider-accounts/{id}/gemini/quota",
            get(get_gemini_account_quota),
        )
        .route(
            "/provider-accounts/{id}/codex/quota",
            get(get_codex_account_quota),
        )
        .route("/oauth/antigravity/start", post(start_antigravity_oauth))
        .route("/provider-presets", get(list_provider_presets))
        .route(
            "/model-catalog",
            get(list_model_catalog).post(create_model_catalog_entry),
        )
        .route("/model-catalog/{id}", patch(update_model_catalog_entry))
        .route(
            "/provider-model-routes",
            get(list_provider_model_routes).post(create_provider_model_route),
        )
        .route(
            "/provider-model-routes/{id}",
            patch(update_provider_model_route).delete(delete_provider_model_route),
        )
        .route("/request-logs", get(list_request_logs))
        .route_layer(middleware::from_fn_with_state(state, require_admin));

    Router::new()
        .route("/auth/login", post(login))
        .route(
            "/oauth/antigravity/callback",
            get(antigravity_oauth_callback),
        )
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

pub async fn relay_gemini_generate_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Path(operation): Path<String>,
    body: Bytes,
) -> Result<Response, AppError> {
    let (model, method) = parse_gemini_model_operation(&operation)?;
    relay_gemini_endpoint(state, headers, uri, body, model, method).await
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

pub async fn list_gemini_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<GeminiModelListResponse>, AppError> {
    authenticate_relay_api_key(&state, &headers, uri.query()).await?;
    Ok(Json(GeminiModelListResponse {
        models: gemini_models(&state).await?,
    }))
}

pub async fn get_gemini_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Path(model): Path<String>,
) -> Result<Json<GeminiModel>, AppError> {
    authenticate_relay_api_key(&state, &headers, uri.query()).await?;
    let model_name = format!("models/{model}");
    let model = gemini_models(&state)
        .await?
        .into_iter()
        .find(|entry| entry.name == model_name)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireApi {
    AnthropicMessages,
    OpenAiChat,
    OpenAiResponses,
    GeminiGenerateContent,
}

impl WireApi {
    fn account_value(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic-messages",
            Self::OpenAiChat => "openai-chat",
            Self::OpenAiResponses => "openai-responses",
            Self::GeminiGenerateContent => "gemini-generate-content",
        }
    }

    fn upstream_path(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "/v1/messages",
            Self::OpenAiChat => "/chat/completions",
            Self::OpenAiResponses => "/v1/responses",
            Self::GeminiGenerateContent => "/v1beta/models",
        }
    }

    fn public_path(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "/anthropic/v1/messages",
            Self::OpenAiChat => "/openai/v1/chat/completions",
            Self::OpenAiResponses => "/openai/v1/responses",
            Self::GeminiGenerateContent => "/gemini/v1beta/models",
        }
    }

    fn validate(self, value: &Value) -> Result<(), AppError> {
        match self {
            Self::AnthropicMessages | Self::OpenAiChat => validate_messages_request(value),
            Self::OpenAiResponses => validate_responses_request(value),
            Self::GeminiGenerateContent => validate_gemini_generate_content_request(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeminiMethod {
    GenerateContent,
    StreamGenerateContent,
}

impl GeminiMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::GenerateContent => "generateContent",
            Self::StreamGenerateContent => "streamGenerateContent",
        }
    }

    fn is_stream(self) -> bool {
        matches!(self, Self::StreamGenerateContent)
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

    let mut request_json: Value = serde_json::from_slice(&body)
        .map_err(|error| AppError::BadRequest(format!("invalid JSON body: {error}")))?;
    wire_api.validate(&request_json)?;
    let model = request_json
        .get("model")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    if model.is_none() {
        return Err(AppError::BadRequest("request model is required".into()));
    }
    let api_key_id = api_key.view.id;

    let selection = state
        .db
        .select_provider_account_for_wire(wire_api.account_value(), model.as_deref())
        .await?
        .ok_or_else(|| AppError::Forbidden("no active provider account is available".into()))?;
    let route_id = selection.route_id.clone();
    let public_model_id = selection.public_model_id.clone();
    let upstream_model_id = selection.upstream_model_id.clone();
    request_json["model"] = Value::String(upstream_model_id.clone());
    let stripped_params = strip_upstream_params(
        &mut request_json,
        &selection.strip_params,
        wire_api,
        &selection.account.account.auth_mode,
    );
    let body = serde_json::to_vec(&request_json)
        .map_err(|error| AppError::Internal(format!("serialize upstream request: {error}")))?;
    let request_summary = build_request_summary(&request_json, body.len() as u64, stripped_params);
    let account = selection.account;
    let base_upstream_url = upstream_url(&account.account.base_url, wire_api.upstream_path());
    let codex_auth = if is_codex_subscription_auth(&account.account.auth_mode) {
        if wire_api != WireApi::OpenAiResponses {
            return Err(AppError::BadRequest(
                "Codex subscription providers only support openai-responses routes".into(),
            ));
        }
        match codex_subscription_authorization(&state.db, &state.http, &account).await {
            Ok(auth) => Some(auth),
            Err(error) => {
                let status = error.status();
                let failure = if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                {
                    RouteFailure {
                        provider_status: Some("blocked"),
                        route_status: "degraded",
                        cooldown_until: None,
                        error: error.to_string(),
                        status_code: Some(status.as_u16()),
                    }
                } else {
                    classify_transport_failure(error.to_string(), Utc::now())
                };
                record_upstream_failure(
                    &state,
                    UpstreamFailureLog {
                        account_id: &account.account.id,
                        route_id: &route_id,
                        api_key_id: api_key_id.clone(),
                        path: wire_api.public_path().to_string(),
                        model: Some(public_model_id.clone()),
                        upstream_model: Some(upstream_model_id.clone()),
                        started,
                        upstream_url: Some(sanitize_upstream_url(&base_upstream_url)),
                        request_summary: Some(request_summary.clone()),
                    },
                    failure,
                )
                .await?;
                return Err(error);
            }
        }
    } else {
        None
    };

    let upstream_url = codex_auth
        .as_ref()
        .map(|auth| auth.endpoint.clone())
        .unwrap_or(base_upstream_url);
    let sanitized_upstream_url = Some(sanitize_upstream_url(&upstream_url));
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
                UpstreamFailureLog {
                    account_id: &account.account.id,
                    route_id: &route_id,
                    api_key_id: api_key_id.clone(),
                    path: wire_api.public_path().to_string(),
                    model: Some(public_model_id.clone()),
                    upstream_model: Some(upstream_model_id.clone()),
                    started,
                    upstream_url: sanitized_upstream_url.clone(),
                    request_summary: Some(request_summary.clone()),
                },
                classify_transport_failure(error.to_string(), Utc::now()),
            )
            .await?;
            return Err(AppError::Upstream(error));
        }
    };
    let request = apply_protocol_headers(request, wire_api, &headers);
    let request = match codex_auth.as_ref() {
        Some(auth) => apply_codex_subscription_auth(request, auth),
        None => apply_provider_auth(request, &account.account.auth_mode, &account.api_key),
    };
    let request = match request {
        Ok(request) => request,
        Err(error) => {
            let failure = classify_transport_failure(error.to_string(), Utc::now());
            record_upstream_failure(
                &state,
                UpstreamFailureLog {
                    account_id: &account.account.id,
                    route_id: &route_id,
                    api_key_id: api_key_id.clone(),
                    path: wire_api.public_path().to_string(),
                    model: Some(public_model_id.clone()),
                    upstream_model: Some(upstream_model_id.clone()),
                    started,
                    upstream_url: sanitized_upstream_url.clone(),
                    request_summary: Some(request_summary.clone()),
                },
                failure,
            )
            .await?;
            return Err(error);
        }
    };

    let upstream = request.send().await;
    match upstream {
        Ok(response) => {
            let status = response.status();
            let response_headers = response.headers().clone();
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
                let error = record_upstream_response_result(
                    &state,
                    &account.account.id,
                    &route_id,
                    &account.account.provider,
                    status,
                    &response_headers,
                    b"",
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
                        model: Some(public_model_id.clone()),
                        upstream_model: Some(upstream_model_id.clone()),
                        upstream_url: sanitized_upstream_url.clone(),
                        request_summary: Some(request_summary.clone()),
                        status_code: status.as_u16(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost_usd: 0.0,
                        created_at: Utc::now(),
                        error,
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
                let error = record_upstream_response_result(
                    &state,
                    &account.account.id,
                    &route_id,
                    &account.account.provider,
                    status,
                    &response_headers,
                    &bytes,
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
                        model: Some(public_model_id.clone()),
                        upstream_model: Some(upstream_model_id.clone()),
                        upstream_url: sanitized_upstream_url.clone(),
                        request_summary: Some(request_summary.clone()),
                        status_code: status.as_u16(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: usage.0,
                        output_tokens: usage.1,
                        cost_usd: 0.0,
                        created_at: Utc::now(),
                        error,
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
                UpstreamFailureLog {
                    account_id: &account.account.id,
                    route_id: &route_id,
                    api_key_id,
                    path: wire_api.public_path().to_string(),
                    model: Some(public_model_id),
                    upstream_model: Some(upstream_model_id),
                    started,
                    upstream_url: sanitized_upstream_url,
                    request_summary: Some(request_summary),
                },
                classify_transport_failure(error.to_string(), Utc::now()),
            )
            .await?;
            Err(AppError::Upstream(error.into()))
        }
    }
}

async fn relay_gemini_endpoint(
    state: AppState,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
    model: String,
    method: GeminiMethod,
) -> Result<Response, AppError> {
    let started = Instant::now();
    let api_key = authenticate_relay_api_key(&state, &headers, uri.query()).await?;

    let mut request_json: Value = serde_json::from_slice(&body)
        .map_err(|error| AppError::BadRequest(format!("invalid JSON body: {error}")))?;
    validate_gemini_generate_content_request(&request_json)?;
    let api_key_id = api_key.view.id;

    let selection = state
        .db
        .select_provider_account_for_wire(
            WireApi::GeminiGenerateContent.account_value(),
            Some(model.as_str()),
        )
        .await?
        .ok_or_else(|| AppError::Forbidden("no active provider account is available".into()))?;
    let route_id = selection.route_id.clone();
    let public_model_id = selection.public_model_id.clone();
    let upstream_model_id = selection.upstream_model_id.clone();
    let public_path = gemini_public_path(&public_model_id, method);
    let stripped_params = strip_top_level_params(&mut request_json, &selection.strip_params);
    let account = selection.account;
    if !is_antigravity_oauth_auth(&account.account.auth_mode) {
        return Err(AppError::BadRequest(
            "Gemini native providers require Antigravity OAuth credentials".into(),
        ));
    }
    let fallback_upstream_url = gemini_code_assist_upstream_url(
        &gemini_code_assist_endpoint(&account.account.base_url),
        method,
        uri.query(),
    );

    let authorization =
        match gemini_code_assist_authorization(&state.db, &state.gemini_http, &account).await {
            Ok(authorization) => authorization,
            Err(error) => {
                let status = error.status();
                let failure = if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                {
                    RouteFailure {
                        provider_status: Some("blocked"),
                        route_status: "degraded",
                        cooldown_until: None,
                        error: error.to_string(),
                        status_code: Some(status.as_u16()),
                    }
                } else {
                    classify_transport_failure(error.to_string(), Utc::now())
                };
                record_upstream_failure(
                    &state,
                    UpstreamFailureLog {
                        account_id: &account.account.id,
                        route_id: &route_id,
                        api_key_id: api_key_id.clone(),
                        path: public_path.clone(),
                        model: Some(public_model_id.clone()),
                        upstream_model: Some(upstream_model_id.clone()),
                        started,
                        upstream_url: Some(sanitize_upstream_url(&fallback_upstream_url)),
                        request_summary: None,
                    },
                    failure,
                )
                .await?;
                return Err(error);
            }
        };

    let session_id = request_json
        .get("sessionId")
        .or_else(|| request_json.get("session_id"))
        .and_then(Value::as_str);
    let code_assist_json = build_code_assist_request(
        &request_json,
        &upstream_model_id,
        authorization.project.as_deref(),
        session_id,
    );
    let body = serde_json::to_vec(&code_assist_json)
        .map_err(|error| AppError::Internal(format!("serialize upstream request: {error}")))?;
    let mut request_summary =
        build_request_summary(&request_json, body.len() as u64, stripped_params);
    request_summary.stream = method.is_stream();
    let upstream_url =
        gemini_code_assist_upstream_url(&authorization.endpoint, method, uri.query());
    let sanitized_upstream_url = Some(sanitize_upstream_url(&upstream_url));

    let request = match state.gemini_http.post(&upstream_url) {
        Ok(request) => request
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(body),
        Err(error) => {
            record_upstream_failure(
                &state,
                UpstreamFailureLog {
                    account_id: &account.account.id,
                    route_id: &route_id,
                    api_key_id: api_key_id.clone(),
                    path: public_path.clone(),
                    model: Some(public_model_id.clone()),
                    upstream_model: Some(upstream_model_id.clone()),
                    started,
                    upstream_url: sanitized_upstream_url.clone(),
                    request_summary: Some(request_summary.clone()),
                },
                classify_transport_failure(error.to_string(), Utc::now()),
            )
            .await?;
            return Err(AppError::Upstream(error));
        }
    };
    let request = request.bearer_auth(&authorization.access_token);
    let request = apply_antigravity_headers(request)?;

    let upstream = request.send().await;
    match upstream {
        Ok(response) => {
            let status = response.status();
            let response_headers = response.headers().clone();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .cloned()
                .unwrap_or_else(|| HeaderValue::from_static("application/json"));

            if method.is_stream() {
                let error = record_upstream_response_result(
                    &state,
                    &account.account.id,
                    &route_id,
                    &account.account.provider,
                    status,
                    &response_headers,
                    b"",
                )
                .await?;
                state
                    .db
                    .insert_request_log(RequestLog {
                        id: Uuid::new_v4().to_string(),
                        api_key_id: api_key_id.clone(),
                        provider_account_id: Some(account.account.id.clone()),
                        method: "POST".to_string(),
                        path: public_path.clone(),
                        model: Some(public_model_id.clone()),
                        upstream_model: Some(upstream_model_id.clone()),
                        upstream_url: sanitized_upstream_url.clone(),
                        request_summary: Some(request_summary.clone()),
                        status_code: status.as_u16(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost_usd: 0.0,
                        created_at: Utc::now(),
                        error,
                    })
                    .await?;
                let body_stream = stream::unfold(
                    (
                        response.into_bytes_stream(),
                        String::new(),
                        status.is_success(),
                    ),
                    |(mut body, mut buffer, unwrap_events)| async move {
                        loop {
                            if unwrap_events && let Some((event, rest)) = take_sse_event(&buffer) {
                                buffer = rest;
                                let chunk = transform_code_assist_sse_event(&event);
                                return Some((
                                    Ok(Bytes::from(chunk)),
                                    (body, buffer, unwrap_events),
                                ));
                            }

                            match body.next().await {
                                Some(Ok(chunk)) if unwrap_events => {
                                    buffer.push_str(&String::from_utf8_lossy(&chunk));
                                }
                                Some(Ok(chunk)) => {
                                    return Some((Ok(chunk), (body, buffer, unwrap_events)));
                                }
                                Some(Err(error)) => {
                                    return Some((
                                        Err(std::io::Error::other(error)),
                                        (body, buffer, unwrap_events),
                                    ));
                                }
                                None if unwrap_events && !buffer.is_empty() => {
                                    let chunk = transform_code_assist_sse_event(&buffer);
                                    buffer.clear();
                                    return Some((
                                        Ok(Bytes::from(chunk)),
                                        (body, buffer, unwrap_events),
                                    ));
                                }
                                None => return None,
                            }
                        }
                    },
                );
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
                let upstream_bytes = response.bytes().await?;
                let relay_bytes = if status.is_success() {
                    unwrap_code_assist_response_bytes(&upstream_bytes)?
                } else {
                    upstream_bytes.to_vec()
                };
                let usage = parse_usage(&relay_bytes);
                let error = record_upstream_response_result(
                    &state,
                    &account.account.id,
                    &route_id,
                    &account.account.provider,
                    status,
                    &response_headers,
                    if status.is_success() {
                        &relay_bytes
                    } else {
                        &upstream_bytes
                    },
                )
                .await?;
                state
                    .db
                    .insert_request_log(RequestLog {
                        id: Uuid::new_v4().to_string(),
                        api_key_id: api_key_id.clone(),
                        provider_account_id: Some(account.account.id.clone()),
                        method: "POST".to_string(),
                        path: public_path,
                        model: Some(public_model_id.clone()),
                        upstream_model: Some(upstream_model_id.clone()),
                        upstream_url: sanitized_upstream_url.clone(),
                        request_summary: Some(request_summary.clone()),
                        status_code: status.as_u16(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: usage.0,
                        output_tokens: usage.1,
                        cost_usd: 0.0,
                        created_at: Utc::now(),
                        error,
                    })
                    .await?;
                let mut relay = Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(relay_bytes))
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
                UpstreamFailureLog {
                    account_id: &account.account.id,
                    route_id: &route_id,
                    api_key_id,
                    path: public_path,
                    model: Some(public_model_id),
                    upstream_model: Some(upstream_model_id),
                    started,
                    upstream_url: sanitized_upstream_url,
                    request_summary: Some(request_summary),
                },
                classify_transport_failure(error.to_string(), Utc::now()),
            )
            .await?;
            Err(AppError::Upstream(error.into()))
        }
    }
}

struct UpstreamFailureLog<'a> {
    account_id: &'a str,
    route_id: &'a str,
    api_key_id: String,
    path: String,
    model: Option<String>,
    upstream_model: Option<String>,
    started: Instant,
    upstream_url: Option<String>,
    request_summary: Option<RequestSummary>,
}

async fn record_upstream_failure(
    state: &AppState,
    context: UpstreamFailureLog<'_>,
    failure: RouteFailure,
) -> Result<(), AppError> {
    record_route_failure_state(state, context.account_id, context.route_id, &failure).await?;
    state
        .db
        .insert_request_log(RequestLog {
            id: Uuid::new_v4().to_string(),
            api_key_id: context.api_key_id,
            provider_account_id: Some(context.account_id.to_string()),
            method: "POST".to_string(),
            path: context.path,
            model: context.model,
            upstream_model: context.upstream_model,
            upstream_url: context.upstream_url,
            request_summary: context.request_summary,
            status_code: failure
                .status_code
                .unwrap_or(StatusCode::BAD_GATEWAY.as_u16()),
            latency_ms: context.started.elapsed().as_millis() as u64,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            created_at: Utc::now(),
            error: Some(failure.error),
        })
        .await?;
    Ok(())
}

async fn record_upstream_response_result(
    state: &AppState,
    account_id: &str,
    route_id: &str,
    provider: &str,
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Option<String>, AppError> {
    if status.is_success() {
        state
            .db
            .mark_provider_result(account_id, "healthy", None)
            .await?;
        state
            .db
            .mark_route_success(route_id, status.as_u16())
            .await?;
        return Ok(None);
    }

    let failure = classify_response_failure(provider, status, headers, body, Utc::now());
    let error = failure.error.clone();
    record_route_failure_state(state, account_id, route_id, &failure).await?;
    Ok(Some(error))
}

async fn record_route_failure_state(
    state: &AppState,
    account_id: &str,
    route_id: &str,
    failure: &RouteFailure,
) -> Result<(), AppError> {
    if let Some(provider_status) = failure.provider_status {
        state
            .db
            .mark_provider_result(account_id, provider_status, Some(&failure.error))
            .await?;
    }
    state
        .db
        .mark_route_failure(
            route_id,
            failure.route_status,
            failure.status_code,
            &failure.error,
            failure.cooldown_until,
        )
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

pub async fn start_antigravity_oauth(
    State(state): State<AppState>,
    Json(input): Json<AntigravityOAuthStartRequest>,
) -> Result<Json<AntigravityOAuthStartResponse>, AppError> {
    Ok(Json(
        begin_antigravity_oauth(&state.antigravity_oauth, input).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct AntigravityOAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn antigravity_oauth_callback(
    State(state): State<AppState>,
    Query(query): Query<AntigravityOAuthCallbackQuery>,
) -> Html<String> {
    let outcome = complete_antigravity_oauth(
        &state.antigravity_oauth,
        &state.db,
        &state.gemini_http,
        query.state.as_deref(),
        query.code.as_deref(),
        query.error.as_deref(),
        query.error_description.as_deref(),
    )
    .await;
    let success = outcome.error.is_none();
    let payload = json!({
        "type": "token-toxication:antigravity-oauth",
        "success": success,
        "accountId": outcome.account_id,
        "error": outcome.error,
    });
    let payload = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let notify_opener = outcome.opener_origin.map_or_else(String::new, |origin| {
        let origin = serde_json::to_string(&origin).unwrap_or_else(|_| "null".to_string());
        format!("if (window.opener) {{ window.opener.postMessage(payload, {origin}); }}")
    });
    Html(format!(
        r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Antigravity OAuth</title></head>
<body style="font:14px system-ui,sans-serif;margin:40px;color:#18181b">
  <strong id="title"></strong>
  <p id="message"></p>
  <script>
    const payload = {payload};
    document.getElementById("title").textContent = payload.success ? "Antigravity connected" : "Antigravity sign-in failed";
    document.getElementById("message").textContent = payload.success ? "This window can close now." : payload.error;
    {notify_opener}
    if (payload.success) setTimeout(() => window.close(), 800);
  </script>
</body>
</html>"#
    ))
}

pub async fn get_gemini_account_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GeminiAccountModelsResponse>, AppError> {
    Ok(Json(
        gemini_account_models(&state.db, &state.gemini_http, &id).await?,
    ))
}

pub async fn get_gemini_account_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GeminiAccountQuotaResponse>, AppError> {
    Ok(Json(
        gemini_account_quota(&state.db, &state.gemini_http, &id).await?,
    ))
}

pub async fn get_codex_account_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CodexAccountQuotaResponse>, AppError> {
    Ok(Json(
        codex_account_quota(&state.db, &state.http, &id).await?,
    ))
}

pub async fn list_provider_presets() -> Json<ProviderPresetListResponse> {
    Json(ProviderPresetListResponse {
        data: provider_presets(),
    })
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

pub async fn list_model_catalog(
    State(state): State<AppState>,
) -> Result<Json<ModelCatalogListResponse>, AppError> {
    Ok(Json(ModelCatalogListResponse {
        data: state.db.list_model_catalog().await?,
    }))
}

pub async fn create_model_catalog_entry(
    State(state): State<AppState>,
    Json(input): Json<CreateModelCatalogEntryRequest>,
) -> Result<(StatusCode, Json<ModelCatalogEntryResponse>), AppError> {
    if input.id.trim().is_empty() {
        return Err(AppError::BadRequest("model id is required".into()));
    }
    let entry = state
        .db
        .create_model_catalog_entry(input)
        .await
        .map_err(map_write_error)?;
    Ok((
        StatusCode::CREATED,
        Json(ModelCatalogEntryResponse { data: entry }),
    ))
}

pub async fn update_model_catalog_entry(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateModelCatalogEntryRequest>,
) -> Result<Json<ModelCatalogEntryResponse>, AppError> {
    let entry = state
        .db
        .update_model_catalog_entry(&id, input)
        .await
        .map_err(map_write_or_not_found)?;
    Ok(Json(ModelCatalogEntryResponse { data: entry }))
}

pub async fn list_provider_model_routes(
    State(state): State<AppState>,
) -> Result<Json<ProviderModelRouteListResponse>, AppError> {
    Ok(Json(ProviderModelRouteListResponse {
        data: state.db.list_provider_model_routes().await?,
    }))
}

pub async fn create_provider_model_route(
    State(state): State<AppState>,
    Json(input): Json<CreateProviderModelRouteRequest>,
) -> Result<(StatusCode, Json<ProviderModelRouteResponse>), AppError> {
    validate_provider_model_route_input(
        &input.public_model_id,
        &input.provider_account_id,
        &input.upstream_model_id,
    )?;
    let route = state
        .db
        .create_provider_model_route(input)
        .await
        .map_err(map_write_error)?;
    Ok((
        StatusCode::CREATED,
        Json(ProviderModelRouteResponse { data: route }),
    ))
}

pub async fn update_provider_model_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProviderModelRouteRequest>,
) -> Result<Json<ProviderModelRouteResponse>, AppError> {
    if input
        .public_model_id
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
        || input
            .provider_account_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        || input
            .upstream_model_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(AppError::BadRequest(
            "public model, provider account, and upstream model cannot be empty".into(),
        ));
    }
    let route = state
        .db
        .update_provider_model_route(&id, input)
        .await
        .map_err(map_write_or_not_found)?;
    Ok(Json(ProviderModelRouteResponse { data: route }))
}

pub async fn delete_provider_model_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if !state.db.delete_provider_model_route(&id).await? {
        return Err(AppError::NotFound("provider model route not found".into()));
    }
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

fn validate_gemini_generate_content_request(value: &Value) -> Result<(), AppError> {
    if !value.is_object() {
        return Err(AppError::BadRequest(
            "request body must be an object".into(),
        ));
    }
    if value
        .get("contents")
        .is_some_and(|contents| !contents.is_null())
    {
        Ok(())
    } else {
        Err(AppError::BadRequest("missing Gemini contents field".into()))
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
    } else if auth_mode == "x-goog-api-key" {
        let api_key = HeaderValue::from_str(api_key)
            .map_err(|error| AppError::Internal(format!("invalid provider API key: {error}")))?;
        Ok(request.header(HeaderName::from_static("x-goog-api-key"), api_key))
    } else if auth_mode == "codex-oauth" {
        Err(AppError::Internal(
            "codex-oauth provider auth must be resolved before proxying".into(),
        ))
    } else {
        let api_key = HeaderValue::from_str(api_key)
            .map_err(|error| AppError::Internal(format!("invalid provider API key: {error}")))?;
        Ok(request.header(HeaderName::from_static("x-api-key"), api_key))
    }
}

fn apply_codex_subscription_auth<'a, R, C>(
    request: RequestBuilderSend<'a, R, C>,
    auth: &CodexSubscriptionAuthorization,
) -> Result<RequestBuilderSend<'a, R, C>, AppError>
where
    R: RuntimePoll,
    C: ConnectorSend,
{
    let mut request = request.bearer_auth(&auth.access_token).header(
        HeaderName::from_static("originator"),
        HeaderValue::from_static("opencode"),
    );
    if let Some(account_id) = &auth.account_id {
        let account_id = HeaderValue::from_str(account_id).map_err(|error| {
            AppError::Internal(format!("invalid ChatGPT account id header: {error}"))
        })?;
        request = request.header(HeaderName::from_static("chatgpt-account-id"), account_id);
    }
    Ok(request)
}

fn upstream_url(base_url: &str, path: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if let Some((version, rest)) = path.split_once('/')
        && version.starts_with('v')
        && base_url.ends_with(&format!("/{version}"))
    {
        format!("{base_url}/{rest}")
    } else {
        format!("{base_url}/{path}")
    }
}

fn gemini_code_assist_upstream_url(
    endpoint: &str,
    method: GeminiMethod,
    query: Option<&str>,
) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    let endpoint = endpoint
        .strip_suffix("/v1internal")
        .unwrap_or(endpoint)
        .trim_end_matches('/');
    let url = format!("{endpoint}/v1internal:{}", method.as_str());
    match forwarded_code_assist_query(query, method) {
        Some(query) => format!("{url}?{query}"),
        None => url,
    }
}

fn gemini_public_path(model: &str, method: GeminiMethod) -> String {
    format!("/gemini/v1beta/models/{model}:{}", method.as_str())
}

fn parse_gemini_model_operation(operation: &str) -> Result<(String, GeminiMethod), AppError> {
    let (model, method) = operation.rsplit_once(':').ok_or_else(|| {
        AppError::BadRequest(
            "Gemini path must end with :generateContent or :streamGenerateContent".into(),
        )
    })?;
    if model.trim().is_empty() {
        return Err(AppError::BadRequest("Gemini model is required".into()));
    }
    let method = match method {
        "generateContent" => GeminiMethod::GenerateContent,
        "streamGenerateContent" => GeminiMethod::StreamGenerateContent,
        _ => {
            return Err(AppError::BadRequest(
                "unsupported Gemini generateContent method".into(),
            ));
        }
    };
    Ok((model.to_string(), method))
}

fn forwarded_code_assist_query(query: Option<&str>, method: GeminiMethod) -> Option<String> {
    let mut forwarded = Vec::new();
    if method.is_stream() {
        forwarded.push("alt=sse");
    }
    if let Some(query) = query {
        forwarded.extend(
            query
                .split('&')
                .filter(|part| !part.is_empty())
                .filter(|part| {
                    let key = part.split_once('=').map_or(*part, |(key, _)| key);
                    key != "key" && key != "alt"
                }),
        );
    }
    if forwarded.is_empty() {
        None
    } else {
        Some(forwarded.join("&"))
    }
}

fn take_sse_event(buffer: &str) -> Option<(String, String)> {
    if let Some(index) = buffer.find("\n\n") {
        let event = buffer[..index].to_string();
        let rest = buffer[index + 2..].to_string();
        return Some((event, rest));
    }
    if let Some(index) = buffer.find("\r\n\r\n") {
        let event = buffer[..index].to_string();
        let rest = buffer[index + 4..].to_string();
        return Some((event, rest));
    }
    None
}

fn transform_code_assist_sse_event(event: &str) -> String {
    let mut transformed = String::new();
    for line in event.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            transformed.push_str("data: ");
            transformed.push_str(&unwrap_code_assist_sse_data(data.trim_start()));
            transformed.push('\n');
        } else {
            transformed.push_str(line);
            transformed.push('\n');
        }
    }
    transformed.push('\n');
    transformed
}

fn sanitize_upstream_url(value: &str) -> String {
    match value.parse::<Uri>() {
        Ok(uri) => match (uri.scheme_str(), uri.authority()) {
            (Some(scheme), Some(authority)) => format!("{scheme}://{}{}", authority, uri.path()),
            _ => value.split('?').next().unwrap_or(value).to_string(),
        },
        Err(_) => value.split('?').next().unwrap_or(value).to_string(),
    }
}

fn strip_top_level_params(value: &mut Value, strip_params: &[String]) -> Vec<String> {
    let Some(object) = value.as_object_mut() else {
        return Vec::new();
    };
    let mut stripped = Vec::new();
    for param in strip_params {
        if object.remove(param).is_some() {
            stripped.push(param.clone());
        }
    }
    stripped
}

fn strip_upstream_params(
    value: &mut Value,
    configured_strip_params: &[String],
    wire_api: WireApi,
    auth_mode: &str,
) -> Vec<String> {
    let mut stripped = strip_top_level_params(value, configured_strip_params);
    if wire_api != WireApi::OpenAiResponses || !is_codex_subscription_auth(auth_mode) {
        return stripped;
    }

    // Codex subscription endpoints follow the Codex CLI request shape, which
    // omits max_output_tokens even though the public Responses API accepts it.
    let Some(object) = value.as_object_mut() else {
        return stripped;
    };
    if object.remove("max_output_tokens").is_some() {
        stripped.push("max_output_tokens".to_string());
    }
    stripped
}

fn build_request_summary(
    value: &Value,
    body_bytes: u64,
    stripped_params: Vec<String>,
) -> RequestSummary {
    let mut top_level_keys = value
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    top_level_keys.sort();
    RequestSummary {
        top_level_keys,
        body_bytes,
        stream: value
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        stripped_params,
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

fn parse_usage(bytes: &[u8]) -> (u64, u64) {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return (0, 0);
    };
    let Some(usage) = value.get("usage") else {
        if let Some(usage) = value.get("usageMetadata") {
            let input = usage
                .get("promptTokenCount")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let output = usage
                .get("candidatesTokenCount")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            return (input, output);
        }
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
        .list_routable_model_catalog(&["openai-chat", "openai-responses"])
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

async fn gemini_models(state: &AppState) -> Result<Vec<GeminiModel>, AppError> {
    let models = state
        .db
        .list_routable_model_catalog(&[WireApi::GeminiGenerateContent.account_value()])
        .await?;
    Ok(models
        .into_iter()
        .map(|model| GeminiModel {
            name: format!("models/{}", model.id),
            display_name: model.display_name,
            supported_generation_methods: vec![
                "generateContent".to_string(),
                "streamGenerateContent".to_string(),
            ],
        })
        .collect())
}

async fn anthropic_models(state: &AppState) -> Result<Vec<AnthropicModel>, AppError> {
    let models = state
        .db
        .list_routable_model_catalog(&["anthropic-messages"])
        .await?;
    Ok(models
        .into_iter()
        .map(|model| AnthropicModel {
            r#type: "model".to_string(),
            display_name: model.display_name,
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

fn map_write_or_not_found(error: rusqlite::Error) -> AppError {
    match error {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("record not found".into()),
        _ => map_write_error(error),
    }
}

fn map_write_error(error: rusqlite::Error) -> AppError {
    let message = error.to_string();
    if message.contains("UNIQUE constraint failed")
        || message.contains("FOREIGN KEY constraint failed")
        || message.contains("NOT NULL constraint failed")
    {
        AppError::BadRequest(
            "invalid model catalog or route data; check duplicate primary routes and references"
                .into(),
        )
    } else {
        AppError::Database(error)
    }
}

fn validate_provider_model_route_input(
    public_model_id: &str,
    provider_account_id: &str,
    upstream_model_id: &str,
) -> Result<(), AppError> {
    if public_model_id.trim().is_empty()
        || provider_account_id.trim().is_empty()
        || upstream_model_id.trim().is_empty()
    {
        return Err(AppError::BadRequest(
            "public model, provider account, and upstream model are required".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strip_params_remove_only_top_level_keys_and_summarize_metadata() {
        let mut body = json!({
            "model": "public-model",
            "temperature": 0.7,
            "messages": [
                {
                    "role": "user",
                    "content": "do not persist this",
                    "temperature": 1.0
                }
            ],
            "stream": true
        });

        let stripped =
            strip_top_level_params(&mut body, &["temperature".to_string(), "top_p".to_string()]);
        let summary = build_request_summary(&body, 123, stripped);

        assert!(body.get("temperature").is_none());
        assert_eq!(body["messages"][0]["temperature"], 1.0);
        assert_eq!(summary.stripped_params, vec!["temperature"]);
        assert_eq!(summary.top_level_keys, vec!["messages", "model", "stream"]);
        assert_eq!(summary.body_bytes, 123);
        assert!(summary.stream);
    }

    #[test]
    fn codex_subscription_responses_strip_unsupported_max_output_tokens() {
        let mut body = json!({
            "model": "gpt-5",
            "max_output_tokens": 32_000,
            "input": "hello"
        });

        let stripped =
            strip_upstream_params(&mut body, &[], WireApi::OpenAiResponses, "codex-oauth");

        assert!(body.get("max_output_tokens").is_none());
        assert_eq!(stripped, vec!["max_output_tokens"]);
    }

    #[test]
    fn api_key_responses_preserve_max_output_tokens() {
        let mut body = json!({
            "model": "gpt-5",
            "max_output_tokens": 32_000,
            "input": "hello"
        });

        let stripped = strip_upstream_params(&mut body, &[], WireApi::OpenAiResponses, "bearer");

        assert_eq!(body["max_output_tokens"], 32_000);
        assert!(stripped.is_empty());
    }

    #[test]
    fn codex_normalization_does_not_duplicate_configured_strip_params() {
        let mut body = json!({
            "model": "gpt-5",
            "max_output_tokens": 32_000,
            "input": "hello"
        });

        let stripped = strip_upstream_params(
            &mut body,
            &["max_output_tokens".to_string()],
            WireApi::OpenAiResponses,
            "codex-oauth",
        );

        assert!(body.get("max_output_tokens").is_none());
        assert_eq!(stripped, vec!["max_output_tokens"]);
    }

    #[test]
    fn sanitize_upstream_url_removes_query() {
        assert_eq!(
            sanitize_upstream_url("https://api.example.com/v1/messages?token=secret"),
            "https://api.example.com/v1/messages"
        );
    }

    #[test]
    fn gemini_model_operation_parses_generate_methods() {
        let (model, method) =
            parse_gemini_model_operation("gemini-3.5-flash:generateContent").unwrap();
        assert_eq!(model, "gemini-3.5-flash");
        assert_eq!(method, GeminiMethod::GenerateContent);

        let (model, method) =
            parse_gemini_model_operation("gemini-3.5-flash:streamGenerateContent").unwrap();
        assert_eq!(model, "gemini-3.5-flash");
        assert_eq!(method, GeminiMethod::StreamGenerateContent);
        assert!(parse_gemini_model_operation("gemini-3.5-flash:countTokens").is_err());
    }

    #[test]
    fn gemini_code_assist_upstream_url_targets_internal_endpoint() {
        assert_eq!(
            gemini_code_assist_upstream_url(
                "https://cloudcode-pa.googleapis.com",
                GeminiMethod::StreamGenerateContent,
                Some("key=tokentoxication-secret&alt=json&trace=1")
            ),
            "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse&trace=1"
        );

        assert_eq!(
            gemini_code_assist_upstream_url(
                "https://cloudcode-pa.googleapis.com/v1internal",
                GeminiMethod::GenerateContent,
                None
            ),
            "https://cloudcode-pa.googleapis.com/v1internal:generateContent"
        );
    }

    #[test]
    fn gemini_code_assist_sse_event_unwraps_response() {
        let event = transform_code_assist_sse_event(
            r#"data: {"traceId":"trace-1","response":{"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}}"#,
        );

        assert!(event.starts_with("data: {"));
        assert!(event.contains(r#""responseId":"trace-1""#));
        assert!(event.contains(r#""text":"hi""#));
        assert!(!event.contains(r#""response":{"#));
    }

    #[test]
    fn gemini_generate_content_requires_contents() {
        assert!(
            validate_gemini_generate_content_request(&json!({
                "contents": [{"parts": [{"text": "hello"}]}]
            }))
            .is_ok()
        );
        assert!(validate_gemini_generate_content_request(&json!({"input": "hello"})).is_err());
    }

    #[test]
    fn parse_usage_reads_gemini_usage_metadata() {
        let body = serde_json::to_vec(&json!({
            "usageMetadata": {
                "promptTokenCount": 11,
                "candidatesTokenCount": 7
            }
        }))
        .unwrap();

        assert_eq!(parse_usage(&body), (11, 7));
    }
}
