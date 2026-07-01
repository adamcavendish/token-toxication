use axum::Json;
use utoipa::OpenApi;

use crate::models::{
    AdminUser, AnthropicModel, AnthropicModelListResponse, ApiKeyListResponse, ApiKeyResponse,
    ApiKeyView, CreateApiKeyRequest, CreateApiKeyResponse, CreateProviderAccountRequest, Dashboard,
    ErrorDetail, ErrorResponse, HealthResponse, LoginRequest, LoginResponse, MetricsResponse,
    OpenAiModel, OpenAiModelListResponse, ProviderAccount, ProviderAccountListResponse,
    ProviderAccountResponse, RequestLog, RequestLogListResponse, UpdateApiKeyRequest,
    UpdateProviderAccountRequest, UsageSummary,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Token Toxication API",
        version = "0.1.0",
        description = "Admin and relay control API for Token Toxication.",
        license(name = "MIT OR Apache-2.0"),
    ),
    paths(
        health,
        metrics,
        admin_login,
        admin_logout,
        admin_me,
        admin_dashboard,
        list_api_keys,
        create_api_key,
        update_api_key,
        delete_api_key,
        list_provider_accounts,
        create_provider_account,
        update_provider_account,
        delete_provider_account,
        list_request_logs,
        list_anthropic_models,
        get_anthropic_model,
        list_openai_models,
        get_openai_model,
        relay_anthropic_messages,
        relay_openai_chat_completions,
        relay_openai_responses,
    ),
    components(schemas(
        AdminUser,
        AnthropicModel,
        AnthropicModelListResponse,
        ApiKeyListResponse,
        ApiKeyResponse,
        ApiKeyView,
        CreateApiKeyRequest,
        CreateApiKeyResponse,
        CreateProviderAccountRequest,
        Dashboard,
        ErrorDetail,
        ErrorResponse,
        HealthResponse,
        LoginRequest,
        LoginResponse,
        MetricsResponse,
        OpenAiModel,
        OpenAiModelListResponse,
        ProviderAccount,
        ProviderAccountListResponse,
        ProviderAccountResponse,
        RequestLog,
        RequestLogListResponse,
        UpdateApiKeyRequest,
        UpdateProviderAccountRequest,
        UsageSummary,
    )),
    tags(
        (name = "Runtime", description = "Runtime health and metrics"),
        (name = "Admin", description = "Administrative control-plane API"),
        (name = "Relay", description = "Namespaced provider-compatible relay API"),
    ),
)]
pub struct ApiDoc;

pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "Runtime",
    responses((status = 200, description = "Service health", body = HealthResponse)),
)]
pub fn health() {}

#[utoipa::path(
    get,
    path = "/metrics",
    tag = "Runtime",
    responses(
        (status = 200, description = "Runtime metrics", body = MetricsResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
pub fn metrics() {}

#[utoipa::path(
    post,
    path = "/admin/api/auth/login",
    tag = "Admin",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Admin session", body = LoginResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
    ),
)]
pub fn admin_login() {}

#[utoipa::path(
    post,
    path = "/admin/api/auth/logout",
    tag = "Admin",
    responses((status = 204, description = "Session deleted")),
)]
pub fn admin_logout() {}

#[utoipa::path(
    get,
    path = "/admin/api/auth/me",
    tag = "Admin",
    responses(
        (status = 200, description = "Current admin user", body = AdminUser),
        (status = 401, description = "Missing or invalid session", body = ErrorResponse),
    ),
)]
pub fn admin_me() {}

#[utoipa::path(
    get,
    path = "/admin/api/dashboard",
    tag = "Admin",
    responses(
        (status = 200, description = "Dashboard summary", body = Dashboard),
        (status = 401, description = "Missing or invalid session", body = ErrorResponse),
    ),
)]
pub fn admin_dashboard() {}

#[utoipa::path(
    get,
    path = "/admin/api/api-keys",
    tag = "Admin",
    responses((status = 200, description = "API keys", body = ApiKeyListResponse)),
)]
pub fn list_api_keys() {}

#[utoipa::path(
    post,
    path = "/admin/api/api-keys",
    tag = "Admin",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "Created API key", body = CreateApiKeyResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
    ),
)]
pub fn create_api_key() {}

#[utoipa::path(
    patch,
    path = "/admin/api/api-keys/{id}",
    tag = "Admin",
    params(("id" = String, Path, description = "API key id")),
    request_body = UpdateApiKeyRequest,
    responses(
        (status = 200, description = "Updated API key", body = ApiKeyResponse),
        (status = 404, description = "API key not found", body = ErrorResponse),
    ),
)]
pub fn update_api_key() {}

#[utoipa::path(
    delete,
    path = "/admin/api/api-keys/{id}",
    tag = "Admin",
    params(("id" = String, Path, description = "API key id")),
    responses(
        (status = 204, description = "Deleted API key"),
        (status = 404, description = "API key not found", body = ErrorResponse),
    ),
)]
pub fn delete_api_key() {}

#[utoipa::path(
    get,
    path = "/admin/api/provider-accounts",
    tag = "Admin",
    responses((status = 200, description = "Provider accounts", body = ProviderAccountListResponse)),
)]
pub fn list_provider_accounts() {}

#[utoipa::path(
    post,
    path = "/admin/api/provider-accounts",
    tag = "Admin",
    request_body = CreateProviderAccountRequest,
    responses(
        (status = 201, description = "Created provider account", body = ProviderAccountResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
    ),
)]
pub fn create_provider_account() {}

#[utoipa::path(
    patch,
    path = "/admin/api/provider-accounts/{id}",
    tag = "Admin",
    params(("id" = String, Path, description = "Provider account id")),
    request_body = UpdateProviderAccountRequest,
    responses(
        (status = 200, description = "Updated provider account", body = ProviderAccountResponse),
        (status = 404, description = "Provider account not found", body = ErrorResponse),
    ),
)]
pub fn update_provider_account() {}

#[utoipa::path(
    delete,
    path = "/admin/api/provider-accounts/{id}",
    tag = "Admin",
    params(("id" = String, Path, description = "Provider account id")),
    responses(
        (status = 204, description = "Deleted provider account"),
        (status = 404, description = "Provider account not found", body = ErrorResponse),
    ),
)]
pub fn delete_provider_account() {}

#[utoipa::path(
    get,
    path = "/admin/api/request-logs",
    tag = "Admin",
    params(("limit" = Option<u32>, Query, description = "Maximum number of request logs to return")),
    responses((status = 200, description = "Request logs", body = RequestLogListResponse)),
)]
pub fn list_request_logs() {}

#[utoipa::path(
    get,
    path = "/anthropic/v1/models",
    tag = "Relay",
    responses(
        (status = 200, description = "Configured Anthropic-compatible models", body = AnthropicModelListResponse),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
    ),
)]
pub fn list_anthropic_models() {}

#[utoipa::path(
    get,
    path = "/anthropic/v1/models/{model}",
    tag = "Relay",
    params(("model" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "Configured Anthropic-compatible model", body = AnthropicModel),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
        (status = 404, description = "Model not found", body = ErrorResponse),
    ),
)]
pub fn get_anthropic_model() {}

#[utoipa::path(
    get,
    path = "/openai/v1/models",
    tag = "Relay",
    responses(
        (status = 200, description = "Configured OpenAI-compatible models", body = OpenAiModelListResponse),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
    ),
)]
pub fn list_openai_models() {}

#[utoipa::path(
    get,
    path = "/openai/v1/models/{model}",
    tag = "Relay",
    params(("model" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "Configured OpenAI-compatible model", body = OpenAiModel),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
        (status = 404, description = "Model not found", body = ErrorResponse),
    ),
)]
pub fn get_openai_model() {}

#[utoipa::path(
    post,
    path = "/anthropic/v1/messages",
    tag = "Relay",
    request_body(
        content = serde_json::Value,
        content_type = "application/json",
        description = "Anthropic Messages-compatible request body",
    ),
    responses(
        (status = 200, description = "Upstream Anthropic-compatible response", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
        (status = 403, description = "No matching provider account", body = ErrorResponse),
    ),
)]
pub fn relay_anthropic_messages() {}

#[utoipa::path(
    post,
    path = "/openai/v1/chat/completions",
    tag = "Relay",
    request_body(
        content = serde_json::Value,
        content_type = "application/json",
        description = "OpenAI Chat Completions-compatible request body",
    ),
    responses(
        (status = 200, description = "Upstream OpenAI-compatible response", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
        (status = 403, description = "No matching provider account", body = ErrorResponse),
    ),
)]
pub fn relay_openai_chat_completions() {}

#[utoipa::path(
    post,
    path = "/openai/v1/responses",
    tag = "Relay",
    request_body(
        content = serde_json::Value,
        content_type = "application/json",
        description = "OpenAI Responses-compatible request body",
    ),
    responses(
        (status = 200, description = "Upstream OpenAI-compatible response", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Missing or invalid API key", body = ErrorResponse),
        (status = 403, description = "No matching provider account", body = ErrorResponse),
    ),
)]
pub fn relay_openai_responses() {}
