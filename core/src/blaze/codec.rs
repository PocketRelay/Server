use std::{
    fmt::{Debug, Display},
    str::Split,
};

use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    packet,
    tag::{Tag, ValueType},
    tagging::*,
};

use serde::{ser::SerializeStruct, Serialize};
use utils::types::PlayerID;

packet! {
    struct UpdateExtDataAttr {
        FLGS flags: u8,
        ID player_id: PlayerID
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

impl Into<u8> for NetworkAddressType {
    fn into(self) -> u8 {
        self.value()
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

impl Codec for NatType {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = self.value();
        value.encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let value = u8::decode(reader)?;
        Ok(Self::from_value(value))
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

//noinspection SpellCheckingInspection
impl Codec for QosNetworkData {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u16(output, "DBPS", self.dbps);
        tag_value(output, "NATT", &self.natt);
        tag_u16(output, "UBPS", self.ubps);
        output.push(0)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let dbps = Tag::expect(reader, "DBPS")?;
        let natt = Tag::expect(reader, "NATT")?;
        let ubps = Tag::expect(reader, "UBPS")?;
        reader.take_one()?;
        Ok(Self { dbps, natt, ubps })
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

/// Type alias for ports which are always u16
pub type Port = u16;

#[derive(Debug, Default, Copy, Clone, Serialize)]
pub struct NetData {
    pub groups: NetGroups,
    pub qos: QosNetworkData,
    pub hardware_flags: u16,
    pub is_set: bool,
}

#[derive(Debug, Default, Copy, Clone, Serialize)]
pub struct NetGroups {
    pub internal: NetGroup,
    pub external: NetGroup,
}

//noinspection SpellCheckingInspection
impl Codec for NetGroups {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "EXIP");
        self.external.encode(output);

        tag_group_start(output, "INIP");
        self.internal.encode(output);

        tag_group_end(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let external = Tag::expect(reader, "EXIP")?;
        let internal = Tag::expect(reader, "INIP")?;
        reader.take_one()?;
        Ok(Self { external, internal })
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

impl NetData {
    pub fn tag_groups(&self, tag: &str, output: &mut Vec<u8>) {
        if !self.is_set {
            tag_union_unset(output, tag);
            return;
        }

        tag_union_value(
            output,
            tag,
            NetworkAddressType::Pair.into(),
            "VALU",
            self.groups,
        );
    }
}

/// Structure for a networking group which consists of a
/// networking address and port value
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct NetGroup(pub NetAddress, pub Port);

impl Codec for NetGroup {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "IP", self.0 .0);
        tag_u16(output, "PORT", self.1);
        tag_group_end(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let ip = Tag::expect(reader, "IP")?;
        let port = Tag::expect(reader, "PORT")?;
        reader.take_one()?;
        Ok(Self(NetAddress(ip), port))
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

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

impl Debug for NetAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_invalid() {
            f.write_str("INVALID_ADDR")
        } else {
            let value = self.to_ipv4();
            f.write_str(&value)
        }
    }
}

impl Display for NetAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_invalid() {
            f.write_str("INVALID_ADDR")
        } else {
            let value = self.to_ipv4();
            f.write_str(&value)
        }
    }
}

impl Serialize for NetAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.is_invalid() {
            serializer.serialize_str("INVALID_ADDR")
        } else {
            let value = self.to_ipv4();
            serializer.serialize_str(&value)
        }
    }
}

impl NetAddress {
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }

    /// Converts the provided IPv4 string into a NetAddress
    pub fn from_ipv4(value: &str) -> NetAddress {
        if let Some(value) = Self::try_from_ipv4(value) {
            value
        } else {
            NetAddress(0)
        }
    }

    /// Attempts to convert the provided IP address string into a
    /// NetAddress value. If the value is not a valid IPv4 address
    /// then None will be returned.
    pub fn try_from_ipv4(value: &str) -> Option<NetAddress> {
        let mut parts = value.split(".");
        let a = Self::next_ip_chunk(&mut parts)?;
        let b = Self::next_ip_chunk(&mut parts)?;
        let c = Self::next_ip_chunk(&mut parts)?;
        let d = Self::next_ip_chunk(&mut parts)?;

        let value = a << 24 | b << 16 | c << 8 | d;
        Some(NetAddress(value))
    }

    /// Obtains the next IPv4 (u8) chunk value from the provided
    /// split iterator
    fn next_ip_chunk(iter: &mut Split<&str>) -> Option<u32> {
        iter.next()?
            .parse::<u8>()
            .map(|value| Some(value as u32))
            .ok()?
    }

    /// Converts the value stored in this NetAddress to an IPv4 string
    pub fn to_ipv4(&self) -> String {
        let a = ((self.0 >> 24) & 0xFF) as u8;
        let b = ((self.0 >> 16) & 0xFF) as u8;
        let c = ((self.0 >> 8) & 0xFF) as u8;
        let d = (self.0 & 0xFF) as u8;
        format!("{a}.{b}.{c}.{d}")
    }
}
