use std::{net::SocketAddr, sync::Arc};

use axum::{
    async_trait,
    body::boxed,
    extract::{rejection::ExtensionRejection, ConnectInfo, FromRequestParts},
    http::request::Parts,
    response::{IntoResponse, Response},
    Extension,
};
use hyper::{HeaderMap, StatusCode};
use log::warn;
use thiserror::Error;

use crate::config::RuntimeConfig;

/// Middleware for extracting the server public address
pub struct IpAddress(pub SocketAddr);

const REAL_IP_HEADER: &str = "X-Real-IP";

#[async_trait]
impl<S> FromRequestParts<S> for IpAddress
where
    S: Send + Sync,
{
    type Rejection = IpAddressError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let config = parts
            .extensions
            .get::<Arc<RuntimeConfig>>()
            .expect("Missing runtime config");

        let reverse_proxy = config.reverse_proxy;
        if reverse_proxy {
            let ip = match extract_ip_header(&parts.headers) {
                Some(ip) => ip,
                None => {
                    warn!("Failed to extract X-Real-IP header from connecting client. If you are NOT using a reverse proxy\n\
                    disable the `reverse_proxy` config property, otherwise check that your reverse proxy is configured\n\
                    correctly according the guide. (Closing connection with error)");
                    return Err(IpAddressError::InvalidOrMissing);
                }
            };
            return Ok(Self(ip));
        }
        let value = Extension::<ConnectInfo<SocketAddr>>::from_request_parts(parts, state).await?;
        Ok(Self(value.0 .0))
    }
}

fn extract_ip_header(headers: &HeaderMap) -> Option<SocketAddr> {
    let header = headers.get(REAL_IP_HEADER)?;
    let value = header.to_str().ok()?;
    value.parse().ok()
}

/// Error type used by the token checking middleware to handle
/// different errors and create error respones based on them
#[derive(Debug, Error)]
pub enum IpAddressError {
    #[error(transparent)]
    ConnectInfo(#[from] ExtensionRejection),
    #[error("X-Real-IP header is invalid or missing")]
    InvalidOrMissing,
}

/// IntoResponse implementation for TokenError to allow it to be
/// used within the result type as a error response
impl IntoResponse for IpAddressError {
    #[inline]
    fn into_response(self) -> Response {
        let status: StatusCode = match self {
            IpAddressError::ConnectInfo(err) => return err.into_response(),
            _ => StatusCode::BAD_REQUEST,
        };
        (status, boxed(self.to_string())).into_response()
    }
}
