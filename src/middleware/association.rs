use crate::services::sessions::{AssociationId, Sessions};
use axum::{extract::FromRequestParts, response::IntoResponse};
use futures_util::future::BoxFuture;
use hyper::StatusCode;
use std::{future::ready, sync::Arc};

/// Extractor for retireving the association token from a request headers
pub struct Association(pub Option<AssociationId>);

/// The HTTP header that contains the association token
const TOKEN_HEADER: &str = "x-association";

impl<S> FromRequestParts<S> for Association {
    type Rejection = InvalidAssociation;

    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut axum::http::request::Parts,
        _state: &'b S,
    ) -> BoxFuture<'c, Result<Self, Self::Rejection>>
    where
        'a: 'c,
        'b: 'c,
        Self: 'c,
    {
        let sessions = parts
            .extensions
            .get::<Arc<Sessions>>()
            .expect("Sessions extension missing");

        let assocation_id = parts
            .headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|token| sessions.verify_assoc_token(token).ok());

        Box::pin(ready(Ok(Self(assocation_id))))
    }
}

/// Association token was invalid
pub struct InvalidAssociation;

impl IntoResponse for InvalidAssociation {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, "Invalid association token").into_response()
    }
}
