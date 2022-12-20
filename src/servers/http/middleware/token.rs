use crate::servers::http::{ext::ErrorStatusCode, stores::token::TokenStore};
use axum::{
    body::boxed,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{fmt::Display, sync::Arc};

/// The HTTP header that authentication tokens are stored in
const TOKEN_HEADER: &str = "X-Token";

/// Guarding middleware layer for ensuring that requests have a valid
/// authentication token in the X-Token header.
///
/// `req`  The request to handle
/// `next` The next layer to use
pub async fn token_auth_layer<T>(req: Request<T>, next: Next<T>) -> Result<Response, TokenError> {
    // Obtain the token store from the extensions
    let store = req
        .extensions()
        .get::<Arc<TokenStore>>()
        .ok_or(TokenError::MissingStore)?;

    // Obtain the token from the headers and convert to owned value
    let token = req
        .headers()
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_owned())
        .ok_or(TokenError::MissingToken)?;

    if store.is_valid_token(&token).await {
        Ok(next.run(req).await)
    } else {
        Err(TokenError::InvalidToken)
    }
}

/// Error type used by the token checking middleware to handle
/// different errors and create error respones based on them
pub enum TokenError {
    /// The token store extension was not provided to the layer
    MissingStore,
    /// The token header was not provided on the request
    MissingToken,
    /// The provided token was not a valid token
    InvalidToken,
}

/// Display implementation for the Token Auth Error this will be displayed
/// as the error response message.
impl Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingToken => "Missing token",
            Self::InvalidToken => "Invalid token",
            Self::MissingStore => "Route missing internal store",
        })
    }
}

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for TokenError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingToken => StatusCode::BAD_REQUEST,
            Self::InvalidToken => StatusCode::UNAUTHORIZED,
            Self::MissingStore => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// IntoResponse implementation for TokenError to allow it to be
/// used within the result type as a error response
impl IntoResponse for TokenError {
    #[inline]
    fn into_response(self) -> Response {
        (self.status_code(), boxed(self.to_string())).into_response()
    }
}
