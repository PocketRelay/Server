use std::fmt::Display;

use actix_web::{
    http::StatusCode,
    post,
    web::{Data, Json, ServiceConfig},
    ResponseError,
};
use serde::{Deserialize, Serialize};

use crate::stores::token::TokenStore;

/// Function for configuring the services in this route
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(authenticate);
}

#[derive(Debug)]
struct AuthError;

#[derive(Deserialize)]
struct AuthRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    token: String,
}

#[post("api/auth")]
async fn authenticate(
    body: Json<AuthRequest>,
    token_store: Data<TokenStore>,
) -> Result<Json<AuthResponse>, AuthError> {
    let token = token_store
        .authenticate(&body.username, &body.password)
        .await;

    match token {
        Some(token) => Ok(Json(AuthResponse { token })),
        None => Err(AuthError),
    }
}

impl Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invalid credentails")
    }
}

impl ResponseError for AuthError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        StatusCode::UNAUTHORIZED
    }
}
