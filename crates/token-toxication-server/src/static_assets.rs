use axum::{
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};

#[cfg(not(debug_assertions))]
use axum::{body::Body, http::header};
#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;

#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "../../apps/admin/dist"]
struct AdminAssets;

#[cfg(not(debug_assertions))]
pub async fn serve_embedded_static(uri: Uri) -> Response {
    let path = static_path(uri.path());
    let Some(asset) = AdminAssets::get(path).or_else(|| AdminAssets::get("index.html")) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = mime_guess::from_path(path).first_or_octet_stream();
    Response::builder()
        .header(header::CONTENT_TYPE, content_type.as_ref())
        .body(Body::from(asset.data.into_owned()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(debug_assertions)]
pub async fn serve_embedded_static(_uri: Uri) -> Response {
    StatusCode::NOT_FOUND.into_response()
}

#[cfg(not(debug_assertions))]
fn static_path(path: &str) -> &str {
    let path = path.trim_start_matches('/').trim();
    if path.is_empty() || path.contains("..") {
        "index.html"
    } else {
        path
    }
}
