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
    pub upstream_model: Option<String>,
    pub upstream_url: Option<String>,
    pub request_summary: Option<RequestSummary>,
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
pub struct AntigravityOAuthStartRequest {
    pub account_id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub priority: i32,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityOAuthStartResponse {
    pub authorization_url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountTier {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountModel {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountModelsResponse {
    pub account_id: String,
    pub project: Option<String>,
    pub endpoint: String,
    pub models: Vec<GeminiAccountModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountQuota {
    pub model_id: String,
    pub remaining_fraction: Option<f64>,
    pub reset_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountQuotaBucket {
    pub bucket_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub window: Option<String>,
    pub remaining_fraction: Option<f64>,
    pub remaining_amount: Option<f64>,
    pub disabled: Option<bool>,
    pub reset_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountQuotaGroup {
    pub display_name: String,
    pub description: Option<String>,
    pub buckets: Vec<GeminiAccountQuotaBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountQuotaSummary {
    pub description: Option<String>,
    pub buckets: Vec<GeminiAccountQuotaBucket>,
    pub groups: Vec<GeminiAccountQuotaGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAccountQuotaResponse {
    pub account_id: String,
    pub auth_mode: String,
    pub project: Option<String>,
    pub endpoint: String,
    pub quota_source: String,
    pub current_tier: Option<GeminiAccountTier>,
    pub paid_tier: Option<GeminiAccountTier>,
    pub quotas: Vec<GeminiAccountQuota>,
    pub quota_summary: Option<GeminiAccountQuotaSummary>,
    pub quota_summary_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountQuotaWindow {
    pub used_percent: Option<f64>,
    pub limit_window_seconds: Option<i64>,
    pub reset_after_seconds: Option<i64>,
    pub reset_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountQuotaLimit {
    pub limit_id: String,
    pub display_name: String,
    pub allowed: Option<bool>,
    pub limit_reached: Option<bool>,
    pub primary_window: Option<CodexAccountQuotaWindow>,
    pub secondary_window: Option<CodexAccountQuotaWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountCredits {
    pub has_credits: Option<bool>,
    pub unlimited: Option<bool>,
    pub balance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountSpendControlLimit {
    pub source: Option<String>,
    pub limit: Option<String>,
    pub used: Option<String>,
    pub remaining: Option<String>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub reset_after_seconds: Option<i64>,
    pub reset_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountSpendControl {
    pub reached: Option<bool>,
    pub individual_limit: Option<CodexAccountSpendControlLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountQuotaResponse {
    pub account_id: String,
    pub auth_mode: String,
    pub endpoint: String,
    pub plan_type: Option<String>,
    pub limits: Vec<CodexAccountQuotaLimit>,
    pub credits: Option<CodexAccountCredits>,
    pub spend_control: Option<CodexAccountSpendControl>,
    pub rate_limit_reached_type: Option<String>,
    pub reset_credits_available_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    pub id: String,
    pub label: String,
    pub name: String,
    pub provider: String,
    pub base_url: String,
    pub auth_mode: String,
    pub wire_api: String,
    pub credential_label: String,
    pub credential_placeholder: String,
    pub credential_help: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPresetListResponse {
    pub data: Vec<ProviderPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelCatalogEntryRequest {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub family: String,
    #[serde(default = "default_active")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelCatalogEntryRequest {
    pub display_name: Option<String>,
    pub family: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogListResponse {
    pub data: Vec<ModelCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntryResponse {
    pub data: ModelCatalogEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelRoute {
    pub id: String,
    pub public_model_id: String,
    pub provider_account_id: String,
    pub upstream_model_id: String,
    pub wire_api: String,
    pub role: String,
    pub enabled: bool,
    pub status: String,
    pub last_error: Option<String>,
    pub last_status_code: Option<u16>,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub strip_params: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderModelRouteRequest {
    pub public_model_id: String,
    pub provider_account_id: String,
    pub upstream_model_id: String,
    pub wire_api: String,
    #[serde(default = "default_route_role")]
    pub role: String,
    #[serde(default = "default_active")]
    pub enabled: bool,
    #[serde(default)]
    pub strip_params: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderModelRouteRequest {
    pub public_model_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub upstream_model_id: Option<String>,
    pub wire_api: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub strip_params: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelRouteListResponse {
    pub data: Vec<ProviderModelRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelRouteResponse {
    pub data: ProviderModelRoute,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogListResponse {
    pub data: Vec<RequestLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestSummary {
    pub top_level_keys: Vec<String>,
    pub body_bytes: u64,
    pub stream: bool,
    pub stripped_params: Vec<String>,
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
#[serde(rename_all = "camelCase")]
pub struct GeminiModel {
    pub name: String,
    pub display_name: String,
    pub supported_generation_methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GeminiModelListResponse {
    pub models: Vec<GeminiModel>,
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

pub fn default_route_role() -> String {
    "primary".to_string()
}
