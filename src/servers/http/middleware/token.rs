use crate::servers::http::stores::token::TokenStore;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::StatusCode,
    Error, ResponseError,
};
use std::fmt::Display;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task;

/// Structure for transformer that creates token
/// authentication middleware
pub struct TokenAuth {
    /// The token store
    store: Arc<TokenStore>,
}

impl TokenAuth {
    /// creates a new token auth transform with the provided
    /// token store.
    ///
    /// `store` The token store
    pub fn new(store: Arc<TokenStore>) -> Self {
        Self { store }
    }
}

/// Error tyoe for
#[derive(Debug)]
enum TokenAuthError {
    MissingToken,
    InvalidToken,
    BadRequest,
}

impl<S, B> Transform<S, ServiceRequest> for TokenAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TokenAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TokenAuthMiddleware {
            service: Rc::new(service),
            store: self.store.clone(),
        }))
    }
}

/// Middleware structure for token authentication. Contains the service
/// behind a reference counter and the token store
pub struct TokenAuthMiddleware<S> {
    /// The service
    service: Rc<S>,
    /// The token store
    store: Arc<TokenStore>,
}

impl<S> TokenAuthMiddleware<S> {
    /// The HTTP header that authentication tokens are stored in
    const TOKEN_HEADER: &str = "X-Token";
}

/// Service implementation for the Token Auth Middleware which checks the header
///
impl<S, B> Service<ServiceRequest> for TokenAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + 'static>>;

    /// Function for polling the readiness of the service. This is forwarded
    /// onto the underlying service readyness
    #[inline]
    fn poll_ready(&self, ctx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    /// Handles the actual middleware action of checking the header token and
    /// returning Errors or completing the underlying request for the response.
    ///
    /// `req` The service request
    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let store = self.store.clone();
        Box::pin(async move {
            let headers = req.headers();
            // Obtain the token from the headers
            let token_header = headers
                .get(Self::TOKEN_HEADER)
                .ok_or(TokenAuthError::MissingToken)?;
            let token = token_header
                .to_str()
                .map_err(|_| TokenAuthError::BadRequest)?;

            // Invalid tokens throw an error
            if !store.is_valid_token(token).await {
                return Err(TokenAuthError::InvalidToken.into());
            }

            // Valid tokens continue the request
            service.call(req).await
        })
    }
}

/// Display implementation for the Token Auth Error this will be displayed
/// as the error response message.
impl Display for TokenAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingToken => "Missing token",
            Self::InvalidToken => "Invalid token",
            Self::BadRequest => "Bad request",
        })
    }
}

/// Response error implementation to allow the TokenAuthError to be used
/// as error responses. The status codes for the different request codes
/// are implemented here to.
///
/// Missing token & Bad Request are both (400 Bad Request)
/// Invalid Token is (401 Unauthorized)
impl ResponseError for TokenAuthError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::MissingToken | Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::InvalidToken => StatusCode::UNAUTHORIZED,
        }
    }
}
