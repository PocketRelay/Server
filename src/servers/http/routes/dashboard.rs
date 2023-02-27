use axum::{
    extract::Path,
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "src/resources/dashboard"]
struct DashboardContent;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /content
pub fn router() -> Router {
    Router::new()
        .route("/*filename", get(content))
        .fallback(serve_index)
}
/// Function for serving content from the embedded public
/// content. Directory structure matches the paths vistied
/// in this url.
///
/// `path` The path of the content to serve
async fn content(Path(path): Path<String>) -> Result<Response, StatusCode> {
    if let Some(file) = DashboardContent::get(&path) {
        use std::path::Path as StdPath;

        let path = StdPath::new(&path);
        let ext = match path.extension() {
            Some(ext) => ext.to_str(),
            None => None,
        };

        serve_file(ext, file)
    } else {
        serve_index().await
    }
}

async fn serve_index() -> Result<Response, StatusCode> {
    let index = DashboardContent::get("index.html").ok_or(StatusCode::NOT_FOUND)?;
    serve_file(Some("html"), index)
}

fn serve_file(ext: Option<&str>, file: EmbeddedFile) -> Result<Response, StatusCode> {
    // Required file extension content types
    let ext = match ext {
        Some(value) => match value {
            "html" => "text/html",
            "js" | "mjs" => "text/javascript",
            "json" => "application/json",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "webp" => "image/webp",
            "css" => "text/css",
            _ => "text/plain",
        },
        None => "text/plain",
    };

    let mut response = file.data.into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(ext));
    Ok(response)
}
