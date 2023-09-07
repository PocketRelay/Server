use crate::{
    database::entities::Player,
    services::tokens::Tokens,
    state::App,
    utils::hashing::{hash_password, verify_password},
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::{DatabaseConnection, DbErr};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that could occur while authenticating
#[derive(Debug, Error)]
pub enum AuthError {
    /// Database error
    #[error("Server error occurred")]
    Database(#[from] DbErr),

    /// Failed to hash the user password
    #[error("Server error occurred")]
    PasswordHash(#[from] argon2::password_hash::Error),

    /// Provided account credentials were invalid
    #[error("Provided credentials are not valid")]
    InvalidCredentails,

    /// Provided username was not valid
    #[error("Provided username is invalid")]
    InvalidUsername,

    /// Provided email was taken
    #[error("Provided email is in use")]
    EmailTaken,

    /// Account was an Origin account without a password
    #[error("Origin account password is not set")]
    OriginAccess,

    /// Server has disabled account creation on dashboard
    #[error("This server has disabled dashboard account registration")]
    RegistrationDisabled,
}

/// Response type alias for JSON responses with AuthError
type AuthRes<T> = Result<Json<T>, AuthError>;

/// Request structure for logging into an account using
/// an email and password
#[derive(Deserialize)]
pub struct LoginRequest {
    /// The email address of the account to login with
    email: String,
    /// The plain-text password to login with
    password: String,
}

/// Response containing a token for authentication
#[derive(Serialize)]
pub struct TokenResponse {
    /// Authentication token
    token: String,
}

/// POST /api/auth/login
///
/// Handles authenticating a user using a username and
/// password. Upon success will provide a [`TokenResponse`]
/// containing the authentication token for the user
pub async fn login(Json(req): Json<LoginRequest>) -> AuthRes<TokenResponse> {
    let LoginRequest { email, password } = req;

    let db: &DatabaseConnection = App::database();

    // Find a player with the matching email
    let player: Player = Player::by_email(db, &email)
        .await?
        .ok_or(AuthError::InvalidCredentails)?;

    // Find the account password or fail if missing one
    let player_password: &str = player.password.as_ref().ok_or(AuthError::OriginAccess)?;

    // Verify that the password matches
    if !verify_password(&password, player_password) {
        return Err(AuthError::InvalidCredentails);
    }

    Ok(player.into())
}

/// Request structure for creating a new account contains
/// the account credentials
#[derive(Deserialize)]
pub struct CreateRequest {
    /// The username to set for the account
    username: String,
    /// The email address of the account to login with
    email: String,
    /// The plain-text password to login with
    password: String,
}

/// POST /api/auth/create
///
/// Handles creating a new user from the provided credentials.
/// Upon success will provide a [`TokenResponse`] containing
/// the authentication token for the created user
pub async fn create(Json(req): Json<CreateRequest>) -> AuthRes<TokenResponse> {
    let config = App::config();
    if config.dashboard.disable_registration {
        return Err(AuthError::RegistrationDisabled);
    }

    let CreateRequest {
        username,
        email,
        password,
    } = req;

    // Validate the username is not empty
    if username.is_empty() {
        return Err(AuthError::InvalidUsername);
    }

    let db: &DatabaseConnection = App::database();

    // Validate email taken status
    if Player::by_email(db, &email).await?.is_some() {
        return Err(AuthError::EmailTaken);
    }

    let password: String = hash_password(&password)?;
    let player: Player = Player::create(db, email, username, Some(password), config).await?;

    Ok(player.into())
}

/// Allow conversion from player into JSON token response for simplicity
impl From<Player> for Json<TokenResponse> {
    fn from(value: Player) -> Self {
        // Claim a token and response with a response
        let token: String = Tokens::service_claim(value.id);
        Json(TokenResponse { token })
    }
}

/// Response implementation for auth errors
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Self::Database(_) | Self::PasswordHash(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidCredentails | Self::OriginAccess => StatusCode::UNAUTHORIZED,
            Self::EmailTaken | Self::InvalidUsername => StatusCode::BAD_REQUEST,
            Self::RegistrationDisabled => StatusCode::FORBIDDEN,
        };

        (status_code, self.to_string()).into_response()
    }
}
