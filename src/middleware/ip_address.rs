use crate::config::RuntimeConfig;
use axum::{
    async_trait,
    body::boxed,
    extract::{rejection::ExtensionRejection, ConnectInfo, FromRequestParts},
    http::request::Parts,
    response::{IntoResponse, Response},
    Extension,
};
use hyper::{header::ToStrError, HeaderMap, StatusCode};
use log::warn;
use std::{
    net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use thiserror::Error;

/// Middleware that extracts the IP address of the connection
pub struct IpAddress(pub Ipv4Addr);

/// Header used to extract the real client IP address, provided by the reverse proxy
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

        // Reverse proxies should respect the X-Real-IP header
        if config.reverse_proxy {
            let ip = match extract_ip_header(&parts.headers) {
                Ok(ip) => ip,
                Err(err) => {
                    warn!("Failed to extract X-Real-IP header from incoming request. If you are NOT using a reverse proxy\n\
                    disable the `reverse_proxy` config property, otherwise check that your reverse proxy is configured\n\
                    correctly according the guide. (Closing connection with error) cause: {}", err);
                    return Err(err);
                }
            };
            return Ok(Self(ip));
        }

        let Extension(ConnectInfo(addr)) =
            Extension::<ConnectInfo<SocketAddr>>::from_request_parts(parts, state).await?;
        let addr = try_socket_address(addr)?;
        Ok(Self(addr))
    }
}

/// Attempts to extract the value of the X-Real-IP header provided
/// by reverse proxies
fn extract_ip_header(headers: &HeaderMap) -> Result<Ipv4Addr, IpAddressError> {
    let header = headers
        .get(REAL_IP_HEADER)
        .ok_or(IpAddressError::MissingHeader)?;

    let value = header.to_str()?;

    // Attempt to parse as IP address first (address)
    if let Ok(addr) = value.parse::<IpAddr>() {
        return match addr {
            IpAddr::V4(addr) => Ok(addr),
            IpAddr::V6(_) => Err(IpAddressError::Unsupported),
        };
    }

    // Fallback attempt to parse as a socket address (address:port)
    let addr = value.parse::<SocketAddr>()?;
    try_socket_address(addr)
}

/// Attempts to extract an [Ipv4Addr] from the provided socket address
/// returning an error if the [SocketAddr] isn't an IPv4 addr
fn try_socket_address(addr: SocketAddr) -> Result<Ipv4Addr, IpAddressError> {
    match addr {
        SocketAddr::V4(addr) => Ok(*addr.ip()),
        SocketAddr::V6(_) => Err(IpAddressError::Unsupported),
    }
}

/// Error type used by the token checking middleware to handle
/// different errors and create error respones based on them
#[derive(Debug, Error)]
pub enum IpAddressError {
    /// Fallback extraction attempt failed
    #[error(transparent)]
    ConnectInfo(#[from] ExtensionRejection),

    /// Header wasn't present on the request
    #[error("X-Real-IP header is missing")]
    MissingHeader,

    /// Header contained non ASCII characters
    #[error("Header X-Real-IP contained unexpected characters")]
    InvalidHeader(#[from] ToStrError),

    /// Header couldn't be parsed as an address`
    #[error("Failed to parse X-Real-IP: {0}")]
    ParsingFailed(#[from] AddrParseError),

    /// Header contained an IPv6 address but only IPv4 can be used by ME3
    #[error("Server was provided IPv6 address but only IPv4 is supported")]
    Unsupported,
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

#[cfg(test)]
mod test {
    use super::{extract_ip_header, IpAddress, IpAddressError, REAL_IP_HEADER};
    use crate::config::RuntimeConfig;
    use axum::{
        extract::{ConnectInfo, FromRequestParts},
        http::HeaderValue,
    };
    use hyper::{HeaderMap, Request};
    use std::{
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        sync::Arc,
    };

    /// Tests that IPv4 addresses can be extracted correctly
    /// from the header
    #[test]
    fn test_valid_ips() {
        let values = [
            ("127.0.0.1", Ipv4Addr::new(127, 0, 0, 1)),
            ("0.0.0.0", Ipv4Addr::new(0, 0, 0, 0)),
            ("1.1.1.1", Ipv4Addr::new(1, 1, 1, 1)),
            ("192.168.0.1", Ipv4Addr::new(192, 168, 0, 1)),
            ("10.168.1.0", Ipv4Addr::new(10, 168, 1, 0)),
        ];

        for (header, expected) in values {
            let mut headers = HeaderMap::new();
            headers.insert(REAL_IP_HEADER, HeaderValue::from_static(header));

            let value = extract_ip_header(&headers).unwrap();
            assert_eq!(value, expected)
        }
    }

    /// Tests that IPv4 socket addresses can be parsed and extracted as [Ipv4Addr]s
    /// without any issue
    #[test]
    fn test_socket_fallback() {
        let values = [
            ("127.0.0.1:80", Ipv4Addr::new(127, 0, 0, 1)),
            ("0.0.0.0:80", Ipv4Addr::new(0, 0, 0, 0)),
            ("1.1.1.1:443", Ipv4Addr::new(1, 1, 1, 1)),
            ("192.168.0.1:230", Ipv4Addr::new(192, 168, 0, 1)),
            ("10.168.1.0:5900", Ipv4Addr::new(10, 168, 1, 0)),
        ];

        for (header, expected) in values {
            let mut headers = HeaderMap::new();
            headers.insert(REAL_IP_HEADER, HeaderValue::from_static(header));

            let value = extract_ip_header(&headers).unwrap();
            assert_eq!(value, expected)
        }
    }

    /// Tests that malformed headers result in an error
    #[test]
    fn test_malformed_addr() {
        let mut headers = HeaderMap::new();
        headers.insert(REAL_IP_HEADER, HeaderValue::from_static("malformed"));

        let value = extract_ip_header(&headers).unwrap_err();
        assert!(matches!(value, IpAddressError::ParsingFailed(_)))
    }

    /// Tests that IPv6 headers result in an error
    #[test]
    fn test_ipv6_addr() {
        let mut headers = HeaderMap::new();
        headers.insert(
            REAL_IP_HEADER,
            HeaderValue::from_static("b44e:2ae1:f85e:2381:7a67:fb1e:2ffd:c053"),
        );

        let value = extract_ip_header(&headers).unwrap_err();
        assert!(matches!(value, IpAddressError::Unsupported))
    }

    /// Tests that missing the required header provides an error
    #[test]
    fn test_missing_header() {
        let headers = HeaderMap::new();

        let value = extract_ip_header(&headers).unwrap_err();
        assert!(matches!(value, IpAddressError::MissingHeader))
    }

    /// Tests that the middleware can extract the header from a request
    #[tokio::test]
    async fn test_extraction_header() {
        let config = Arc::new(RuntimeConfig {
            reverse_proxy: true,
            ..Default::default()
        });

        let req = Request::builder()
            .extension(config)
            .header(REAL_IP_HEADER, HeaderValue::from_static("127.0.0.1"))
            .body("")
            .unwrap();

        let (mut parts, _) = req.into_parts();

        let IpAddress(ip) = IpAddress::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(ip, Ipv4Addr::new(127, 0, 0, 1));
    }

    /// Tests that when the reverse proxy mode is disabled that the [ConnectInfo]
    /// extension is used instead
    #[tokio::test]
    async fn test_extraction_fallback() {
        let config = Arc::new(RuntimeConfig {
            reverse_proxy: false,
            ..Default::default()
        });
        let req = Request::builder()
            .extension(config)
            .extension(ConnectInfo(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(127, 0, 0, 1),
                0,
            ))))
            .body("")
            .unwrap();

        let (mut parts, _) = req.into_parts();

        let IpAddress(ip) = IpAddress::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(ip, Ipv4Addr::new(127, 0, 0, 1));
    }
}
