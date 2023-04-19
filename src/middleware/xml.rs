use axum::{
    http::header::{HeaderValue, CONTENT_TYPE},
    response::{IntoResponse, Response},
};

/// Wrapping structure for creating XML respones from
/// a string value
pub struct Xml(pub String);

impl IntoResponse for Xml {
    fn into_response(self) -> Response {
        let mut response = self.0.into_response();
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("text/xml"));
        response
    }
}
