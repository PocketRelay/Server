use tdf::{TdfDeserialize, TdfSerialize, TdfSerializer, TdfType, TdfTyped};

use crate::{database::entities::Player, utils::types::PlayerID};
use std::{borrow::Cow, sync::Arc};

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum AuthenticationError {
    InvalidUser = 0xb,
    InvalidPassword = 0xc,
    InvalidToken = 0xd,
    ExpiredToken = 0xe,
    Exists = 0xf,
    TooYoung = 0x10,
    NoAccount = 0x11,
    PersonaNotFound = 0x12,
    PersonaInactive = 0x13,
    InvalidPMail = 0x14,
    InvalidField = 0x15,
    InvalidEmail = 0x16,
    InvalidStatus = 0x17,
    InvalidSessionKey = 0x1f,
    PersonaBanned = 0x20,
    InvalidPersona = 0x21,
    Banned = 0x2b,
    FieldInvalidChars = 0xc9,
    FieldTooShort = 0xca,
    FieldTooLong = 0xcb,
}

/// Login through login prompt menu with email and password
/// ```
/// {
///     "DVID": 0,
///     "MAIL": "ACCOUNT_EMAIL",
///     "PASS": "ACCOUNT_PASSWORD",
///     "TOKN": "",
///     "TYPE": 0
/// }
/// ```
#[derive(TdfDeserialize)]
pub struct LoginRequest {
    /// The email addresss of the account to login with
    #[tdf(tag = "MAIL")]
    pub email: String,
    /// The plain text password of the account to login to
    #[tdf(tag = "PASS")]
    pub password: String,
}

/// Silent token based authentication with player ID
/// ```
/// {
///     "AUTH": "AUTH_TOKEN",
///     "PID": 1,
///     "TYPE": 2
/// }
/// ```
#[derive(TdfDeserialize)]
pub struct SilentLoginRequest {
    /// The authentication token previously provided to the client
    /// on a previous successful authentication attempt
    #[tdf(tag = "AUTH")]
    pub token: String,
}

/// Authentication through origin token
/// ```
/// {
///     "AUTH": "ORIGIN_AUTH_TOKEN",
///     "TYPE": 1
/// }
/// ```
#[derive(TdfDeserialize, TdfSerialize)]
pub struct OriginLoginRequest {
    /// The token generated by Origin
    #[tdf(tag = "AUTH")]
    pub token: String,
    #[tdf(tag = "TYPE")]
    pub ty: u8,
}

/// Encodes a mock persona from the provided player using its
/// display name and ID as the values
///
/// `writer`       The Tdf writer to use for writing the values
/// `id`           The id of the player to write for
/// `display_name` The display name of the player
fn encode_persona<S: TdfSerializer>(w: &mut S, id: PlayerID, display_name: &str) {
    w.group_body(|w| {
        w.tag_str(b"DSNM", display_name);
        w.tag_zero(b"LAST");
        w.tag_u32(b"PID", id);
        w.tag_zero(b"STAS");
        w.tag_zero(b"XREF");
        w.tag_zero(b"XTYP");
    });
}

/// Structure for the response to an authentication request.
pub struct AuthResponse {
    /// The authenticated player
    pub player: Arc<Player>,
    /// The session token for the completed authentication
    pub session_token: String,
    /// Whether the authentication proccess was silent
    pub silent: bool,
}

impl TdfSerialize for AuthResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        if self.silent {
            w.tag_zero(b"AGUP");
        }
        w.tag_str_empty(b"LDHT");
        w.tag_zero(b"NTOS");
        w.tag_str(b"PCTK", &self.session_token); // PC Authentication Token
        if self.silent {
            w.tag_str_empty(b"PRIV");
            {
                w.group(b"SESS", |writer| {
                    writer.tag_owned(b"BUID", self.player.id);
                    writer.tag_zero(b"FRST");
                    writer.tag_str(b"KEY", &format!("{:X}", self.player.id)); // Session Token
                    writer.tag_zero(b"LLOG");
                    writer.tag_str(b"MAIL", &self.player.email); // Player Email
                    {
                        writer.tag_group(b"PDTL");
                        encode_persona(writer, self.player.id, &self.player.display_name);
                        // Persona Details
                    }
                    writer.tag_owned(b"UID", self.player.id);
                });
            }
        } else {
            w.tag_list_start(b"PLST", TdfType::Group, 1);
            encode_persona(w, self.player.id, &self.player.display_name);
            w.tag_str_empty(b"PRIV");
            w.tag_str(b"SKEY", &format!("{:X}", self.player.id));
        }
        w.tag_zero(b"SPAM");
        w.tag_str_empty(b"THST");
        w.tag_str_empty(b"TSUI");
        w.tag_str_empty(b"TURI");
        if !self.silent {
            w.tag_owned(b"UID", self.player.id);
        }
    }
}

/// Structure for request to create a new account with
/// the provided email and password
#[derive(TdfDeserialize)]
pub struct CreateAccountRequest {
    /// The email address of the account to create
    #[tdf(tag = "MAIL")]
    pub email: String,
    /// The password of the account to create
    #[tdf(tag = "PASS")]
    pub password: String,
}

/// Structure for the persona response which contains details
/// about the current persona. Which in this case is just the
/// player details
pub struct PersonaResponse {
    /// The player
    pub player: Arc<Player>,
}

impl TdfSerialize for PersonaResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_owned(b"BUID", self.player.id);
        w.tag_zero(b"FRST");
        w.tag_str(b"KEY", &format!("{:X}", self.player.id));
        w.tag_zero(b"LLOG");
        w.tag_str(b"MAIL", &self.player.email);

        w.tag_group(b"PDTL");
        encode_persona(w, self.player.id, &self.player.display_name);
        w.tag_owned(b"UID", self.player.id);
    }
}

/// Request for listing entitlements
#[derive(TdfDeserialize)]
pub struct ListEntitlementsRequest {
    /// The entitlements tag
    #[tdf(tag = "ETAG")]
    pub tag: String,
}

/// Response of an entitlements list
#[derive(TdfSerialize)]
pub struct ListEntitlementsResponse {
    #[tdf(tag = "NLST")]
    pub list: &'static [Entitlement],
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

    pub const fn new_pc(
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

    pub const fn new_gen(
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

impl TdfSerialize for Entitlement {
    fn serialize<S: TdfSerializer>(&self, w: &mut S) {
        w.tag_str_empty(b"DEVI");
        w.tag_str(b"GDAY", "2012-12-15T16:15Z");
        w.tag_str(b"GNAM", self.name);
        w.tag_u64(b"ID", self.id);
        w.tag_u8(b"ISCO", 0);
        w.tag_u8(b"PID", 0);
        w.tag_str(b"PJID", self.pjid);
        w.tag_u8(b"PRCA", self.prca);
        w.tag_str(b"PRID", self.prid);
        w.tag_u8(b"STAT", 1);
        w.tag_u8(b"STRC", 0);
        w.tag_str(b"TAG", self.tag);
        w.tag_str_empty(b"TDAY");
        w.tag_u8(b"TYPE", self.ty);
        w.tag_u8(b"UCNT", 0);
        w.tag_u8(b"VER", 0);
        w.tag_group_end();
    }
}

impl TdfTyped for Entitlement {
    const TYPE: TdfType = TdfType::Group;
}

/// Structure for a request to send a forgot password email. Currently
/// only logs that a reset was requested and doesn't actually send
/// an email.
#[derive(TdfDeserialize)]
pub struct ForgotPasswordRequest {
    /// The email of the account that needs a password reset
    #[tdf(tag = "MAIL")]
    pub email: String,
}

/// Dummy structure for the LegalDocsInfo response. None of the
/// values in this struct ever change.
pub struct LegalDocsInfo;

impl TdfSerialize for LegalDocsInfo {
    fn serialize<S: TdfSerializer>(&self, w: &mut S) {
        w.tag_zero(b"EAMC");
        w.tag_str_empty(b"LHST");
        w.tag_zero(b"PMC");
        w.tag_str_empty(b"PPUI");
        w.tag_str_empty(b"TSUI");
    }
}

/// Structure for legal content responses such as the Privacy Policy
/// and the terms and condition.
#[derive(TdfSerialize)]
pub struct LegalContent {
    /// The url path to the legal content (Prefix this value with https://tos.ea.com/legalapp/ to get the url)
    #[tdf(tag = "LDVC")]
    pub path: &'static str,
    /// Unknown value
    #[tdf(tag = "TCOL")]
    pub col: u16,
    /// The actual HTML content of the legal document
    #[tdf(tag = "TCOT")]
    pub content: Cow<'static, str>,
}

/// Response to the client requesting a shared token
#[derive(TdfSerialize)]
pub struct GetTokenResponse {
    /// The generated shared token
    #[tdf(tag = "AUTH")]
    pub token: String,
}
