use std::{fmt::Debug, str::Split};

use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    packet,
    tag::{Tag, ValueType},
    tagging::*,
    types::TdfOptional,
};

use serde::Serialize;
use utils::types::PlayerID;

packet! {
    struct UpdateExtDataAttr {
        FLGS flags: u8,
        ID player_id: PlayerID
    }
}

/// Structure for storing extended network data
#[derive(Debug, Copy, Clone, Default, Serialize)]
pub struct NetExt {
    pub dbps: u16,
    pub natt: u8,
    pub ubps: u16,
}

//noinspection SpellCheckingInspection
impl Codec for NetExt {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u16(output, "DBPS", self.dbps);
        tag_u8(output, "NATT", self.natt);
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
    pub ext: NetExt,
    pub is_unset: bool,
    pub hwfg: u16,
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
    pub fn get_groups(&self) -> TdfOptional<NetGroups> {
        if self.is_unset {
            TdfOptional::None
        } else {
            TdfOptional::Some(0x2, (String::from("VALU"), self.groups))
        }
    }
}

/// Structure for a networking group which consists of a
/// networking address and port value
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize)]
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

/// Structure for wrapping a Blaze networking address
#[derive(Copy, Clone, Default, Eq, PartialEq, Serialize)]
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
