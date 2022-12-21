use axum::{
    extract::Path,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::{EmbeddedFile, RustEmbed};

use std::{ffi::OsStr, path::Path as StdPath};

/// Public resource content folder
#[derive(RustEmbed)]
#[folder = "src/resources/public"]
struct PublicContent;

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: Router) -> Router {
    router.route("/content/*filename", get(content))
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
