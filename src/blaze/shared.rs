use blaze_pk::{Blob, Codec, CodecError, CodecResult, group, packet, Reader, Tag, TdfMap, TdfOptional, ValueType, VarInt, VarIntList};
use crate::blaze::SessionData;

packet! {
    struct SilentAuthRes {
        AGUP agup: u8,
        LDHT ldht: &'static str,
        NTOS ntos: u8,
        PCTK token: String,
        PRIV pri: &'static str,
        SESS session: SessionDetailsSilent,
        SPAM spam: u8,
        THST thst: &'static str,
        TSUI tsui: &'static str,
        TURI turi: &'static str
    }
}

packet! {
    struct AuthRes {
        LDHT ldht: &'static str,
        NTOS ntos: u8,
        PCTK token: String,
        PLST personas: Vec<PersonaDetails>,
        PRIV pri: &'static str,
        SKEY skey: String,
        SPAM spam: u8,
        THST thst: &'static str,
        TSUI tsui: &'static str,
        TURI turi: &'static str,
        UID uid: u32
    }
}

packet! {
    struct SessionDetailsSilent {
        BUID buid: u32,
        FRST frst: u8,
        KEY key: String,
        LLOG llog: u8,
        MAIL mail: String,
        PDTL personal_details: PersonaDetails,
        UID uid: u32,
    }
}

group! {
    struct PersonaDetails {
        DSNM display_name: String,
        LAST last_login_time: u32,
        PID  id: u32,
        STAS stas: u8,
        XREF xref: u8,
        XTYP xtype: u8
    }
}

packet! {
    struct SessionDetails {
        DATA data: SessionDataCodec,
        USER user: SessionUser
    }
}

packet! {
    struct UpdateExtDataAttr {
        FLGS flags: u8,
        ID id: u32
    }
}

packet! {
    struct SessionUser {
        AID aid: u32,
        ALOC location: u32,
        EXBB exbb: Blob,
        EXID exid: u8,
        ID id: u32,
        NAME name: String
    }
}

group! {
    struct SessionDataCodec {
        ADDR addr: TdfOptional<NetGroups>,
        BPS bps: &'static str,
        CTY cty: &'static str,
        CVAR cvar: VarIntList,
        DMAP dmap: TdfMap<u32, u32>,
        HWFG hardware_flag: u16,
        PSLM pslm: Vec<u32>,
        QDAT net_ext: NetExt,
        UATT uatt: u8,
        ULST ulst: Vec<(u8, u8, u32)>
    }
}

/// Structure for storing extended network data
#[derive(Debug, Copy, Clone, Default)]
pub struct NetExt {
    pub dbps: u16,
    pub natt: u8,
    pub ubps: u16,
}

impl Codec for NetExt {
    fn encode(&self, output: &mut Vec<u8>) {
        Tag::encode_from("DBPS", &ValueType::VarInt, output);
        self.dbps.encode(output);
        Tag::encode_from("NATT", &ValueType::VarInt, output);
        self.natt.encode(output);
        Tag::encode_from("UBPS", &ValueType::VarInt, output);
        self.ubps.encode(output);
        output.push(0)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        Tag::expect_tag("DBPS", &ValueType::VarInt, reader)?;
        let dbps = u16::decode(reader)?;
        Tag::expect_tag("NATT", &ValueType::VarInt, reader)?;
        let natt = u8::decode(reader)?;
        Tag::expect_tag("UBPS", &ValueType::VarInt, reader)?;
        let ubps = u16::decode(reader)?;

        reader.take_one()?;
        Ok(Self { dbps, natt, ubps })
    }
}

/// Type alias for ports which are always u16
pub type Port = u16;

#[derive(Debug, Default)]
pub struct NetData {
    pub groups: NetGroups,
    pub ext: NetExt,
    pub is_unset: bool,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct NetGroups {
    pub internal: NetGroup,
    pub external: NetGroup,
}

impl Codec for NetGroups {
    fn encode(&self, output: &mut Vec<u8>) {
        Tag::encode_from("EXIP", &ValueType::Group, output);
        self.external.encode(output);
        Tag::encode_from("INIP", &ValueType::Group, output);
        self.internal.encode(output);
        output.push(0)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        Tag::expect_tag("EXIP", &ValueType::Group, reader)?;
        let external = NetGroup::decode(reader)?;
        Tag::expect_tag("INIP", &ValueType::Group, reader)?;
        let internal = NetGroup::decode(reader)?;
        reader.take_one()?;
        Ok(Self {
            external,
            internal,
        })
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
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct NetGroup(pub NetAddress, pub Port);

impl Codec for NetGroup {
    fn encode(&self, output: &mut Vec<u8>) {
        Tag::encode_from("IP", &ValueType::VarInt, output);
        self.0.0.encode(output);

        Tag::encode_from("PORT", &ValueType::VarInt, output);
        self.1.encode(output);

        output.push(0)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        Tag::expect_tag("IP", &ValueType::VarInt, reader)?;
        let ip = u32::decode(reader)?;

        Tag::expect_tag("PORT", &ValueType::VarInt, reader)?;
        let port = u16::decode(reader)?;

        reader.take_one()?;
        Ok(Self(NetAddress(ip), port))
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

/// Structure for wrapping a Blaze networking address
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct NetAddress(pub u32);

impl NetAddress {
    /// Converts the provided IPv4 string into a NetAddress
    pub fn from_ipv4(value: &str) -> NetAddress {
        let parts = value.split(".")
            .filter_map(|value| value.parse::<u32>().ok())
            .collect::<Vec<u32>>();
        if parts.len() < 4 {
            return NetAddress(0);
        }
        let value = parts[0] << 24 | parts[1] << 16 | parts[2] << 8 | parts[3];
        NetAddress(value)
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