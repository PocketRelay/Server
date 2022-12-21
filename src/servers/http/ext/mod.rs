use axum::{
    http::{
        header::{self, HeaderValue},
        StatusCode,
    },
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
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/xml"));
        response
    }
}

/// Trait implemented by error response types that have
/// different possible status codes
pub trait ErrorStatusCode {
    /// Function for retrieving the status code of
    /// the specific error
    fn status_code(&self) -> StatusCode;
}
