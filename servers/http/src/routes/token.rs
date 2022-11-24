//! This module contains routes that handle the authentication tokens
//! for dealing with the server API

use crate::stores::token::TokenStore;
use actix_web::{
    delete,
    http::{header::ContentType, StatusCode},
    post,
    web::{Data, Json, ServiceConfig},
    HttpResponse, Responder, ResponseError,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, time::SystemTime};

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_token);
    cfg.service(delete_token);
}

/// Structure for possible errors that could happen
/// while attempting to access token routes
#[derive(Debug)]
enum TokenError {
    InvalidCredentials,
}

/// Result type alias for Json responses that could be a TokenError
type TokenResult<T> = Result<Json<T>, TokenError>;

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
    /// The time at which the token expires
    expiry_time: SystemTime,
}

/// Route for generating new tokens using a username and password to
/// authenticate with
///
/// `body`        The username and password request body
/// `token_store` The token store to create the token with
#[post("api/token")]
async fn get_token(
    body: Json<GetTokenRequest>,
    token_store: Data<TokenStore>,
) -> TokenResult<GetTokenResponse> {
    let (token, expiry_time) = token_store
        .authenticate(&body.username, &body.password)
        .await
        .ok_or_else(|| TokenError::InvalidCredentials)?;

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
#[delete("api/token")]
async fn delete_token(
    body: Json<DeleteTokenRequest>,
    token_store: Data<TokenStore>,
) -> impl Responder {
    token_store.remove_token(&body.token).await;
    HttpResponse::Ok()
        .content_type(ContentType::json())
        .finish()
}

impl Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCredentials => f.write_str("invalid credentials"),
        }
    }
}

impl ResponseError for TokenError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
        }
    }
}
