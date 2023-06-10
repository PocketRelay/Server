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
        let port: u16 = reader.tag(b"PORT")?;
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
            Self::Address(NetAddress(value))
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
            InstanceHost::Address(value) => writer.tag_value(b"IP", value),
        }
    }
}

impl Decodable for InstanceHost {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let host: Option<String> = reader.try_tag(b"HOST")?;
        if let Some(host) = host {
            return Ok(Self::Host(host));
        }
        let ip: NetAddress = reader.tag(b"IP")?;
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
        writer.tag_union_start(b"ADDR", ServerAddressType::IpAddress as u8);
        writer.tag_value(b"VALU", &self.net);

        writer.tag_bool(b"SECU", self.secure);
        writer.tag_bool(b"XDNS", false);
    }
}

impl Decodable for InstanceDetails {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let net: InstanceNet = match reader.tag::<Union<InstanceNet>>(b"ADDR")? {
            Union::Set { value, .. } => value,
            Union::Unset => {
                return Err(blaze_pk::error::DecodeError::MissingTag {
                    tag: b"ADDR".into(),
                    ty: TdfType::Union,
                })
            }
        };
        let secure: bool = reader.tag(b"SECU")?;
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

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum NetworkAddressType {
    // XboxClient = 0x0,
    // XboxServer = 0x1,
    Pair = 0x2,
    // IpAddress = 0x3,
    // HostnameAddress = 0x4,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum ServerAddressType {
    IpAddress = 0x0,
    // XboxServer = 0x1,
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
#[repr(u8)]
pub enum NatType {
    Open = 0x0,
    Moderate = 0x1,
    Sequential = 0x2,
    Strict = 0x3,
    Unknown = 0x4,
}

impl NatType {
    pub fn from_value(value: u8) -> Self {
        match value {
            0x1 => Self::Open,
            0x2 => Self::Moderate,
            0x3 => Self::Sequential,
            0x4 => Self::Strict,
            // TODO: Possibly debug log this
            _ => Self::Unknown,
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
        writer.write_u8((*self) as u8);
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
        let dbps: u16 = reader.tag(b"DBPS")?;
        let natt: NatType = reader.tag(b"NATT")?;
        let ubps: u16 = reader.tag(b"UBPS")?;
        reader.read_byte()?;
        Ok(Self { dbps, natt, ubps })
    }
}

value_type!(QosNetworkData, TdfType::Group);

/// Type alias for ports which are always u16
pub type Port = u16;

#[derive(Debug, Default, Clone, Serialize)]
pub struct NetData {
    pub groups: Option<NetGroups>,
    pub qos: QosNetworkData,
    pub hardware_flags: u16,
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
        let external: NetGroup = reader.tag(b"EXIP")?;
        let internal: NetGroup = reader.tag(b"INIP")?;
        reader.read_byte()?;
        Ok(Self { external, internal })
    }
}

value_type!(NetGroups, TdfType::Group);

impl NetData {
    pub fn tag_groups(&self, tag: &[u8], writer: &mut TdfWriter) {
        if let Some(groups) = &self.groups {
            writer.tag_union_value(tag, NetworkAddressType::Pair as u8, b"VALU", groups);
        } else {
            writer.tag_union_unset(tag);
        }
    }
}

/// Structure for a networking group which consists of a
/// networking address and port value
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct NetGroup(pub NetAddress, pub Port);

impl Encodable for NetGroup {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"IP", &self.0);
        writer.tag_u16(b"PORT", self.1);
        writer.tag_group_end();
    }
}

impl Decodable for NetGroup {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let ip: NetAddress = reader.tag(b"IP")?;
        let port: u16 = reader.tag(b"PORT")?;
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
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct NetAddress(pub Ipv4Addr);

impl Default for NetAddress {
    fn default() -> Self {
        Self(Ipv4Addr::LOCALHOST)
    }
}

impl Encodable for NetAddress {
    fn encode(&self, writer: &mut TdfWriter) {
        let bytes = self.0.octets();
        let value = u32::from_be_bytes(bytes);
        writer.write_u32(value);
    }
}

impl Decodable for NetAddress {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value = reader.read_u32()?;
        let bytes = value.to_be_bytes();
        let addr = Ipv4Addr::from(bytes);
        Ok(Self(addr))
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
        write!(f, "{}", self.0)
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
