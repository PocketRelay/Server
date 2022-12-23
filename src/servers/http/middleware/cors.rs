use axum::{
    http::{header, HeaderValue, Method, Request, StatusCode},
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
        let mut res = Response::default();
        *res.status_mut() = StatusCode::NO_CONTENT;
        let headers = res.headers_mut();
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("*"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("*"),
        );
        res
    } else {
        next.run(req).await
    };

    let headers = res.headers_mut();

    // Append access control allow origin header for all origins
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    res
}
