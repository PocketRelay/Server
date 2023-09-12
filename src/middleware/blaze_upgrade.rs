use axum::{
    extract::FromRequestParts,
    http::{Method, StatusCode},
    response::IntoResponse,
};
use futures_util::future::BoxFuture;
use hyper::upgrade::{OnUpgrade, Upgraded};
use std::future::ready;
use thiserror::Error;

/// Errors that could occur while upgrading
#[derive(Debug, Error)]
pub enum BlazeUpgradeError {
    #[error("Cannot upgrade not GET requests")]
    UnacceptableMethod,
    #[error("Failed to upgrade connection")]
    FailedUpgrade,
    #[error("Cannot upgrade connection")]
    CannotUpgrade,
}

/// Extractor for initiated the upgrade process for a request
pub struct BlazeUpgrade {
    /// The upgrade handle
    on_upgrade: OnUpgrade,
}

/// HTTP request upgraded into a Blaze socket along with
/// extra information
pub struct BlazeSocket(pub Upgraded);

impl BlazeUpgrade {
    /// Upgrades the underlying hook returning the newly created socket
    pub async fn upgrade(self) -> Result<BlazeSocket, BlazeUpgradeError> {
        // Attempt to upgrade the connection
        let upgrade = match self.on_upgrade.await {
            Ok(value) => value,
            Err(_) => return Err(BlazeUpgradeError::FailedUpgrade),
        };

        Ok(BlazeSocket(upgrade))
    }
}

impl<S> FromRequestParts<S> for BlazeUpgrade
where
    S: Send + Sync,
{
    type Rejection = BlazeUpgradeError;

    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut axum::http::request::Parts,
        _state: &'b S,
    ) -> BoxFuture<'c, Result<Self, Self::Rejection>>
    where
        'a: 'c,
        'b: 'c,
        Self: 'c,
    {
        // Ensure the method is GET
        if parts.method != Method::GET {
            return Box::pin(ready(Err(BlazeUpgradeError::UnacceptableMethod)));
        }

        // Get the upgrade hook
        let on_upgrade = match parts.extensions.remove::<OnUpgrade>() {
            Some(value) => value,
            None => return Box::pin(ready(Err(BlazeUpgradeError::CannotUpgrade))),
        };

        Box::pin(ready(Ok(Self { on_upgrade })))
    }
}

impl IntoResponse for BlazeUpgradeError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}
