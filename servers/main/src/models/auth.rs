use blaze_pk::{
    codec::{Codec, CodecError, CodecResult, Reader},
    tag::{Tag, ValueType},
    tagging::*,
};
use database::Player;
use utils::types::PlayerID;

/// Different possible authentication request types.
pub enum AuthRequest {
    /// Silent token based authentication with player ID
    Silent { token: String, player_id: PlayerID },
    /// Login through login prompt menu with email and password
    Login { email: String, password: String },
    /// AUthentication through origin token
    Origin { token: String },
}

impl AuthRequest {
    pub fn is_silent(&self) -> bool {
        match self {
            Self::Silent { .. } | Self::Origin { .. } => true,
            Self::Login { .. } => false,
        }
    }
}

impl Codec for AuthRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        reader.mark();
        // Read the type of request
        let ty = Tag::expect::<u8>(reader, "TYPE")?;
        reader.reset_marker();
        match ty {
            0 => {
                let email = Tag::expect(reader, "MAIL")?;
                let password = Tag::expect(reader, "PASS")?;
                Ok(Self::Login { email, password })
            }
            1 => {
                let token = Tag::expect(reader, "AUTH")?;
                Ok(Self::Origin { token })
            }
            2 => {
                let token = Tag::expect(reader, "AUTH")?;
                let player_id = Tag::expect(reader, "PID")?;
                Ok(Self::Silent { token, player_id })
            }
            _ => Err(CodecError::Other("Unknown auth request type")),
        }
    }
}

/// Encodes a mock persona from the provided player using its
/// display name and ID as the values
fn encode_persona(output: &mut Vec<u8>, player: &Player) {
    tag_str(output, "DSNM", &player.display_name);
    tag_zero(output, "LAST");
    tag_u32(output, "PID", player.id);
    tag_zero(output, "STAS");
    tag_zero(output, "XREF");
    tag_zero(output, "XTYP");
    tag_group_end(output);
}

/// Structure for the response to an authentication request.
pub struct AuthResponse<'a> {
    /// The authenticated player
    pub player: &'a Player,
    /// The session token for the completed authentication
    pub session_token: String,
    /// Whether the authentication proccess was silent
    pub silent: bool,
}

impl Codec for AuthResponse<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        if self.silent {
            tag_zero(output, "AGUP");
        }
        tag_empty_str(output, "LDHT");
        tag_zero(output, "NTOS");
        tag_str(output, "PCTK", &self.session_token); // PC Authentication Token
        if self.silent {
            tag_empty_str(output, "PRIV");
            {
                tag_group_start(output, "SESS");
                tag_u32(output, "BUID", self.player.id);
                tag_zero(output, "FRST");
                tag_str(output, "KEY", &self.session_token); // Session Token
                tag_zero(output, "LLOG");
                tag_str(output, "MAIL", &self.player.email); // Player Email
                {
                    tag_group_start(output, "PDTL");
                    encode_persona(output, &self.player); // Persona Details
                }
                tag_u32(output, "UID", self.player.id);
                tag_group_end(output);
            }
        } else {
            tag_list_start(output, "PLST", ValueType::Group, 1);
            encode_persona(output, &self.player);
            tag_empty_str(output, "PRIV");
            tag_str(output, "SKEY", &self.session_token);
        }
        tag_zero(output, "SPAM");
        tag_empty_str(output, "THST");
        tag_empty_str(output, "TSUI");
        tag_empty_str(output, "TURI");
        if !self.silent {
            tag_u32(output, "UID", self.player.id);
        }
    }
}

/// Structure for request to create a new account with
/// the provided email and password
pub struct CreateAccountRequest {
    /// The email address of the account to create
    pub email: String,
    /// The password of the account to create
    pub password: String,
}

impl Codec for CreateAccountRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let email = Tag::expect(reader, "MAIL")?;
        let password = Tag::expect(reader, "PASS")?;
        Ok(Self { email, password })
    }
}

pub struct PersonaResponse<'a> {
    pub player: &'a Player,
    pub session_token: String,
}

impl Codec for PersonaResponse<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u32(output, "BUID", self.player.id);
        tag_zero(output, "FRST");
        tag_str(output, "KEY", &self.session_token);
        tag_zero(output, "LLOG");
        tag_str(output, "MAIL", &self.player.email);
        tag_group_start(output, "PDTL");
        encode_persona(output, &self.player);
        tag_u32(output, "UID", self.player.id);
    }
}

/// Request for listing entitlements
pub struct ListEntitlementsRequest {
    /// The entitlements tag
    pub tag: String,
}

impl Codec for ListEntitlementsRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let tag = Tag::expect(reader, "ETAG")?;
        Ok(Self { tag })
    }
}

/// Response of an entitlements list
pub struct ListEntitlementsResponse {
    pub list: Vec<Entitlement>,
}

impl Codec for ListEntitlementsResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "NLST", &self.list);
    }
}

//noinspection SpellCheckingInspection
pub struct Entitlement {
    pub name: &'static str,
    pub id: u64,
    pub pjid: &'static str,
    pub prca: u8,
    pub prid: &'static str,
    pub tag: &'static str,
    pub ty: u8,
}

impl Entitlement {
    pub const PC_TAG: &'static str = "ME3PCOffers";
    pub const GEN_TAG: &'static str = "ME3GenOffers";

    pub fn new_pc(
        id: u64,
        pjid: &'static str,
        prca: u8,
        prid: &'static str,
        tag: &'static str,
        ty: u8,
    ) -> Self {
        Self {
            name: Self::PC_TAG,
            id,
            pjid,
            prca,
            prid,
            tag,
            ty,
        }
    }

    pub fn new_gen(
        id: u64,
        pjid: &'static str,
        prca: u8,
        prid: &'static str,
        tag: &'static str,
        ty: u8,
    ) -> Self {
        Self {
            name: Self::GEN_TAG,
            id,
            pjid,
            prca,
            prid,
            tag,
            ty,
        }
    }
}

impl Codec for Entitlement {
    //noinspection SpellCheckingInspection
    fn encode(&self, output: &mut Vec<u8>) {
        tag_empty_str(output, "DEVI");
        tag_str(output, "GDAY", "2012-12-15T16:15Z");
        tag_str(output, "GNAM", self.name);
        tag_u64(output, "ID", self.id);
        tag_u8(output, "ISCO", 0);
        tag_u8(output, "PID", 0);
        tag_str(output, "PJID", self.pjid);
        tag_u8(output, "PRCA", self.prca);
        tag_str(output, "PRID", self.prid);
        tag_u8(output, "STAT", 1);
        tag_u8(output, "STRC", 0);
        tag_str(output, "TAG", self.tag);
        tag_empty_str(output, "TDAY");
        tag_u8(output, "TTYPE", self.ty);
        tag_u8(output, "UCNT", 0);
        tag_u8(output, "VER", 0);
        tag_group_end(output);
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

/// Structure for a request to send a forgot password email. Currently
/// only logs that a reset was requested and doesn't actually send
/// an email.
pub struct ForgotPasswordRequest {
    /// The email of the account that needs a password reset
    pub email: String,
}

impl Codec for ForgotPasswordRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let email = Tag::expect(reader, "MAIL")?;
        Ok(Self { email })
    }
}

/// Dummy structure for the LegalDocsInfo response. None of the
/// values in this struct ever change.
pub struct LegalDocsInfo;

impl Codec for LegalDocsInfo {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_zero(output, "EAMC");
        tag_empty_str(output, "LHST");
        tag_zero(output, "PMC");
        tag_empty_str(output, "PPUI");
        tag_empty_str(output, "TSUI");
    }
}

/// Structure for legal content responses such as the Privacy Policy
/// and the terms and condition.
pub struct LegalContent<'a> {
    /// The url path to the legal content (Prefix this value with https://tos.ea.com/legalapp/ to get the url)
    pub path: &'static str,
    /// The actual HTML content of the legal document
    pub content: &'a str,
    /// Unknown value
    pub col: u16,
}

impl Codec for LegalContent<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "LDVC", self.path);
        tag_u16(output, "TCOL", self.col);
        tag_str(output, "TCOT", self.content);
    }
}

/// Response to the client requesting a shared token
pub struct GetTokenResponse {
    /// The generated shared token
    pub token: String,
}

impl Codec for GetTokenResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "AUTH", &self.token)
    }
}
