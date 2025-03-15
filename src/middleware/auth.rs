use crate::{
    config::Config,
    database::{
        entities::{players::PlayerRole, Player},
        DbErr,
    },
    services::sessions::{Sessions, VerifyError},
};
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DatabaseConnection;
use std::future::Future;
use std::sync::Arc;
use thiserror::Error;

pub struct MaybeAuth(pub Option<Player>);

impl<S> FromRequestParts<S> for MaybeAuth {
    type Rejection = TokenError;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let auth = Auth::from_request_parts(parts, state);
        Box::pin(async move {
            match auth.await {
                Ok(Auth(value)) => Ok(MaybeAuth(Some(value))),
                Err(TokenError::MissingToken) => Ok(MaybeAuth(None)),
                Err(err) => Err(err),
            }
        })
    }
}

pub struct Auth(pub Player);
pub struct AdminAuth(pub Player);

impl<S> FromRequestParts<S> for AdminAuth {
    type Rejection = TokenError;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
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

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let config = parts
            .extensions
            .get::<Arc<Config>>()
            .expect("config extension missing")
            .clone();
        let db = parts
            .extensions
            .get::<DatabaseConnection>()
            .expect("Database connection extension missing")
            .clone();
        let sessions = parts
            .extensions
            .get::<Arc<Sessions>>()
            .expect("Sessions extension missing");

        // Extract the token from the headers and verify it as a player id
        let player_id = parts
            .headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or(TokenError::MissingToken)
            .and_then(|token| {
                if let Some(config_user_id) = config.api.access_tokens.get(token) {
                    return Ok(*config_user_id);
                }

                sessions.verify_token(token).map_err(|err| match err {
                    VerifyError::Expired => TokenError::ExpiredToken,
                    VerifyError::Invalid => TokenError::InvalidToken,
                })
            });

        Box::pin(async move {
            let player_id = player_id?;

            let player = Player::by_id(&db, player_id)
                .await?
                .ok_or(TokenError::InvalidToken)?;

            Ok(Self(player))
        })
    }
}

/// Error type used by the token checking middleware to handle
/// different errors and create error response based on them
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
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Body::from(self.to_string())).into_response()
    }
}
