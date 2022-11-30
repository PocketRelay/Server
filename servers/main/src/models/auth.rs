use blaze_pk::{
    codec::{Decodable, Encodable},
    error::{DecodeError, DecodeResult},
    reader::TdfReader,
    tag::TdfType,
    value_type,
    writer::TdfWriter,
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

impl Decodable for AuthRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let ty = {
            let start = reader.cursor;
            let ty: u8 = reader.tag("TYPE")?;
            reader.cursor = start;
            ty
        };

        match ty {
            0 => {
                let email: String = reader.tag("MAIL")?;
                let password: String = reader.tag("PASS")?;
                Ok(Self::Login { email, password })
            }
            1 => {
                let token: String = reader.tag("AUTH")?;
                Ok(Self::Origin { token })
            }
            2 => {
                let token: String = reader.tag("AUTH")?;
                let player_id: u32 = reader.tag("PID")?;
                Ok(Self::Silent { token, player_id })
            }
            _ => Err(DecodeError::UnknownType { ty }),
        }
    }
}

/// Encodes a mock persona from the provided player using its
/// display name and ID as the values
fn encode_persona(writer: &mut TdfWriter, id: PlayerID, display_name: &str) {
    writer.tag_str(b"DSNM", display_name);
    writer.tag_zero(b"LAST");
    writer.tag_u32(b"PID", id);
    writer.tag_zero(b"STAS");
    writer.tag_zero(b"XREF");
    writer.tag_zero(b"XTYP");
    writer.tag_group_end();
}

/// Structure for the response to an authentication request.
pub struct AuthResponse {
    /// The ID of the authenticated player
    player_id: PlayerID,
    /// The email of the authenticated player
    email: String,
    /// The display name of the authenticated player
    display_name: String,
    /// The session token for the completed authentication
    session_token: String,
    /// Whether the authentication proccess was silent
    silent: bool,
}

impl AuthResponse {
    /// Creates a new auth response from the provided player, session token
    /// and whether or not to be a silent value
    ///
    /// `player`        The player that was authenticated
    /// `session_token` The session token to use
    /// `silent`        Whether the auth request was silent
    pub fn new(player: &Player, session_token: String, silent: bool) -> Self {
        Self {
            player_id: player.id,
            email: player.email.clone(),
            display_name: player.display_name.clone(),
            session_token,
            silent,
        }
    }
}

impl Encodable for AuthResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        if self.silent {
            writer.tag_zero(b"AGUP");
        }
        writer.tag_str_empty(b"LDHT");
        writer.tag_zero(b"NTOS");
        writer.tag_str(b"PCTK", &self.session_token); // PC Authentication Token
        if self.silent {
            writer.tag_str_empty(b"PRIV");
            {
                writer.tag_group(b"SESS");
                writer.tag_u32(b"BUID", self.player_id);
                writer.tag_zero(b"FRST");
                writer.tag_str(b"KEY", &self.session_token); // Session Token
                writer.tag_zero(b"LLOG");
                writer.tag_str(b"MAIL", &self.email); // Player Email
                {
                    writer.tag_group(b"PDTL");
                    encode_persona(writer, self.player_id, &self.display_name); // Persona Details
                }
                writer.tag_u32(b"UID", self.player_id);
                writer.tag_group_end();
            }
        } else {
            writer.tag_list_start(b"PLST", TdfType::Group, 1);
            encode_persona(writer, self.player_id, &self.display_name);
            writer.tag_str_empty(b"PRIV");
            writer.tag_str(b"SKEY", &self.session_token);
        }
        writer.tag_zero(b"SPAM");
        writer.tag_str_empty(b"THST");
        writer.tag_str_empty(b"TSUI");
        writer.tag_str_empty(b"TURI");
        if !self.silent {
            writer.tag_u32(b"UID", self.player_id);
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

impl Decodable for CreateAccountRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let email: String = reader.tag("MAIL")?;
        let password: String = reader.tag("PASS")?;
        Ok(Self { email, password })
    }
}

pub struct PersonaResponse {
    player_id: PlayerID,
    email: String,
    display_name: String,
    session_token: String,
}

impl PersonaResponse {
    /// Creates a new auth response from the provided player, session token
    ///
    /// `player`        The player that was authenticated
    /// `session_token` The session token to use
    pub fn new(player: &Player, session_token: String) -> Self {
        Self {
            player_id: player.id,
            email: player.email.clone(),
            display_name: player.display_name.clone(),
            session_token,
        }
    }
}

impl Encodable for PersonaResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u32(b"BUID", self.player_id);
        writer.tag_zero(b"FRST");
        writer.tag_str(b"KEY", &self.session_token);
        writer.tag_zero(b"LLOG");
        writer.tag_str(b"MAIL", &self.email);

        writer.tag_group(b"PDTL");
        encode_persona(writer, self.player_id, &self.display_name);
        writer.tag_u32(b"UID", self.player_id);
    }
}

/// Request for listing entitlements
pub struct ListEntitlementsRequest {
    /// The entitlements tag
    pub tag: String,
}

impl Decodable for ListEntitlementsRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let tag: String = reader.tag("ETAG")?;
        Ok(Self { tag })
    }
}

/// Response of an entitlements list
pub struct ListEntitlementsResponse {
    pub list: Vec<Entitlement>,
}

impl Encodable for ListEntitlementsResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"NLST", &self.list);
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

impl Encodable for Entitlement {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_str_empty(b"DEVI");
        writer.tag_str(b"GDAY", "2012-12-15T16:15Z");
        writer.tag_str(b"GNAM", self.name);
        writer.tag_u64(b"ID", self.id);
        writer.tag_u8(b"ISCO", 0);
        writer.tag_u8(b"PID", 0);
        writer.tag_str(b"PJID", self.pjid);
        writer.tag_u8(b"PRCA", self.prca);
        writer.tag_str(b"PRID", self.prid);
        writer.tag_u8(b"STAT", 1);
        writer.tag_u8(b"STRC", 0);
        writer.tag_str(b"TAG", self.tag);
        writer.tag_str_empty(b"TDAY");
        writer.tag_u8(b"TTYPE", self.ty);
        writer.tag_u8(b"UCNT", 0);
        writer.tag_u8(b"VER", 0);
        writer.tag_group_end();
    }
}

value_type!(Entitlement, TdfType::Group);

/// Structure for a request to send a forgot password email. Currently
/// only logs that a reset was requested and doesn't actually send
/// an email.
pub struct ForgotPasswordRequest {
    /// The email of the account that needs a password reset
    pub email: String,
}

impl Decodable for ForgotPasswordRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let email: String = reader.tag("MAIL")?;
        Ok(Self { email })
    }
}

/// Dummy structure for the LegalDocsInfo response. None of the
/// values in this struct ever change.
pub struct LegalDocsInfo;

impl Encodable for LegalDocsInfo {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_zero(b"EAMC");
        writer.tag_str_empty(b"LHST");
        writer.tag_zero(b"PMC");
        writer.tag_str_empty(b"PPUI");
        writer.tag_str_empty(b"TSUI");
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

impl Encodable for LegalContent<'_> {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_str(b"LDVC", self.path);
        writer.tag_u16(b"TCOL", self.col);
        writer.tag_str(b"TCOT", self.content);
    }
}

/// Response to the client requesting a shared token
pub struct GetTokenResponse {
    /// The generated shared token
    pub token: String,
}

impl Encodable for GetTokenResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_str(b"AUTH", &self.token)
    }
}
