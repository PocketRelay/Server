use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, Method, StatusCode},
    response::IntoResponse,
};
use hyper::upgrade::OnUpgrade;
use thiserror::Error;

/// Errors that could occur while upgrading
#[derive(Debug, Error)]
pub enum UpgradeError {
    #[error("Request method must be `GET`")]
    UnacceptableMethod,
    #[error("Request couldn't be upgraded since no upgrade state was present")]
    ConnectionNotUpgradable,
}

/// Extractor for extracting the [OnUpgrade] from requests
/// to upgrade the connection
pub struct Upgrade(pub OnUpgrade);

#[async_trait]
impl<S> FromRequestParts<S> for Upgrade
where
    S: Send + Sync,
{
    type Rejection = UpgradeError;

    async fn from_request_parts(req: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Ensure the method is GET
        if req.method != Method::GET {
            return Err(UpgradeError::UnacceptableMethod);
        }

        req.extensions
            .remove::<OnUpgrade>()
            .ok_or(UpgradeError::ConnectionNotUpgradable)
            .map(Self)
    }
}

impl IntoResponse for UpgradeError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            UpgradeError::UnacceptableMethod => StatusCode::METHOD_NOT_ALLOWED,
            UpgradeError::ConnectionNotUpgradable => StatusCode::UPGRADE_REQUIRED,
        };

        (status, self.to_string()).into_response()
    }
}
