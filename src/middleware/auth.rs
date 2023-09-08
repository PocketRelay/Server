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
use thiserror::Error;

pub struct Auth(pub Player);
pub struct AdminAuth(pub Player);

impl<S> FromRequestParts<S> for AdminAuth {
    type Rejection = TokenError;

    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut axum::http::request::Parts,
        state: &'b S,
    ) -> BoxFuture<'c, Result<Self, Self::Rejection>>
    where
        'a: 'c,
        'b: 'c,
        Self: 'c,
    {
        let auth = Auth::from_request_parts(parts, state);
        Box::pin(async move {
            let Auth(player) = auth.await?;
            if player.role < PlayerRole::Admin {
                return Err(TokenError::MissingRole);
            }
            Ok(AdminAuth(player))
        })
    }
}

/// The HTTP header that contains the authentication token
const TOKEN_HEADER: &str = "X-Token";

impl<S> FromRequestParts<S> for Auth {
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

            Ok(Self(player))
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
    /// Authentication is not high enough role
    #[error("Missing required role")]
    MissingRole,
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
            Self::MissingRole => StatusCode::FORBIDDEN,
            Self::Database(_) | Self::SessionService(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, boxed(self.to_string())).into_response()
    }
}
