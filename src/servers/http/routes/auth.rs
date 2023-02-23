//! This module contains routes that handle the authentication tokens
//! for dealing with the server API

use crate::{state::GlobalState, utils::hashing::verify_password};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use database::Player;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use validator::Validate;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/auth
pub fn router() -> Router {
    Router::new().route("/login", get(login))
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Server error occurred")]
    ServerError,
    #[error("The provided credentials are invalid")]
    InvalidCredentails,
    #[error(
        "The provided email is for an origin account without a password ask an Admin to set one"
    )]
    OriginAccess,
}

#[derive(Deserialize, Validate)]
pub struct LoginRequest {
    /// The email address of the account to login with
    #[validate(email)]
    email: String,
    /// The plain-text password to login with
    password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    token: String,
}

/// Route for logging into a non origin account
async fn login(Json(req): Json<LoginRequest>) -> Result<Json<TokenResponse>, AuthError> {
    let db = GlobalState::database();
    let player = Player::by_email(&db, &req.email)
        .await
        .map_err(|_| AuthError::ServerError)?
        .ok_or(AuthError::InvalidCredentails)?;

    let password = match &player.password {
        Some(value) => value,
        None => return Err(AuthError::OriginAccess),
    };

    if !verify_password(&req.password, password) {
        return Err(AuthError::InvalidCredentails);
    }

    let services = GlobalState::services();

    let token = services
        .jwt
        .claim(player.id)
        .map_err(|_| AuthError::ServerError)?;

    Ok(Json(TokenResponse { token }))
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AuthError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::InvalidCredentails | AuthError::OriginAccess => StatusCode::UNAUTHORIZED,
        };

        (status_code, self.to_string()).into_response()
    }
}
