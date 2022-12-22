use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::{EmbeddedFile, RustEmbed};

/// Public resource content folder
#[derive(RustEmbed)]
#[folder = "src/resources/public"]
struct PublicContent;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /content
pub(super) fn router() -> Router {
    Router::new().route("/*filename", get(content))
}

/// Function for serving content from the embedded public
/// content. Directory structure matches the paths vistied
/// in this url.
///
/// `path` The path of the content to serve
async fn content(Path(path): Path<String>) -> Result<Response, StatusCode> {
    // Obtain the embedded file
    let file: EmbeddedFile = PublicContent::get(&path).ok_or(StatusCode::NOT_FOUND)?;
    // Create the response from the raw binary data
    let res: Response = file.data.into_response();
    Ok(res)
}
