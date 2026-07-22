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
    let Some((path, asset)) = resolve_static_asset(uri.path(), AdminAssets::get) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    Response::builder()
        .header(header::CONTENT_TYPE, static_content_type(path))
        .body(Body::from(asset.data.into_owned()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(debug_assertions)]
pub async fn serve_embedded_static(_uri: Uri) -> Response {
    StatusCode::NOT_FOUND.into_response()
}

#[cfg(any(not(debug_assertions), test))]
fn static_path(path: &str) -> &str {
    let path = path.trim_start_matches('/').trim();
    if path.is_empty() || path.contains("..") {
        "index.html"
    } else {
        path
    }
}

#[cfg(any(not(debug_assertions), test))]
fn resolve_static_asset<Asset>(
    uri_path: &str,
    mut get: impl FnMut(&str) -> Option<Asset>,
) -> Option<(&str, Asset)> {
    let requested_path = static_path(uri_path);
    get(requested_path)
        .map(|asset| (requested_path, asset))
        .or_else(|| get("index.html").map(|asset| ("index.html", asset)))
}

#[cfg(any(not(debug_assertions), test))]
fn static_content_type(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensionless_deep_link_resolves_to_the_html_fallback() {
        let (path, contents) = resolve_static_asset("/accounts/acme", |path| {
            (path == "index.html").then_some("admin document")
        })
        .expect("the embedded admin document is available");

        assert_eq!(path, "index.html");
        assert_eq!(contents, "admin document");
        assert_eq!(static_content_type(path), "text/html");
    }

    #[test]
    fn existing_asset_keeps_its_requested_path() {
        let (path, contents) = resolve_static_asset("/assets/admin.js", |path| {
            (path == "assets/admin.js").then_some("bundle")
        })
        .expect("the embedded asset is available");

        assert_eq!(path, "assets/admin.js");
        assert_eq!(contents, "bundle");
    }

    #[cfg(not(debug_assertions))]
    #[tokio::test]
    async fn embedded_extensionless_deep_link_returns_the_html_entry_document() {
        let response = serve_embedded_static(Uri::from_static("/accounts/acme")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html")
        );
    }
}
