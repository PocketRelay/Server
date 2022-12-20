use axum::{
    http::{header, HeaderValue, Method, Request},
    middleware::Next,
    response::Response,
};

/// Middleware layer function for appending CORS headers to requests
/// and responding to options requests
///
/// `req`  The request to handle
/// `next` The next layer to use
pub async fn cors_layer<T>(req: Request<T>, next: Next<T>) -> Response {
    // Create a new response for OPTIONS requests
    let mut res: Response = if req.method() == Method::OPTIONS {
        // Default response for OPTIONS requests
        Response::default()
    } else {
        next.run(req).await
    };
    // Append access control allow origin header for all origins
    res.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    res
}
