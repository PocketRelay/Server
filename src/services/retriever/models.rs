use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
    tag::TdfType,
    writer::TdfWriter,
};

/// Packet encoding for Redirector GetServerInstance packets
/// this contains basic information about the client session.
///
/// These details are extracted from an official game copy
pub struct InstanceRequest;

impl Encodable for InstanceRequest {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_str(b"BSDK", "3.15.6.0");
        writer.tag_str(b"BTIM", "Dec 21 2012 12:47:10");
        writer.tag_str(b"CLNT", "MassEffect3-pc");
        writer.tag_u8(b"CLTP", 0);
        writer.tag_str(b"CSKU", "134845");
        writer.tag_str(b"CVER", "05427.124");
        writer.tag_str(b"DSDK", "8.14.7.1");
        writer.tag_str(b"ENV", "prod");
        writer.tag_union_unset(b"FPID");
        writer.tag_u32(b"LOC", 0x656e4e5a);
        writer.tag_str(b"NAME", "masseffect-3-pc");
        writer.tag_str(b"PLAT", "Windows");
        writer.tag_str(b"PROF", "standardSecure_v3");
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

impl Decodable for OriginLoginResponse {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.until_tag(b"SESS", TdfType::Group)?;
        let email: String = reader.tag(b"MAIL")?;

        reader.until_tag(b"PDTL", TdfType::Group)?;
        let display_name: String = reader.tag(b"DSNM")?;
        Ok(Self {
            email,
            display_name,
        })
    }
}
