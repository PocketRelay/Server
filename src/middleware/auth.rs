use crate::{
    database::{
        entities::{players::PlayerRole, Player},
        DbErr,
    },
    services::sessions::{Sessions, VerifyError, VerifyTokenMessage},
    utils::types::BoxFuture,
};
use axum::{
    body::boxed,
    extract::FromRequestParts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use interlink::prelude::{Link, LinkError};
use sea_orm::DatabaseConnection;
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
        let db = parts
            .extensions
            .get::<DatabaseConnection>()
            .expect("Database connection extension missing")
            .clone();
        let sessions = parts
            .extensions
            .get::<Link<Sessions>>()
            .expect("Database connection extension missing")
            .clone();

        Box::pin(async move {
            // Extract the token from the headers
            let token = parts
                .headers
                .get(TOKEN_HEADER)
                .and_then(|value| value.to_str().ok())
                .ok_or(TokenError::MissingToken)?;

            let player_id = sessions
                .send(VerifyTokenMessage(token.to_string()))
                .await
                .map_err(TokenError::SessionService)?
                .map_err(|err| match err {
                    VerifyError::Expired => TokenError::ExpiredToken,
                    VerifyError::Invalid => TokenError::InvalidToken,
                })?;

            let player = Player::by_id(&db, player_id)
                .await?
                .ok_or(TokenError::InvalidToken)?;

            Ok(Self(player, PhantomData))
        })
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
    /// Session service error
    #[error("Session service unavailable")]
    SessionService(LinkError),
}

/// IntoResponse implementation for TokenError to allow it to be
/// used within the result type as a error response
impl IntoResponse for TokenError {
    #[inline]
    fn into_response(self) -> Response {
        let status = match &self {
            Self::MissingToken => StatusCode::BAD_REQUEST,
            Self::InvalidToken | Self::ExpiredToken => StatusCode::UNAUTHORIZED,
            Self::Database(_) | Self::SessionService(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, boxed(self.to_string())).into_response()
    }
}
