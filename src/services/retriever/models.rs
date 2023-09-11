use tdf::{TdfDeserializeOwned, TdfSerialize, TdfType};

/// Packet encoding for Redirector GetServerInstance packets
/// this contains basic information about the client session.
///
/// These details are extracted from an official game copy
pub struct InstanceRequest;

impl TdfSerialize for InstanceRequest {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_str(b"BSDK", "3.15.6.0");
        w.tag_str(b"BTIM", "Dec 21 2012 12:47:10");
        w.tag_str(b"CLNT", "MassEffect3-pc");
        w.tag_u8(b"CLTP", 0);
        w.tag_str(b"CSKU", "134845");
        w.tag_str(b"CVER", "05427.124");
        w.tag_str(b"DSDK", "8.14.7.1");
        w.tag_str(b"ENV", "prod");
        w.tag_union_unset(b"FPID");
        w.tag_u32(b"LOC", 0x656e4e5a);
        w.tag_str(b"NAME", "masseffect-3-pc");
        w.tag_str(b"PLAT", "Windows");
        w.tag_str(b"PROF", "standardSecure_v3");
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

impl TdfDeserializeOwned for OriginLoginResponse {
    fn deserialize_owned(r: &mut tdf::TdfDeserializer<'_>) -> tdf::DecodeResult<Self> {
        r.until_tag(b"SESS", TdfType::Group)?;
        let email: String = r.tag(b"MAIL")?;

        r.until_tag(b"PDTL", TdfType::Group)?;
        let display_name: String = r.tag(b"DSNM")?;
        Ok(Self {
            email,
            display_name,
        })
    }
}
