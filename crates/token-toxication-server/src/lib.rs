pub mod auth;
pub mod codex_subscription;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod openapi;
pub mod provider_catalog;
pub mod routes;
pub mod routing;
pub mod static_assets;

use std::{path::PathBuf, sync::Arc};

use axum::Router;
use chrono::{DateTime, Utc};
use config::Config;
use db::Db;
use routes::{
    admin_routes, get_anthropic_model, get_openai_model, health, list_anthropic_models,
    list_openai_models, metrics, relay_messages, relay_openai_chat, relay_openai_responses,
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
    pub started_at: DateTime<Utc>,
}

pub fn app(state: AppState, static_dir: PathBuf) -> Router {
    let index_file = static_dir.join("index.html");

    let app = Router::new()
        .route("/health", axum::routing::get(health))
        .route("/metrics", axum::routing::get(metrics))
        .route("/openapi.json", axum::routing::get(openapi::openapi_json))
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
