use crate::{servers::http::ext::ErrorStatusCode, state::GlobalState, utils::types::BoxFuture};
use axum::{
    body::boxed,
    extract::FromRequestParts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use database::{DbErr, Player, PlayerRole};
use futures::FutureExt;
use jsonwebtoken::errors::ErrorKind;
use std::marker::PhantomData;
use thiserror::Error;

/// Extractor for extracting authentication from a request
/// authorization header Bearer token
pub struct Auth<V: AuthVerifier = ()>(pub Player, PhantomData<V>);

impl<V: AuthVerifier> Auth<V> {
    /// Converts the auth guard into its inner player
    pub fn into_inner(self) -> Player {
        self.0
    }
}

/// Alias for an auth gaurd using admin verification
pub type AdminAuth = Auth<AdminVerify>;

pub trait AuthVerifier {
    /// Verify function for checking that the provided
    /// player meets the requirements
    fn verify(player: &Player) -> bool;
}

/// Unit auth verifier type for accepting any player
impl AuthVerifier for () {
    fn verify(_player: &Player) -> bool {
        true
    }
}

/// Auth verifier implementation requiring a role of
/// Admin or higher
pub struct AdminVerify;

impl AuthVerifier for AdminVerify {
    fn verify(player: &Player) -> bool {
        player.role >= PlayerRole::Admin
    }
}

/// The HTTP header that contains the authentication token
const TOKEN_HEADER: &str = "X-Token";

impl<V: AuthVerifier, S> FromRequestParts<S> for Auth<V> {
    type Rejection = TokenError;

    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut axum::http::request::Parts,
        _state: &'b S,
    ) -> BoxFuture<'c, Result<Self, Self::Rejection>>
    where
        'a: 'c,
        'b: 'c,
        Self: 'c,
    {
        async move {
            // Extract the token from the headers
            let token = parts
                .headers
                .get(TOKEN_HEADER)
                .and_then(|value| value.to_str().ok())
                .ok_or(TokenError::MissingToken)?;

            // Verify the token claim
            let services = GlobalState::services();
            let claim = services.jwt.verify(token)?;

            // Load the claimed player
            let db = GlobalState::database();
            let player = Player::by_id(&db, claim.id)
                .await?
                .ok_or(TokenError::InvalidToken)?;

            Ok(Self(player, PhantomData))
        }
        .boxed()
    }
}

/// Error type used by the token checking middleware to handle
/// different errors and create error respones based on them
#[derive(Debug, Error)]
pub enum TokenError {
    /// The token was expired
    #[error("Expired token")]
    ExpiredToken,
    /// The token header was not provided on the request
    #[error("Missing token")]
    MissingToken,
    /// The provided token was not a valid token
    #[error("Invalid token")]
    InvalidToken,
    /// Database error
    #[error("Internal server error")]
    Database(#[from] DbErr),
}

impl From<jsonwebtoken::errors::Error> for TokenError {
    fn from(value: jsonwebtoken::errors::Error) -> Self {
        match value.into_kind() {
            ErrorKind::ExpiredSignature => Self::ExpiredToken,
            _ => Self::InvalidToken,
        }
    }
}

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for TokenError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingToken => StatusCode::BAD_REQUEST,
            Self::InvalidToken | Self::ExpiredToken => StatusCode::UNAUTHORIZED,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
