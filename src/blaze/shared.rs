use blaze_pk::{Blob, Codec, CodecError, CodecResult, decode_field, encode_empty_str, encode_field, encode_zero, group, packet, PacketContent, Reader, Tag, TdfMap, TdfOptional, ValueType, VarIntList};
use crate::blaze::SessionData;
use crate::database::entities::PlayerModel;


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
        CVAR cvar: VarIntList<u16>,
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
        encode_field!(output, DBPS, &self.dbps, u16);
        encode_field!(output, NATT, &self.natt, u8);
        encode_field!(output, UBPS, &self.ubps, u16);
        output.push(0)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        decode_field!(reader, DBPS, dbps, u16);
        decode_field!(reader, NATT, natt, u8);
        decode_field!(reader, UBPS, ubps, u16);
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


/// Complex authentication result structure is manually encoded because it
/// has complex nesting and output can vary based on inputs provided
#[derive(Debug)]
pub struct AuthRes<'a, 'b> {
    pub session_data: &'a SessionData,
    pub player: &'b PlayerModel,
    pub session_token: String,
    pub silent: bool,
}

impl PacketContent for AuthRes<'_, '_> {}

impl Codec for AuthRes<'_, '_> {
    fn encode(&self, output: &mut Vec<u8>) {
        let silent = self.silent;
        if silent {
            encode_zero!(output, AGUP);
        }

        encode_empty_str!(output, LDHT);
        encode_zero!(output, NTOS);
        encode_field!(output, PCTK, &self.session_token, String);

        #[inline]
        fn encode_persona(player: &PlayerModel, output: &mut Vec<u8>) {
            encode_field!(output, DSNM, &player.display_name, String);
            encode_zero!(output, LAST);
            encode_field!(output, PID, &player.id, u32);
            encode_zero!(output, STAS);
            encode_zero!(output, XREF);
            encode_zero!(output, XTYP);
            output.push(0);
        }

        if silent {
            encode_empty_str!(output, PRIV);
            Tag::encode_from("SESS", &ValueType::Group, output);
            encode_field!(output, BUID, &self.player.id, u32);
            encode_zero!(output, FRST);
            encode_field!(output, KEY, &self.session_token, String);
            encode_zero!(output, LLOG);
            encode_field!(output, MAIL, &self.player.email, String);
            Tag::encode_from("PDTL", &ValueType::Group, output);
            encode_persona(&self.player, output);
            encode_field!(output, UID, &self.player.id, u32);
            output.push(0);
        } else {
            Tag::encode_from("PLST", &ValueType::List, output);
            ValueType::Group.encode(output);
            output.push(1);
            encode_persona(&self.player, output);

            encode_empty_str!(output, PRIV);
            encode_field!(output, SKEY, &self.session_token, String);
        }
        encode_zero!(output, SPAM);
        encode_empty_str!(output, THST);
        encode_empty_str!(output, TSUI);
        encode_empty_str!(output, TURI);
        if !silent {
            encode_field!(output, UID, &self.player.id, u32);
        }
    }

    fn decode(_: &mut Reader) -> CodecResult<Self> {
        Err(CodecError::InvalidAction("Not allowed to decode AuthRes"))
    }
}