//! This module contains routes that handle the authentication tokens
//! for dealing with the server API

use crate::state::GlobalState;
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
    // .route("/register", get(register))
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Email address already in use")]
    EmailTaken,
    #[error("Invalid email address")]
    InvalidEmail,
    #[error("Server error occurred")]
    ServerError,
    #[error("The provided credentials are invalid")]
    InvalidCredentails,
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
    let player = Player::by_email(&db, &req.email, false)
        .await
        .map_err(|_| AuthError::ServerError)?
        .ok_or(AuthError::InvalidCredentails)?;

    let jwt = GlobalState::jwt();

    let token = jwt.claim(&player).map_err(|_| AuthError::ServerError)?;

    Ok(Json(TokenResponse { token }))
}

// #[derive(Deserialize, Validate)]
// pub struct RegisterRequest {
//     username: String,
//     /// The email address of the account to login with
//     #[validate(email)]
//     email: String,
//     /// The plain-text password to login with
//     password: String,
// }

// /// Route for logging into a non origin account
// async fn register(Json(req): Json<RegisterRequest>) -> Result<Json<TokenResponse>, AuthError> {
//     let db = GlobalState::database();
//     let player = Player::by_email(&db, &req.email, false)
//         .await
//         .map_err(|_| AuthError::ServerError)?
//         .ok_or(AuthError::InvalidCredentails)?;

//     let jwt = GlobalState::jwt();

//     let token = jwt.claim(&player).map_err(|_| AuthError::ServerError)?;

//     Ok(Json(TokenResponse { token }))
// }

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AuthError::EmailTaken => StatusCode::CONFLICT,
            AuthError::InvalidEmail => StatusCode::BAD_REQUEST,
            AuthError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::InvalidCredentails => StatusCode::UNAUTHORIZED,
        };

        (status_code, self.to_string()).into_response()
    }
}
