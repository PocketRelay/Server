use crate::{
    database::Player,
    state::GlobalState,
    utils::hashing::{hash_password, verify_password},
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/auth
pub fn router() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/create", post(create))
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Server error occurred")]
    ServerError,
    #[error("The provided credentials are invalid")]
    InvalidCredentails,
    #[error("The provided username is invalid")]
    InvalidUsername,
    #[error("The provided email is in use")]
    EmailTaken,
    #[error(
        "The provided email is for an origin account without a password ask an Admin to set one"
    )]
    OriginAccess,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    /// The email address of the account to login with
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
    let token = services.tokens.claim(player.id);

    Ok(Json(TokenResponse { token }))
}

#[derive(Deserialize)]
pub struct CreateRequest {
    username: String,
    /// The email address of the account to login with
    email: String,
    /// The plain-text password to login with
    password: String,
}

/// Route for creating accounts
async fn create(Json(req): Json<CreateRequest>) -> Result<Json<TokenResponse>, AuthError> {
    let db = GlobalState::database();

    // Validate the username is not empty
    if req.username.is_empty() {
        return Err(AuthError::InvalidUsername);
    }

    // Validate email taken status
    match Player::by_email(&db, &req.email).await {
        Ok(Some(_)) => return Err(AuthError::EmailTaken),
        Ok(None) => {}
        Err(_) => return Err(AuthError::ServerError),
    }

    let password = match hash_password(&req.password) {
        Ok(value) => value,
        Err(_) => return Err(AuthError::ServerError),
    };

    let player = Player::create(&db, req.email, req.username, Some(password))
        .await
        .map_err(|_| AuthError::ServerError)?;

    let services = GlobalState::services();
    let token = services.tokens.claim(player.id);

    Ok(Json(TokenResponse { token }))
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AuthError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::InvalidCredentails | AuthError::OriginAccess => StatusCode::UNAUTHORIZED,
            AuthError::EmailTaken | AuthError::InvalidUsername => StatusCode::BAD_REQUEST,
        };

        (status_code, self.to_string()).into_response()
    }
}
