use std::{fmt::Display, sync::Arc};

use axum::{
    body::boxed,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::servers::http::stores::token::TokenStore;
use reqwest::StatusCode;

pub async fn guard_token_auth<T>(req: Request<T>, next: Next<T>) -> Result<Response, Response> {
    let Some(store)= req.extensions().get::<Arc<TokenStore>>() else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR
            .into_response())
    };

    /// The HTTP header that authentication tokens are stored in
    const TOKEN_HEADER: &str = "X-Token";

    // Obtain the token from the headers and convert to owned value
    let token = req
        .headers()
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_owned())
        .ok_or(TokenAuthError::MissingToken.into_response())?;

    let is_valid = store.is_valid_token(&token).await;
    if is_valid {
        Ok(next.run(req).await)
    } else {
        Err(TokenAuthError::InvalidToken.into_response())
    }
}

/// Error tyoe for
#[derive(Debug)]
enum TokenAuthError {
    MissingToken,
    InvalidToken,
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
