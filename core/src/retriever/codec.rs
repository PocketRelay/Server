use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    tag::{Tag, ValueType},
    tagging::*,
    types::TdfMap,
};

/// Packet encoding for Redirector GetServerInstance packets
/// this contains basic information about the client session.
///
/// These details are extracted from an official game copy
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
        tag_union_unset(output, "FPID");
        tag_u32(output, "LOC", 0x656e4e5a);
        tag_str(output, "NAME", "masseffect-3-pc");
        tag_str(output, "PLAT", "Windows");
        tag_str(output, "PROF", "standardSecure_v3");
    }
}

/// Structure for the response from the server after
/// authenticating with Origin. Contains the email and
/// display name of the authenticated account
pub struct OriginLoginResponse {
    /// The email address of the Origin Account
    pub email: String,
    /// The display name of the origin account
    pub display_name: String,
}

impl Codec for OriginLoginResponse {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        Tag::decode_until(reader, "SESS", ValueType::Group)?;
        let email = expect_tag(reader, "MAIL")?;
        Tag::decode_until(reader, "PDTL", ValueType::Group)?;
        let display_name = expect_tag(reader, "DSNM")?;
        Tag::discard_group(reader)?; // End group PDTL
        Tag::discard_group(reader)?; // End group MAIL
        Ok(Self {
            email,
            display_name,
        })
    }
}

/// Structure for a request to login with Origin using
/// the Origin token that was provided by the client
pub struct OriginLoginRequest {
    /// The origin token provided by the client
    pub token: String,
}

impl Codec for OriginLoginRequest {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "AUTH", &self.token);
        tag_u8(output, "TYPE", 0x1);
    }
}

/// Structure for the response from retrieving the user
/// settings from the official servers
pub struct SettingsResponse {
    /// The settings from the server
    pub settings: TdfMap<String, String>,
}

impl Codec for SettingsResponse {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let settings = expect_tag(reader, "SMAP")?;
        Ok(Self { settings })
    }
}
