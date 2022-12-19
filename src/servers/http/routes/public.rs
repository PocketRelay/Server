use axum::{
    extract::Path,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;

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
async fn content(Path(path): Path<String>) -> Response {
    if let Some(file) = PublicContent::get(&path) {
        let mut response = file.data.into_response();
        if let Ok(header_value) =
            HeaderValue::from_str(mime_guess::from_path(&path).first_or_text_plain().as_ref())
        {
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, header_value);
        }

        response
    } else {
        (StatusCode::NOT_FOUND, "Not Found").into_response()
    }
}
