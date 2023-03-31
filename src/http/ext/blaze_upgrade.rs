use axum::{
    extract::FromRequestParts,
    http::{HeaderValue, Method, StatusCode},
    response::IntoResponse,
};
use hyper::{
    upgrade::{OnUpgrade, Upgraded},
    HeaderMap,
};
use std::future::ready;
use thiserror::Error;

use crate::{
    session::SessionHostTarget,
    utils::{models::Port, types::BoxFuture},
};

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
    host_target: SessionHostTarget,
}

/// HTTP request upgraded into a Blaze socket along with
/// extra information
pub struct BlazeSocket {
    /// The upgraded connection
    pub upgrade: Upgraded,

    pub host_target: SessionHostTarget,
}

#[derive(Default, Clone, Copy)]
pub enum BlazeScheme {
    /// HTTP Scheme (http://)
    #[default]
    Http,
    /// HTTPS Scheme (https://)
    Https,
}

impl BlazeScheme {
    /// Provides the default port used by the scheme
    fn default_port(&self) -> u16 {
        match self {
            BlazeScheme::Http => 80,
            BlazeScheme::Https => 443,
        }
    }

    /// Returns the scheme value
    pub fn value(&self) -> &'static str {
        match self {
            BlazeScheme::Http => "http://",
            BlazeScheme::Https => "https://",
        }
    }
}

impl From<&HeaderValue> for BlazeScheme {
    fn from(value: &HeaderValue) -> Self {
        match value.as_bytes() {
            b"https" => BlazeScheme::Https,
            _ => BlazeScheme::default(),
        }
    }
}

impl BlazeUpgrade {
    /// Upgrades the underlying hook returning the newly created socket
    pub async fn upgrade(self) -> Result<BlazeSocket, BlazeUpgradeError> {
        // Attempt to upgrade the connection
        let upgrade = match self.on_upgrade.await {
            Ok(value) => value,
            Err(_) => return Err(BlazeUpgradeError::FailedUpgrade),
        };

        Ok(BlazeSocket {
            upgrade,
            host_target: self.host_target,
        })
    }

    /// Extracts the blaze scheme header from the provided headers map
    /// returning the scheme. On failure will return the default scheme
    fn extract_scheme(headers: &HeaderMap) -> BlazeScheme {
        let header = match headers.get(HEADER_SCHEME) {
            Some(value) => value,
            None => return BlazeScheme::default(),
        };
        let scheme: BlazeScheme = header.into();
        scheme
    }

    /// Extracts the client port from the provided headers map.
    ///
    /// `headers` The header map
    fn extract_port(headers: &HeaderMap) -> Option<Port> {
        // Get the port header
        let header = headers.get(HEADER_PORT)?;
        // Convert the header to a string
        let header = header.to_str().ok()?;
        // Parse the header value
        header.parse().ok()
    }

    /// Extracts the host address from the provided headers map
    fn extract_host(headers: &HeaderMap) -> Option<String> {
        // Get the port header
        let header = headers.get(HEADER_HOST)?;
        // Convert the header to a string
        let header = header.to_str().ok()?;
        Some(header.to_string())
    }
}

/// Header for the Pocket Relay connection scheme used by the client
const HEADER_SCHEME: &str = "X-Pocket-Relay-Scheme";
/// Header for the Pocket Relay connection port used by the client
const HEADER_PORT: &str = "X-Pocket-Relay-Port";
/// Header for the Pocket Relay connection host used by the client
const HEADER_HOST: &str = "X-Pocket-Relay-Host";

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

        let headers = &parts.headers;

        // Get the client scheme header
        let scheme: BlazeScheme = BlazeUpgrade::extract_scheme(headers);

        // Get the client port header
        let port: Port = match BlazeUpgrade::extract_port(headers) {
            Some(value) => value,
            None => scheme.default_port(),
        };

        // Get the client host
        let host: String = match BlazeUpgrade::extract_host(headers) {
            Some(value) => value,
            None => return Box::pin(ready(Err(BlazeUpgradeError::CannotUpgrade))),
        };

        Box::pin(ready(Ok(Self {
            on_upgrade,
            host_target: SessionHostTarget { scheme, host, port },
        })))
    }
}

impl IntoResponse for BlazeUpgradeError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}
