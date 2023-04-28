use axum::{
    body::Full,
    http::{HeaderValue, Request},
    response::{IntoResponse, Response},
};
use embeddy::Embedded;
use hyper::{header::CONTENT_TYPE, StatusCode};
use std::{
    borrow::Cow,
    convert::Infallible,
    future::{ready, Ready},
    path::Path,
    task::{Context, Poll},
};
use tower::Service;

/// Resources embedded from the public data folder such as the
/// dashboard static assets and the content for the ingame store.
///
/// Also acts a service for publicly sharing the content
///
/// TODO: This may not be particularly performant with a match statement
/// over all the public assets
#[derive(Clone, Embedded)]
#[folder = "src/resources/public"]
pub struct PublicContent;

impl<T> Service<Request<T>> for PublicContent {
    type Response = Response;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<T>) -> Self::Future {
        let mut path = req.uri().path();
        let std_path = Path::new(path);

        // Determine type using extension
        let extension: Cow<'_, str> = match std_path.extension() {
            // Extract the extension lossily
            Some(value) => value.to_string_lossy(),
            // Use the index file when responding to paths (For SPA dashboard support)
            None => {
                path = "index.html";
                Cow::Borrowed("html")
            }
        };

        // Strip the leading slash in order to match paths correctly
        let path = match path.strip_prefix('/') {
            Some(value) => value,
            None => path,
        };

        // Create the response message
        let response = match Self::get(path) {
            // File exists, serve it with its known extension
            Some(file) => {
                // Guess mime type from file extension
                let mime_type: &'static str = match extension.as_ref() {
                    "html" => "text/html",
                    "js" | "mjs" => "text/javascript",
                    "json" => "application/json",
                    "woff" => "font/woff",
                    "woff2" => "font/woff2",
                    "webp" => "image/webp",
                    "css" => "text/css",
                    _ => "text/plain",
                };

                // Create byte reponse from the embedded file
                let mut response = Full::from(file).into_response();
                response
                    .headers_mut()
                    .insert(CONTENT_TYPE, HeaderValue::from_static(mime_type));
                response
            }
            // File not found 404
            None => StatusCode::NOT_FOUND.into_response(),
        };

        ready(Ok(response))
    }
}
