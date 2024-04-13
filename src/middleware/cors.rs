use axum::{
    body::Body,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use hyper::Request;

/// Middleware layer function for appending CORS headers to requests
/// and responding to options requests
///
/// `req`  The request to handle
/// `next` The next layer to use
pub async fn cors_layer(req: Request<Body>, next: Next) -> Response {
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

#[cfg(test)]
mod test {
    use super::cors_layer;
    use axum::{body::Body, middleware::from_fn, routing::get, Router};
    use hyper::{
        header::{
            ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
        },
        Method, Request, StatusCode,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_options() {
        let app = Router::new()
            .route("/", get(|| async {}))
            .layer(from_fn(cors_layer));

        let req = Request::builder()
            .uri("/")
            .method(Method::OPTIONS)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let headers = res.headers();
        let allowed_methods = headers
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .expect("Missing allowed methods header");
        assert_eq!(allowed_methods.to_str().unwrap(), "*");

        let allowed_headers = headers
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("Missing allowed headers header");
        assert_eq!(allowed_headers.to_str().unwrap(), "*");

        let allowed_origin = headers
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("Missing allowed origin header");
        assert_eq!(allowed_origin.to_str().unwrap(), "*");
    }

    #[tokio::test]
    async fn test_get() {
        let app = Router::new()
            .route("/", get(|| async {}))
            .layer(from_fn(cors_layer));

        let req = Request::builder()
            .uri("/")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);

        let headers = res.headers();

        let allowed_origin = headers
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("Missing allowed origin header");
        assert_eq!(allowed_origin.to_str().unwrap(), "*");
    }
}
