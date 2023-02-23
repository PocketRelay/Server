use crate::{servers::http::ext::ErrorStatusCode, state::GlobalState, utils::types::BoxFuture};
use axum::{
    body::boxed,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, StatusCode},
    response::{IntoResponse, Response},
};
use database::{Player, PlayerRole};
use futures::FutureExt;
use jsonwebtoken::errors::ErrorKind;
use std::{fmt::Display, marker::PhantomData};

/// Extractor for extracting authentication from a request
/// authorization header Bearer token
pub struct Auth<V: AuthVerifier = ()>(pub Player, PhantomData<V>);

impl<V: AuthVerifier> Auth<V> {
    pub fn into_inner(self) -> Player {
        self.0
    }
}

pub type AdminAuth = Auth<AdminVerify>;

pub trait AuthVerifier {
    /// Verify function for checking that the provided
    /// player meets the requirements
    fn verify(player: &Player) -> bool;
}

impl AuthVerifier for () {
    fn verify(_player: &Player) -> bool {
        true
    }
}

pub struct AdminVerify;

impl AuthVerifier for AdminVerify {
    fn verify(player: &Player) -> bool {
        player.role >= PlayerRole::Admin
    }
}

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
            let token = parts
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(TokenError::MissingToken)?;

            let (schema, token) = token.split_once(' ').ok_or(TokenError::InvalidToken)?;

            if schema != "Bearer" {
                return Err(TokenError::InvalidToken);
            }

            let services = GlobalState::services();

            let claim = match services.jwt.verify(token) {
                Ok(value) => value,
                Err(err) => {
                    return Err(match err.kind() {
                        ErrorKind::ExpiredSignature => TokenError::ExpiredToken,
                        _ => TokenError::InvalidToken,
                    })
                }
            };

            let db = GlobalState::database();
            let player = Player::by_id(&db, claim.id)
                .await
                .map_err(|_| TokenError::Server)?;
            let player = player.ok_or(TokenError::InvalidToken)?;

            Ok(Self(player, PhantomData))
        }
        .boxed()
    }
}

/// Error type used by the token checking middleware to handle
/// different errors and create error respones based on them
pub enum TokenError {
    /// The token was expired
    ExpiredToken,
    /// The token header was not provided on the request
    MissingToken,
    /// The provided token was not a valid token
    InvalidToken,
    /// Server error occurred
    Server,
}

/// Display implementation for the TokenError this will be displayed
/// as the error response message.
impl Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingToken => "Missing token",
            Self::InvalidToken => "Invalid token",
            Self::ExpiredToken => "Expired token",
            Self::Server => "Internal server error",
        })
    }
}

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for TokenError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MissingToken => StatusCode::BAD_REQUEST,
            Self::InvalidToken | Self::ExpiredToken => StatusCode::UNAUTHORIZED,
            Self::Server => StatusCode::INTERNAL_SERVER_ERROR,
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
