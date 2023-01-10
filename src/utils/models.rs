use crate::utils::types::PlayerID;
use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
    tag::TdfType,
    types::Union,
    value_type,
    writer::TdfWriter,
};
use serde::{ser::SerializeStruct, Serialize};
use std::{
    fmt::{Debug, Display},
    net::Ipv4Addr,
};

/// Networking information for an instance. Contains the
/// host address and the port
pub struct InstanceNet {
    pub host: InstanceHost,
    pub port: Port,
}

impl From<(String, Port)> for InstanceNet {
    fn from((host, port): (String, Port)) -> Self {
        let host = InstanceHost::from(host);
        Self { host, port }
    }
}

impl Encodable for InstanceNet {
    fn encode(&self, writer: &mut TdfWriter) {
        self.host.encode(writer);
        writer.tag_u16(b"PORT", self.port);
        writer.tag_group_end();
    }
}

impl Decodable for InstanceNet {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let host: InstanceHost = InstanceHost::decode(reader)?;
        let port: u16 = reader.tag("PORT")?;
        reader.read_byte()?;
        Ok(Self { host, port })
    }
}

value_type!(InstanceNet, TdfType::Group);

/// Type of instance details provided either hostname
/// encoded as string or IP address encoded as NetAddress
pub enum InstanceHost {
    Host(String),
    Address(NetAddress),
}

/// Attempts to convert the provided value into a instance type. If
/// the provided value is an IPv4 value then Address is used otherwise
/// Host is used.
impl From<String> for InstanceHost {
    fn from(value: String) -> Self {
        if let Ok(value) = value.parse::<Ipv4Addr>() {
            Self::Address(NetAddress::from_ipv4(&value))
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

impl Encodable for InstanceHost {
    fn encode(&self, writer: &mut TdfWriter) {
        match self {
            InstanceHost::Host(value) => writer.tag_str(b"HOST", value),
            InstanceHost::Address(value) => writer.tag_u32(b"IP", value.0),
        }
    }
}

impl Decodable for InstanceHost {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let host: Option<String> = reader.try_tag("HOST")?;
        if let Some(host) = host {
            return Ok(Self::Host(host));
        }
        let ip: NetAddress = reader.tag("IP")?;
        Ok(Self::Address(ip))
    }
}

/// Details about an instance. This is used for the redirector system
/// to both encode for redirections and decode for the retriever system
pub struct InstanceDetails {
    /// The networking information for the instance
    pub net: InstanceNet,
    /// Whether the host requires a secure connection (SSLv3)
    pub secure: bool,
}

impl Encodable for InstanceDetails {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_union_start(b"ADDR", NetworkAddressType::Server.into());
        writer.tag_value(b"VALU", &self.net);

        writer.tag_bool(b"SECU", self.secure);
        writer.tag_bool(b"XDNS", false);
    }
}

impl Decodable for InstanceDetails {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let net: InstanceNet = match reader.tag::<Union<InstanceNet>>("ADDR")? {
            Union::Set { value, .. } => value,
            Union::Unset => {
                return Err(blaze_pk::error::DecodeError::MissingTag {
                    tag: "ADDR".to_string(),
                    ty: TdfType::Union,
                })
            }
        };
        let secure: bool = reader.tag("SECU")?;
        Ok(InstanceDetails { net, secure })
    }
}

pub struct UpdateExtDataAttr {
    pub flags: u8,
    pub player_id: PlayerID,
}

impl Encodable for UpdateExtDataAttr {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u8(b"FLGS", self.flags);
        writer.tag_u32(b"ID", self.player_id);
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
pub enum NetworkAddressType {
    Server,
    Client,
    Pair,
    IpAddress,
    HostnameAddress,
    Unknown(u8),
}

impl NetworkAddressType {
    pub fn value(&self) -> u8 {
        match self {
            Self::Server => 0x0,
            Self::Client => 0x1,
            Self::Pair => 0x2,
            Self::IpAddress => 0x3,
            Self::HostnameAddress => 0x4,
            Self::Unknown(value) => *value,
        }
    }

    pub fn from_value(value: u8) -> Self {
        match value {
            0x0 => Self::Server,
            0x1 => Self::Client,
            0x2 => Self::Pair,
            0x3 => Self::IpAddress,
            0x4 => Self::HostnameAddress,
            value => Self::Unknown(value),
        }
    }
}

impl From<NetworkAddressType> for u8 {
    fn from(value: NetworkAddressType) -> Self {
        value.value()
    }
}

/// Structure for storing extended network data
#[derive(Debug, Copy, Clone, Default, Serialize)]
pub struct QosNetworkData {
    /// Downstream bits per second
    pub dbps: u16,
    /// Natt type
    pub natt: NatType,
    /// Upstream bits per second
    pub ubps: u16,
}

//
#[derive(Debug, Copy, Clone, Serialize)]
pub enum NatType {
    Open,
    Moderate,
    Sequential,
    Strict,
    Unknown(u8),
}

impl NatType {
    pub fn value(&self) -> u8 {
        match self {
            Self::Open => 0x1,
            Self::Moderate => 0x2,
            Self::Sequential => 0x3,
            Self::Strict => 0x4,
            Self::Unknown(value) => *value,
        }
    }

    pub fn from_value(value: u8) -> Self {
        match value {
            0x1 => Self::Open,
            0x2 => Self::Moderate,
            0x3 => Self::Sequential,
            0x4 => Self::Strict,
            value => Self::Unknown(value),
        }
    }
}

impl Default for NatType {
    fn default() -> Self {
        Self::Strict
    }
}

impl Encodable for NatType {
    #[inline]
    fn encode(&self, writer: &mut TdfWriter) {
        writer.write_u8(self.value());
    }
}

impl Decodable for NatType {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        Ok(Self::from_value(reader.read_u8()?))
    }
}

value_type!(NatType, TdfType::VarInt);

impl Encodable for QosNetworkData {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u16(b"DBPS", self.dbps);
        writer.tag_value(b"NATT", &self.natt);
        writer.tag_u16(b"UBPS", self.ubps);
        writer.tag_group_end();
    }
}

impl Decodable for QosNetworkData {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let dbps: u16 = reader.tag("DBPS")?;
        let natt: NatType = reader.tag("NATT")?;
        let ubps: u16 = reader.tag("UBPS")?;
        reader.read_byte()?;
        Ok(Self { dbps, natt, ubps })
    }
}

value_type!(QosNetworkData, TdfType::Group);

/// Type alias for ports which are always u16
pub type Port = u16;

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub groups: NetGroups,
    pub qos: QosNetworkData,
    pub hardware_flags: u16,
    pub is_set: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetGroups {
    pub internal: NetGroup,
    pub external: NetGroup,
}

impl Encodable for NetGroups {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"EXIP");
        self.external.encode(writer);

        writer.tag_group(b"INIP");
        self.internal.encode(writer);

        writer.tag_group_end();
    }
}

impl Decodable for NetGroups {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let external: NetGroup = reader.tag("EXIP")?;
        let internal: NetGroup = reader.tag("INIP")?;
        reader.read_byte()?;
        Ok(Self { external, internal })
    }
}

value_type!(NetGroups, TdfType::Group);

impl NetData {
    pub fn tag_groups(&self, tag: &[u8], writer: &mut TdfWriter) {
        if !self.is_set {
            writer.tag_union_unset(tag);
            return;
        }
        writer.tag_union_value(tag, NetworkAddressType::Pair.into(), b"VALU", &self.groups);
    }
}

/// Structure for a networking group which consists of a
/// networking address and port value
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct NetGroup(pub NetAddress, pub Port);

impl Encodable for NetGroup {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"IP", self.0 .0);
        writer.tag_u16(b"PORT", self.1);
        writer.tag_group_end();
    }
}

impl Decodable for NetGroup {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let ip: NetAddress = reader.tag("IP")?;
        let port: u16 = reader.tag("PORT")?;
        reader.read_byte()?;
        Ok(Self(ip, port))
    }
}

value_type!(NetGroup, TdfType::Group);

impl Serialize for NetGroup {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("NetGroup", 2)?;
        s.serialize_field("address", &self.0)?;
        s.serialize_field("port", &self.1)?;
        s.end()
    }
}

/// Structure for wrapping a Blaze networking address
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct NetAddress(pub u32);

impl Encodable for NetAddress {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.write_u32(self.0);
    }
}

impl Decodable for NetAddress {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value = reader.read_u32()?;
        Ok(Self(value))
    }
}

value_type!(NetAddress, TdfType::VarInt);

/// Debug trait implementation sample implementation as the Display
/// implementation so that is just called instead
impl Debug for NetAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

/// Display trait implementation for NetAddress. If the value is valid
/// the value is translated into the IPv4 representation
impl Display for NetAddress {
    /// Converts the value stored in this NetAddress to an IPv4 string
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "{a}.{b}.{c}.{d}")
    }
}

/// Serialization implementation for NetAddress which uses the IPv4
/// representation implemented in Display
impl Serialize for NetAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = self.to_string();
        serializer.serialize_str(&value)
    }
}

impl NetAddress {
    /// Converts the provided IPv4 addr into a NetAddress by
    /// converting its bytes into a u32 value
    pub fn from_ipv4(value: &Ipv4Addr) -> NetAddress {
        let bytes = value.octets();
        NetAddress(u32::from_be_bytes(bytes))
    }
}
