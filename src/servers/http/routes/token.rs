//! This module contains routes that handle the authentication tokens
//! for dealing with the server API

use crate::servers::http::stores::token::TokenStore;
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/token
pub(super) fn router() -> Router {
    Router::new().route(
        "/",
        get(validate_token).post(get_token).delete(delete_token),
    )
}

/// Error type for invalid credentials being provided to the
/// get_token route
struct InvalidCredentails;

/// Request structure for requesting a new token to be
/// generated for the session.
#[derive(Deserialize)]
struct GetTokenRequest {
    /// The username to authenticate with
    username: String,
    /// The password to authenticate with
    password: String,
}

/// Response for a successful authentication attempt contains the
/// session token and the time the token will expire at.
#[derive(Serialize)]
struct GetTokenResponse {
    /// The generated token
    token: String,
    /// The time at which the token expires (Seconds since unix epoch)
    expiry_time: u64,
}

/// Route for generating new tokens using a username and password to
/// authenticate with
///
/// `body`        The username and password request body
/// `token_store` The token store to create the token with
async fn get_token(
    Extension(token_store): Extension<Arc<TokenStore>>,
    Json(body): Json<GetTokenRequest>,
) -> Result<Json<GetTokenResponse>, InvalidCredentails> {
    let (token, expiry_time): (String, SystemTime) = token_store
        .authenticate(&body.username, &body.password)
        .await
        .ok_or(InvalidCredentails)?;

    let expiry_time = expiry_time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();

    Ok(Json(GetTokenResponse { token, expiry_time }))
}

/// Request structure for a request to delete a token
/// from the token store
#[derive(Deserialize)]
struct DeleteTokenRequest {
    /// The token to delete from the token store
    token: String,
}

/// Route for deleting existing tokens from the token store
///
/// `body`        The token request body
/// `token_store` The token store to remove the token from
async fn delete_token(
    Extension(token_store): Extension<Arc<TokenStore>>,
    Json(body): Json<DeleteTokenRequest>,
) -> Response {
    token_store.remove_token(&body.token).await;
    StatusCode::OK.into_response()
}

/// Query structure for a query to validate a token
#[derive(Deserialize)]
struct ValidateTokenQuery {
    /// The token to validate
    token: String,
}

/// Response for a token validity request contains whether the token is
/// valid and the expiry time of the token if its valid
#[derive(Serialize)]
struct ValidateTokenResponse {
    /// Whether the token is valid or not
    valid: bool,
    /// The time at which the token expires (Seconds since unix epoch)
    expiry_time: Option<u64>,
}

/// Route for validating a token. Used to check if a token is valid and
/// retrieve the expiry time of the token if the token is valid.
///
/// `token`       The token query containing the token
/// `token_store` The token store to validate with
async fn validate_token(
    Extension(token_store): Extension<Arc<TokenStore>>,
    Query(token): Query<ValidateTokenQuery>,
) -> Json<ValidateTokenResponse> {
    let expiry = token_store.get_token_expiry(&token.token).await;

    let (valid, expiry_time) = match expiry {
        Some(value) => {
            let expiry_time = value
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            (true, Some(expiry_time))
        }
        None => (false, None),
    };

    Json(ValidateTokenResponse { valid, expiry_time })
}

/// IntoResponse implementation for InvalidCredentails to allow it to be
/// used within the result type as a error response
impl IntoResponse for InvalidCredentails {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, "InvalidCredentails").into_response()
    }
}
