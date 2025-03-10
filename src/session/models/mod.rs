//! This module contains the packet content models for all the routes in
//! the routes module and any dependencies for those

pub mod auth;
pub mod errors;
pub mod game_manager;
pub mod messaging;
pub mod other;
pub mod stats;
pub mod user_sessions;
pub mod util;

use serde::Serialize;
use std::{fmt::Debug, net::Ipv4Addr};
use tdf::{GroupSlice, TdfDeserialize, TdfDeserializeOwned, TdfSerialize, TdfTyped};

/// Networking information for an instance. Contains the
/// host address and the port
#[derive(TdfTyped)]
#[tdf(group)]
pub struct InstanceAddress {
    pub host: InstanceHost,
    pub port: Port,
}

impl TdfSerialize for InstanceAddress {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group_body(|w| {
            self.host.serialize(w);
            w.tag_u16(b"PORT", self.port);
        });
    }
}

impl TdfDeserializeOwned for InstanceAddress {
    fn deserialize_owned(r: &mut tdf::TdfDeserializer<'_>) -> tdf::DecodeResult<Self> {
        let host: InstanceHost = InstanceHost::deserialize_owned(r)?;
        let port: u16 = r.tag(b"PORT")?;
        GroupSlice::deserialize_content_skip(r)?;
        Ok(Self { host, port })
    }
}

/// Type of instance details provided either hostname
/// encoded as string or IP address encoded as NetAddress
pub enum InstanceHost {
    Host(String),
    Address(Ipv4Addr),
}

/// Attempts to convert the provided value into a instance type. If
/// the provided value is an IPv4 value then Address is used otherwise
/// Host is used.
impl From<String> for InstanceHost {
    fn from(value: String) -> Self {
        if let Ok(value) = value.parse::<Ipv4Addr>() {
            Self::Address(value)
        } else {
            Self::Host(value)
        }
    }
}

/// Function for converting an instance type into its address
/// string value for use in connections
impl From<InstanceHost> for String {
    fn from(value: InstanceHost) -> Self {
        match value {
            InstanceHost::Address(value) => value.to_string(),
            InstanceHost::Host(value) => value,
        }
    }
}

impl TdfSerialize for InstanceHost {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        match self {
            InstanceHost::Host(value) => w.tag_str(b"HOST", value),
            InstanceHost::Address(value) => w.tag_u32(b"IP", (*value).into()),
        }
    }
}

impl TdfDeserializeOwned for InstanceHost {
    fn deserialize_owned(r: &mut tdf::TdfDeserializer<'_>) -> tdf::DecodeResult<Self> {
        let host: Option<String> = r.try_tag(b"HOST")?;
        if let Some(host) = host {
            return Ok(Self::Host(host));
        }
        let ip: u32 = r.tag(b"IP")?;
        Ok(Self::Address(Ipv4Addr::from(ip)))
    }
}

/// Details about an instance. This is used for the redirector system
/// to both encode for redirection and decode for the retriever system
#[derive(TdfDeserialize)]
#[allow(unused)]
pub struct InstanceDetails {
    /// The networking information for the instance
    #[tdf(tag = "ADDR")]
    pub net: InstanceNet,
    /// Whether the host requires a secure connection (SSLv3)
    #[tdf(tag = "SECU")]
    pub secure: bool,
    #[tdf(tag = "XDNS")]
    pub xdns: bool,
}

#[derive(Default, TdfSerialize, TdfDeserialize, TdfTyped)]
pub enum InstanceNet {
    #[tdf(key = 0x0, tag = "VALU")]
    InstanceAddress(InstanceAddress),
    #[tdf(unset)]
    Unset,
    #[default]
    #[tdf(default)]
    Default,
    // IpAddress = 0x0,
    // XboxServer = 0x1,
}

/// Structure for storing extended network data
#[derive(Debug, Copy, Clone, Default, Serialize, TdfSerialize, TdfDeserialize, TdfTyped)]
#[tdf(group)]
pub struct QosNetworkData {
    /// The client's downstream network bandwidth (in bits per second).
    #[tdf(tag = "DBPS")]
    pub dbps: u32,
    /// The client's network address translation type (aka firewall/router type).
    #[tdf(tag = "NATT")]
    pub natt: NatType,
    /// The client's upstream network bandwidth (in bits per second).
    #[tdf(tag = "UBPS")]
    pub ubps: u32,
}

//
#[derive(Debug, Default, Copy, Clone, Serialize, TdfDeserialize, TdfSerialize, TdfTyped)]
#[repr(u8)]
pub enum NatType {
    /// Players behind an open NAT can usually connect to any other player and are ideal game hosts.
    Open = 0x0,
    /// Players behind a moderate NAT can usually connect to other open or moderate players.
    Moderate = 0x1,
    /// Players behind a strict (but sequential) NAT can usually only connect to open players and are poor game hosts.
    StrictSequential = 0x2,
    /// Players behind a strict (non-sequential) NAT can usually only connect to open players and are the worst game hosts.
    Strict = 0x3,
    /// unknown NAT type; possibly timed out trying to detect NAT.
    #[default]
    #[tdf(default)]
    Unknown = 0x4,
}

#[derive(Default, Debug, Clone, TdfSerialize, TdfDeserialize, TdfTyped, Serialize)]
#[serde(untagged)]
pub enum NetworkAddress {
    #[tdf(key = 0x2, tag = "VALU")]
    AddressPair(IpPairAddress),
    #[tdf(unset)]
    Unset,
    #[default]
    #[tdf(default)]
    Default,
    // XboxClient = 0x0,
    // XboxServer = 0x1,
    // Pair = 0x2,
    // IpAddress = 0x3,
    // HostnameAddress = 0x4,
}

/// Type alias for ports which are always u16
pub type Port = u16;

/// Pair of socket addresses
#[derive(Debug, Default, Clone, TdfDeserialize, TdfSerialize, TdfTyped, Serialize)]
#[tdf(group)]
pub struct IpPairAddress {
    #[tdf(tag = "EXIP")]
    pub external: PairAddress,
    #[tdf(tag = "INIP")]
    pub internal: PairAddress,
}

#[derive(Debug, Clone, TdfDeserialize, TdfSerialize, TdfTyped, Serialize)]
#[tdf(group)]
pub struct PairAddress {
    #[tdf(tag = "IP", into = u32)]
    #[serde(rename = "address")]
    pub addr: Ipv4Addr,
    #[tdf(tag = "PORT")]
    pub port: u16,
}

impl Default for PairAddress {
    fn default() -> Self {
        Self {
            addr: Ipv4Addr::UNSPECIFIED,
            port: 0,
        }
    }
}
