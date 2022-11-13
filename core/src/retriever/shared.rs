use blaze_pk::{
    codec::{Codec, CodecError, CodecResult, Reader},
    group, packet,
    tag::Tag,
    tagging::*,
    types::{TdfMap, TdfOptional},
};

/// Packet encoding for Redirector GetServerInstance packets
pub struct InstanceRequest;

impl Codec for InstanceRequest {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "BSDK", "3.15.6.0");
        tag_str(output, "BTIM", "Dec 21 2012 12:47:10");
        tag_str(output, "CLNT", "MassEffect3-pc");
        tag_u8(output, "CLTP", 0);
        tag_str(output, "CSKU", "134845");
        tag_str(output, "CVER", "05427.124");
        tag_str(output, "DSDK", "8.14.7.1");
        tag_str(output, "ENV", "prod");
        tag_value(output, "FPID", &TdfOptional::<String>::None);
        tag_u32(output, "LOC", 0x656e4e5a);
        tag_str(output, "NAME", "masseffect-3-pc");
        tag_str(output, "PLAT", "Windows");
        tag_str(output, "PROF", "standardSecure_v3");
    }
}

#[derive(Debug)]
pub struct InstanceResponse {
    pub host: String,
    pub port: u16,
}

group! {
    struct AddrValue {
        HOST host: String,
        PORT port: u16,
    }
}

impl Codec for InstanceResponse {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let (host, port) = match Tag::expect::<TdfOptional<AddrValue>>(reader, "ADDR")? {
            TdfOptional::Some(_, (_, value)) => (value.host, value.port),
            TdfOptional::None => {
                return Err(CodecError::Other(
                    "Expected address value to have its contents",
                ))
            }
        };
        Ok(InstanceResponse { host, port })
    }
}

pub struct OriginLoginRes {
    pub email: String,
    pub display_name: String,
}

group! {
    struct OriginSess {
        MAIL mail: String,
        PDTL data: OriginPlayerData,
    }
}

group! {
    struct OriginPlayerData {
        DSNM display_name: String
    }
}

impl Codec for OriginLoginRes {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let sess = Tag::expect::<OriginSess>(reader, "SESS")?;
        Ok(Self {
            email: sess.mail,
            display_name: sess.data.display_name,
        })
    }
}
pub struct OriginLoginReq {
    pub token: String,
}

impl Codec for OriginLoginReq {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "AUTH", &self.token);
        tag_u8(output, "TYPE", 0x1);
    }
}

packet! {
    struct UserSettingsAll {
        SMAP value: TdfMap<String, String>
    }
}
