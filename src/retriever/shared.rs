use blaze_pk::{
    group, tag_str, tag_u32, tag_u8, tag_value, Codec, CodecError, CodecResult, Reader, Tag,
    TdfOptional,
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
    pub secu: bool,
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
        let secu = Tag::expect(reader, "SECU")?;
        Ok(InstanceResponse { host, port, secu })
    }
}
