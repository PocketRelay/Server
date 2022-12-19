use axum::{
    http::{header, HeaderValue, Method, Request},
    middleware::Next,
    response::Response,
};

use hyper::StatusCode;

pub async fn cors_layer<T>(req: Request<T>, next: Next<T>) -> Result<Response, StatusCode> {
    if req.method() == Method::OPTIONS {
        let mut res = Response::new(Default::default());
        res.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
        return Ok(res);
    }

    let mut res = next.run(req).await;
    res.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );

    Ok(res)
}
