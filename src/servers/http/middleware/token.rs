use std::{fmt::Display, sync::Arc};

use axum::{
    body::{boxed, BoxBody, HttpBody},
    http::Request,
    response::{IntoResponse, Response},
};
use futures_util::future::BoxFuture;
use reqwest::StatusCode;
use tower::{Layer, Service};

use crate::servers::http::stores::token::TokenStore;

/// Layer for providing token based authentication middleware
pub struct TokenAuthLayer {
    /// The token store for authenticating
    store: Arc<TokenStore>,
}

impl<S> Layer<S> for TokenAuthLayer {
    type Service = TokenAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TokenAuthService {
            store: self.store.clone(),
            inner,
        }
    }
}

/// Service for providing token authentication wrapping
/// over the inner service
pub struct TokenAuthService<S> {
    store: Arc<TokenStore>,
    inner: S,
}

impl<S> TokenAuthService<S> {
    /// The HTTP header that authentication tokens are stored in
    const TOKEN_HEADER: &str = "X-Token";
}

/// Error tyoe for
#[derive(Debug)]
enum TokenAuthError {
    MissingToken,
    InvalidToken,
}

impl<S, B, R> Service<Request<B>> for TokenAuthService<S>
where
    S: Service<Request<B>, Response = Response<R>>,
    R: HttpBody + Send + 'static,
{
    type Response = Response<BoxBody>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        // Obtain the token from the headers and convert to owned value
        let token = req
            .headers()
            .get(Self::TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_owned());

        // Create the future for the underlying result using the service
        let res = self.inner.call(req);

        // Obtain arc clone of the store for use in the pin box
        let store = self.store.clone();

        Box::pin(async move {
            if let Some(token) = token {
                let is_valid = store.is_valid_token(&token).await;
                if is_valid {
                    // Valid tokens continue the request
                    let res = res.await?;
                    let res = res.map(|value| boxed(value));
                    Ok(res)
                } else {
                    // Invalid tokens throw an error
                    Ok(TokenAuthError::InvalidToken.into_response())
                }
            } else {
                Ok(TokenAuthError::MissingToken.into_response())
            }
        })
    }
}

/// Display implementation for the Token Auth Error this will be displayed
/// as the error response message.
impl Display for TokenAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingToken => "Missing token",
            Self::InvalidToken => "Invalid token",
        })
    }
}

impl TokenAuthError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingToken => StatusCode::BAD_REQUEST,
            Self::InvalidToken => StatusCode::UNAUTHORIZED,
        }
    }
}

impl IntoResponse for TokenAuthError {
    fn into_response(self) -> Response {
        (self.status_code(), boxed(self.to_string())).into_response()
    }
}
