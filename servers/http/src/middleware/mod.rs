use std::{
    fmt::Display,
    future::{ready, Ready},
    rc::Rc,
    sync::Arc,
};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    Error, ResponseError,
};
use futures_util::{future::LocalBoxFuture, FutureExt};
use std::task;

use crate::stores::token::TokenStore;

pub struct TokenAuth {
    store: Arc<TokenStore>,
}

impl TokenAuth {
    pub fn new(store: Arc<TokenStore>) -> Self {
        Self { store }
    }
}

impl<S, B> Transform<S, ServiceRequest> for TokenAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TokenAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TokenAuthMiddleware {
            service: Rc::new(service),
            store: self.store.clone(),
        }))
    }
}

pub struct TokenAuthMiddleware<S> {
    service: Rc<S>,
    store: Arc<TokenStore>,
}

const TOKEN_HEADER: &str = "X-Token";

#[derive(Debug)]
enum AuthError {
    MissingToken,
    InvalidToken,
    BadRequest,
}

impl Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingToken => "Missing token",
            Self::InvalidToken => "Invalid token",
            Self::BadRequest => "Bad request",
        })
    }
}

impl ResponseError for AuthError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::MissingToken => StatusCode::BAD_REQUEST,
            Self::InvalidToken => StatusCode::UNAUTHORIZED,
            Self::BadRequest => StatusCode::BAD_REQUEST,
        }
    }
}

impl<S, B> Service<ServiceRequest> for TokenAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&self, ctx: &mut task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx).map_err(|err| err.into())
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let store = self.store.clone();

        async move {
            let headers = req.headers();
            let token_header = headers.get(TOKEN_HEADER).ok_or(AuthError::MissingToken)?;
            let token = token_header.to_str().map_err(|_| AuthError::BadRequest)?;

            let is_valid = store.is_valid_token(token).await;
            if is_valid {
                service.call(req).await
            } else {
                Err(AuthError::InvalidToken.into())
            }
        }
        .boxed_local()
    }
}
