use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub key_preview: String,
    pub is_active: bool,
    pub permissions: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub concurrency_limit: u32,
    pub daily_cost_limit: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    pub view: ApiKeyView,
    pub key_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub rate_limit_per_minute: u32,
    #[serde(default)]
    pub concurrency_limit: u32,
    #[serde(default)]
    pub daily_cost_limit: f64,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyResponse {
    pub key: ApiKeyView,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub permissions: Option<Vec<String>>,
    pub rate_limit_per_minute: Option<u32>,
    pub concurrency_limit: Option<u32>,
    pub daily_cost_limit: Option<f64>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccount {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub base_url: String,
    pub auth_mode: String,
    pub wire_api: String,
    pub model_hint: String,
    pub is_active: bool,
    pub priority: i32,
    pub status: String,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ProviderAccountRecord {
    pub account: ProviderAccount,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderAccountRequest {
    pub name: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub base_url: String,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: String,
    #[serde(default)]
    pub wire_api: String,
    pub api_key: String,
    #[serde(default)]
    pub model_hint: String,
    #[serde(default = "default_active")]
    pub is_active: bool,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderAccountRequest {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub auth_mode: Option<String>,
    pub wire_api: Option<String>,
    pub api_key: Option<String>,
    pub model_hint: Option<String>,
    pub is_active: Option<bool>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestLog {
    pub id: String,
    pub api_key_id: String,
    pub provider_account_id: Option<String>,
    pub method: String,
    pub path: String,
    pub model: Option<String>,
    pub status_code: u16,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub created_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub requests_today: u64,
    pub tokens_today: u64,
    pub total_requests: u64,
    pub total_tokens: u64,
    pub estimated_cost_today: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub active_api_keys: u64,
    pub total_api_keys: u64,
    pub healthy_accounts: u64,
    pub total_accounts: u64,
    pub usage: UsageSummary,
    pub accounts: Vec<ProviderAccount>,
    pub recent_requests: Vec<RequestLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyListResponse {
    pub data: Vec<ApiKeyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyResponse {
    pub data: ApiKeyView,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountListResponse {
    pub data: Vec<ProviderAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountResponse {
    pub data: ProviderAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogListResponse {
    pub data: Vec<RequestLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenAiModel {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpenAiModelListResponse {
    pub object: String,
    pub data: Vec<OpenAiModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AnthropicModel {
    pub r#type: String,
    pub id: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AnthropicModelListResponse {
    pub data: Vec<AnthropicModel>,
    pub has_more: bool,
    pub first_id: Option<String>,
    pub last_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub r#type: u16,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub uptime_seconds: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResponse {
    pub active_api_keys: u64,
    pub total_api_keys: u64,
    pub healthy_accounts: u64,
    pub total_accounts: u64,
    pub usage: UsageSummary,
    pub timestamp: DateTime<Utc>,
}

pub fn default_provider() -> String {
    "anthropic".to_string()
}

pub fn default_auth_mode() -> String {
    "x-api-key".to_string()
}

pub fn default_active() -> bool {
    true
}
