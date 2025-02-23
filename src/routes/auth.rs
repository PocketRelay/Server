use std::sync::Arc;

use crate::{
    config::Config,
    database::entities::{Player, PlayerRole},
    services::sessions::Sessions,
    session::{models::messaging::MessageNotify, packet::Packet},
    utils::{
        components::messaging,
        hashing::{hash_password, verify_password},
    },
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use log::error;
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
    InvalidCredentials,

    /// Provided account didn't exist
    #[error("No matching account")]
    NoMatchingAccount,

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

    /// Session is not active
    #[error("This player is not currently connected, please connect to the server and visit the main menu in-game before attempting this action.")]
    SessionNotActive,

    /// Failed to create login code
    #[error("Failed to generate login code")]
    FailedGenerateCode,

    /// Failed to create login code
    #[error("The provided login code was incorrect")]
    InvalidCode,
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
pub async fn login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Json(LoginRequest { email, password }): Json<LoginRequest>,
) -> AuthRes<TokenResponse> {
    // Find a player with the matching email
    let player: Player = Player::by_email(&db, &email)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    // Find the account password or fail if missing one
    let player_password: &str = player.password.as_ref().ok_or(AuthError::OriginAccess)?;

    // Verify that the password matches
    if !verify_password(&password, player_password) {
        return Err(AuthError::InvalidCredentials);
    }

    // Update last login timestamp
    if let Err(err) = Player::set_last_login(&db, player.id, Utc::now()).await {
        error!("failed to store last login time: {err}");
    }

    let token = sessions.create_token(player.id);
    Ok(Json(TokenResponse { token }))
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
pub async fn create(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<Config>>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Json(CreateRequest {
        username,
        email,
        password,
    }): Json<CreateRequest>,
) -> AuthRes<TokenResponse> {
    if config.dashboard.disable_registration {
        return Err(AuthError::RegistrationDisabled);
    }

    // Validate the username is not empty
    if username.is_empty() {
        return Err(AuthError::InvalidUsername);
    }

    // Validate email taken status
    if Player::by_email(&db, &email).await?.is_some() {
        return Err(AuthError::EmailTaken);
    }

    // Use the super admin role if the email is the super admins
    let role: PlayerRole = if config.dashboard.is_super_email(&email) {
        PlayerRole::SuperAdmin
    } else {
        PlayerRole::Default
    };

    let password: String = hash_password(&password)?;
    let player: Player = Player::create(&db, email, username, Some(password), role).await?;

    // Update last login timestamp
    if let Err(err) = Player::set_last_login(&db, player.id, Utc::now()).await {
        error!("failed to store last login time: {err}");
    }

    let token = sessions.create_token(player.id);
    Ok(Json(TokenResponse { token }))
}

/// Request structure for requesting a login code
#[derive(Deserialize)]
pub struct RequestLoginCodeRequest {
    /// The email address of the account to login with
    email: String,
}

/// POST /api/auth/request-code
///
/// Requests a login code be sent to a active session to be used
/// for logging in without a password
pub async fn handle_request_login_code(
    Extension(db): Extension<DatabaseConnection>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Json(RequestLoginCodeRequest { email }): Json<RequestLoginCodeRequest>,
) -> Result<StatusCode, AuthError> {
    // Player must exist
    let player = Player::by_email(&db, &email)
        .await?
        .ok_or(AuthError::NoMatchingAccount)?;

    // Session must be active
    let session = sessions
        .lookup_session(player.id)
        .ok_or(AuthError::SessionNotActive)?;

    // Generate the login code
    let login_code = sessions
        .create_login_code(player.id)
        .map_err(|_| AuthError::FailedGenerateCode)?;

    let small_message =
        format!("Login confirmation code: <font color='#FFFF66'>{login_code}</font>");
    let full_message = format!("Your login confirmation code is <font color='#FFFF66'>{login_code}</font>, enter this on the dashboard to login");

    // Create and serialize the message
    let origin_message = serde_json::to_string(&SystemMessage {
        title: "Login Confirmation Code".to_string(),
        message: full_message,
        image: "".to_string(),
        ty: 0,
        tracking_id: -1,
        priority: 1,
    })
    .map_err(|_| AuthError::FailedGenerateCode)?;

    // Craft the payload with  new lines so outdated clients don't see the JSON
    let game_message = format!("{small_message}\n[SYSTEM_TERMINAL]{origin_message}\n");

    let notify_origin = Packet::notify(
        messaging::COMPONENT,
        messaging::SEND_MESSAGE,
        MessageNotify {
            message: game_message,
            player_id: player.id,
        },
    );

    // Send the message
    session.notify_handle.notify(notify_origin);

    Ok(StatusCode::OK)
}

#[derive(Deserialize, Serialize)]
pub struct SystemMessage {
    title: String,
    message: String,
    image: String,
    ty: u8,
    tracking_id: i32,
    priority: i32,
}

/// Request structure for requesting a login code
#[derive(Deserialize)]
pub struct RequestExchangeLoginCode {
    /// The email address of the account to login with
    login_code: String,
}

/// POST /api/auth/exchange-code
///
/// Requests a login code be sent to a active session to be used
/// for logging in without a password
pub async fn handle_exchange_login_code(
    Extension(db): Extension<DatabaseConnection>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Json(RequestExchangeLoginCode { login_code }): Json<RequestExchangeLoginCode>,
) -> AuthRes<TokenResponse> {
    // Exchange the code for a token
    let (player_id, token) = sessions
        .exchange_login_code(&login_code)
        .ok_or(AuthError::InvalidCode)?;

    // Update last login timestamp
    if let Err(err) = Player::set_last_login(&db, player_id, Utc::now()).await {
        error!("failed to store last login time: {err}");
    }

    Ok(Json(TokenResponse { token }))
}

/// Response implementation for auth errors
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Self::Database(_) | Self::PasswordHash(_) | Self::FailedGenerateCode => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::InvalidCredentials | Self::OriginAccess => StatusCode::UNAUTHORIZED,
            Self::EmailTaken
            | Self::InvalidUsername
            | Self::SessionNotActive
            | Self::NoMatchingAccount
            | Self::InvalidCode => StatusCode::BAD_REQUEST,
            Self::RegistrationDisabled => StatusCode::FORBIDDEN,
        };

        (status_code, self.to_string()).into_response()
    }
}
