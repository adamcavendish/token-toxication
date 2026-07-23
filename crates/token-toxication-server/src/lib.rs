pub mod acme;
pub mod antigravity_oauth;
pub mod auth;
pub mod codex_subscription;
pub mod config;
pub mod db;
pub mod error;
pub mod gemini_code_assist;
pub mod models;
pub mod openapi;
pub mod provider_catalog;
pub(crate) mod relay_attempt;
pub mod routes;
pub mod routing;
pub mod server;
pub mod static_assets;

use std::{path::PathBuf, sync::Arc, time::Duration};

use axum::Router;
use chrono::{DateTime, Utc};
use config::Config;
use db::Db;
use routes::{
    admin_routes, antigravity_oauth_callback, get_anthropic_model, get_gemini_model,
    get_openai_model, health, list_anthropic_models, list_gemini_models, list_openai_models,
    metrics, relay_gemini_generate_content, relay_messages, relay_openai_chat,
    relay_openai_responses,
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub http: aioduct::TokioClient,
    pub gemini_http: aioduct::TokioClient,
    pub antigravity_oauth: antigravity_oauth::AntigravityOAuthStore,
    pub relay_stream_idle_timeout: Duration,
    pub relay_stream_max_duration: Duration,
    pub shutdown: server::ShutdownSignal,
    pub started_at: DateTime<Utc>,
}

pub fn app(state: AppState, static_dir: PathBuf) -> Router {
    let index_file = static_dir.join("index.html");

    let app = Router::new()
        .route("/health", axum::routing::get(health))
        .route("/metrics", axum::routing::get(metrics))
        .route("/openapi.json", axum::routing::get(openapi::openapi_json))
        .route(
            "/oauth-callback",
            axum::routing::get(antigravity_oauth_callback),
        )
        .route(
            "/anthropic/v1/messages",
            axum::routing::post(relay_messages),
        )
        .route(
            "/anthropic/v1/models",
            axum::routing::get(list_anthropic_models),
        )
        .route(
            "/anthropic/v1/models/{model}",
            axum::routing::get(get_anthropic_model),
        )
        .route("/openai/v1/models", axum::routing::get(list_openai_models))
        .route(
            "/openai/v1/models/{model}",
            axum::routing::get(get_openai_model),
        )
        .route(
            "/openai/v1/chat/completions",
            axum::routing::post(relay_openai_chat),
        )
        .route(
            "/openai/v1/responses",
            axum::routing::post(relay_openai_responses),
        )
        .route(
            "/gemini/v1beta/models",
            axum::routing::get(list_gemini_models),
        )
        .route(
            "/gemini/v1beta/models/{*operation}",
            axum::routing::get(get_gemini_model).post(relay_gemini_generate_content),
        )
        .nest("/admin/api", admin_routes(state.clone()));

    let app = if index_file.exists() {
        app.fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(index_file)))
    } else {
        app.fallback(axum::routing::get(static_assets::serve_embedded_static))
    };

    app.layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
