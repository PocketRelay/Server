use std::ops::Deref;
use blaze_pk::{Codec, CodecError, CodecResult, OpaquePacket, packet, Packets, Reader, tag_list_start, ValueType};
use log::debug;
use regex::Regex;
use crate::blaze::components::Authentication;
use crate::blaze::errors::{BlazeError, HandleResult, LoginError, LoginErrorRes};
use crate::blaze::Session;
use crate::blaze::shared::{AuthRes, Entitlement, LegalDocsInfo, Sess, TermsContent};
use crate::database::entities::PlayerModel;
use crate::database::interface::players;
use crate::database::interface::players::find_by_email;
use crate::utils::hashing::{hash_password, verify_password};

/// Routing function for handling packets with the `Authentication` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: Authentication, packet: &OpaquePacket) -> HandleResult {
    match component {
        Authentication::SilentLogin => handle_silent_login(session, packet).await,
        Authentication::Logout => handle_logout(session, packet).await,
        Authentication::Login => handle_login(session, packet).await,
        Authentication::ListUserEntitlements2 => handle_list_user_entitlements_2(session, packet).await,
        Authentication::CreateAccount => handle_create_account(session, packet).await,
        Authentication::LoginPersona => handle_login_persona(session, packet).await,
        Authentication::PasswordForgot => handle_forgot_password(session, packet).await,
        Authentication::GetLegalDocsInfo => handle_get_legal_docs_info(session, packet).await,
        Authentication::GetTermsOfServiceConent => handle_terms_of_service_content(session, packet).await,
        Authentication::GetPrivacyPolicyContent => handle_privacy_policy_content(session, packet).await,
        Authentication::GetPasswordRules => handle_get_password_rules(session, packet).await,


        component => {
            debug!("Got {component:?}");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}


packet! {
    struct SilentLoginReq {
        AUTH token: String,
        PID id: u32,
    }
}

/// Creates a new blaze error response from the provided login error
fn login_error(packet: &OpaquePacket, error: LoginError) -> BlazeError {
    BlazeError::Response(Packets::error(packet, error, &LoginErrorRes::default()))
}

/// Handles silent authentication from a client (Token based authentication) If the token provided
/// by the client is correct the session is updated accordingly to match the player
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.SILENT_LOGIN, 0x6) {
///   text("AUTH", "128 CHAR TOKEN OMITTED")
///   number("PID", 0x1)
///   number("TYPE", 0x2)
/// }
/// ```
///
async fn handle_silent_login(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let silent_login = packet.contents::<SilentLoginReq>()?;
    let id = silent_login.id;
    let token = silent_login.token;

    debug!("Attempted silent authentication: {id} ({token})");

    let player = players::find_by_id(session.db(), id)
        .await?
        .ok_or_else(|| login_error(packet, LoginError::InvalidSession))?;

    if player.session_token.ne(&Some(token)) {
        return Err(login_error(packet, LoginError::InvalidSession));
    }

    debug!("Silent authentication success");
    debug!("ID = {}", &player.id);
    debug!("Username = {}", &player.display_name);
    debug!("Email = {}", &player.email);

    complete_auth(session, packet, player, true).await?;
    Ok(())
}

/// Completes the authentication process for the provided session using the provided Player
/// Model as the authenticated player.
async fn complete_auth(session: &Session, packet: &OpaquePacket, player: PlayerModel, silent: bool) -> HandleResult {
    session.set_player(Some(player)).await;
    let session_token = session.session_token().await?;
    let session_data = session.data.read().await;
    let player = session_data.expect_player()?;
    let response = AuthRes {
        sess: Sess {
            session_data: session_data.deref(),
            session_token,
            player,
        },
        silent,
    };

    session.response(packet, &response).await?;
    if silent {
        session.update_for(session).await?;
    }
    Ok(())
}

/// Handles logging out by the client this removes any current player data from the
/// session and updating anything that depends on the session having a player.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.LOGOUT, 0x0, 0x7) {}
/// ```
async fn handle_logout(session: &Session, packet: &OpaquePacket) -> HandleResult {
    debug!("Logging out for session:");
    debug!("ID = {}", &session.id);
    session.set_player(None).await;
    session.response_empty(packet).await
}

packet! {
    struct AccountReq {
        MAIL email: String,
        PASS password: String
    }
}

/// Handles logging into an account with the email and password provided. This is
/// when the login prompt appears in game
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.LOGIN, 0xe) {
///   number("DVID", 0x0)
///   text("MAIL", "EMAIL OMITTED")
///   text("PASS", "PASSWORD OMITTED")
///   text("TOKN", "")
///   number("TYPE", 0x0)
/// }
/// ```
async fn handle_login(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<AccountReq>()?;
    let email = req.email;
    let password = req.password;

    if !is_email(&email)? {
        return Err(login_error(packet, LoginError::InvalidEmail));
    }

    let player = find_by_email(session.db(), &email)
        .await?
        .ok_or_else(|| login_error(packet, LoginError::EmailNotFound))?;

    if !verify_password(&password, &player.password) {
        return Err(login_error(packet, LoginError::WrongPassword));
    }

    complete_auth(session, packet, player, false).await?;
    Ok(())
}

fn is_email(email: &str) -> bool {
    let regex = Regex::new(r#"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-.][a-z0-9]+)*\.[a-z]{2,6})"#)
        .unwrap();
    regex.is_match(email)
}

/// Handles creating accounts
async fn handle_create_account(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<AccountReq>()?;
    let email = req.email;
    let password = req.password;

    if !is_email(&email)? {
        return Err(login_error(packet, LoginError::InvalidEmail));
    }

    let email_exists = find_by_email(session.db(), &email)
        .await?
        .is_some();

    if email_exists {
        return Err(login_error(packet, LoginError::EmailAlreadyInUse));
    }

    let hashed_password = hash_password(&password)
        .map_err(|_| BlazeError::Other("Failed to hash user password"))?;

    let player = players::create_normal(session.db(), email, hashed_password).await?;

    complete_auth(session, packet, player, false).await?;
    Ok(())
}

packet! {
    struct LUEReq {
        ETAG tag: String
    }
}

#[derive(Debug)]
struct LUERes<'a> {
    list: Vec<Entitlement<'a>>,
}

impl Codec for LUERes<'_> {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_list_start(output, "NLST", ValueType::Group, self.list.len());
        for value in &self.list {
            value.encode(output);
        }
    }

    fn decode(_reader: &mut Reader) -> CodecResult<Self> {
        Err(CodecError::InvalidAction("Not allowed to decode"))
    }
}


/// Handles list user entitlements 2 responses requests which contains information
/// about certain content the user has access two
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.LIST_USER_ENTITLEMENTS_2, 0x8) {
///   number("BUID", 0x0)
///   number("EPSN", 0x1)
///   number("EPSZ", 0x32)
///   text("ETAG", "")
///   text("GDAY", "")
///   list("GNLS", listOf("ME3PCOffers", "ME3PCContent", "ME3GenOffers", "ME3GenContent", "ME3GenAncillary"))
///   number("HAUP", 0x0)
///   text("PJID", "")
///   text("PRID", "")
///   number("RECU", 0x0)
///   number("STAT", 0x0)
///   text("TERD", "")
///   number("TYPE", 0x0)
/// }
/// ```
async fn handle_list_user_entitlements_2(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<LUEReq>()?;
    let tag = req.tag;
    if !tag.is_empty() {
        return session.response_empty(packet).await;
    }
    const PC_TAG: &str = "ME3PCOffers";
    const GEN_TAG: &str = "ME3GenOffers";
    let list = vec![
        // Project 10 = $10 Multiplayer Pass (Entitlement Required For Online Access)
        Entitlement::new_pc(0xec5090c43f, "303107", 2, "DR:229644400", "PROJECT10_CODE_CONSUMED", 1),
        Entitlement::new_pc(0xec3e4d793f, "304141", 2, "DR:230773600", "PROJECT10_CODE_CONSUMED_LE1", 1),
        Entitlement::new_pc(0xec3e4d793f, "304141", 2, "DR:230773600", "PROJECT10_CODE_CONSUMED_LE1", 1),

        // Jeeze so many online pass entitlements
        Entitlement::new_pc(0xec50b255ff, "300241", 2, "OFB-MASS:44370", "ONLINE_ACCESS", 1),
        Entitlement::new_pc(0xec50a620ff, "300241", 2, "OFB-MASS:49465", "ONLINE_ACCESS", 1),
        Entitlement::new_pc(0xec508db6ff, "303107", 2, "DR:229644400", "ONLINE_ACCESS", 1),
        Entitlement::new_pc(0xec3e5393bf, "300241", 2, "OFB-EAST:46112", "ONLINE_ACCESS", 1),
        Entitlement::new_pc(0xec3e50867f, "304141", 2, "DR:230773600", "ONLINE_ACCESS", 1),
        Entitlement::new_gen(0xec4495bfff, "303107", 0, "", "ONLINE_ACCESS_GAW_PC", 1),
        Entitlement::new_gen(0xea234c3e7f, "303107", 2, "", "ONLINE_ACCESS_GAW_XBL2", 1),

        // Singleplayer DLC
        Entitlement::new_pc(0xec3e62d5ff, "300241", 2, "OFB-MASS:51074", "ME3_PRC_EXTENDEDCUT", 5),
        Entitlement::new_pc(0xec50b5633f, "300241", 2, "OFB-MASS:44370", "ME3_PRC_PROTHEAN", 5),
        Entitlement::new_pc(0xec3e56a0ff, "300241", 2, "OFB-EAST:46112", "ME3_PRC_PROTHEAN", 5),
        Entitlement::new_pc(0xec50b8707f, "300241", 2, "OFB-MASS:52001", "ME3_PRC_LEVIATHAN", 5),
        Entitlement::new_pc(0xec50ac3b7f, "300241", 2, "OFB-MASS:55146", "ME3_PRC_OMEGA", 5),
        Entitlement::new_pc(0xec5093d17f, "300241", 2, "OFB-EAST:58040", "MET_BONUS_CONTENT_DW", 5),

        // Singleplayer Packs
        Entitlement::new_pc(0xec50bb7dbf, "300241", 2, "OFB-MASS:56984", "ME3_MTX_APP01", 5),
        Entitlement::new_pc(0xec5099ebff, "300241", 2, "OFB-MASS:49032", "ME3_MTX_GUN01", 5),
        Entitlement::new_pc(0xec50c1983f, "300241", 2, "OFB-MASS:55147", "ME3_MTX_GUN02", 5),

        // Multiplayer DLC
        Entitlement::new_pc(0xec50a0067f, "300241", 2, "OFB-MASS:47872", "ME3_PRC_RESURGENCE", 5),
        Entitlement::new_pc(0xec50a92e3f, "300241", 2, "OFB-MASS:49465", "ME3_PRC_REBELLION", 5),
        Entitlement::new_pc(0xec5096debf, "300241", 2, "OFB-MASS:51073", "ME3_PRC_EARTH", 5),
        Entitlement::new_pc(0xec509cf93f, "300241", 2, "OFB-MASS:52000", "ME3_PRC_GOBIG", 5),
        Entitlement::new_pc(0xec50a313bf, "300241", 2, "OFB-MASS:59712", "ME3_PRC_MP5", 5),

        // Collectors Edition
        Entitlement::new_pc(0xec3e5fc8bf, "300241", 2, "OFB-MASS:46484", "ME3_MTX_COLLECTORS_EDITION", 5),
        Entitlement::new_pc(0xec3e5cbb7f, "300241", 2, "OFB-MASS:46483", "ME3_MTX_DIGITAL_ART_BOOKS", 5),
        Entitlement::new_gen(0xec3e59ae3f, "300241", 2, "OFB-MASS:46482", "ME3_MTX_SOUNDTRACK", 5),

        // Darkhorse Redeem Code (Character boosters and Collector Assault Rifle)
        Entitlement::new_pc(0xec50be8aff, "300241", 2, "OFB-MASS:61524", "ME3_PRC_DARKHORSECOMIC", 5),
    ];
    let response = LUERes { list };
    session.response(packet, &response).await
}

/// Handles logging into a persona. This system doesn't implement the persona system so
/// the account details are just used instead
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.LOGIN_PERSONA, 0xe) {
///   text("PNAM", "Jacobtread")
/// }
/// ```
async fn handle_login_persona(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let session_token = session.session_token().await?;
    let session_data = session.data.read().await;
    let player = session_data.expect_player()?;
    let response = Sess {
        session_data: session_data.deref(),
        session_token,
        player,
    };
    session.response(packet, &response).await?;
    session.update_for(session).await?;
    Ok(())
}


packet! {
    struct ForgotPaswdReq {
        MAIL email: String
    }
}

/// Handles forgot password requests. This normally would send a forgot password
/// email but this server does not yet implement that functionality so it is just
/// logged to debug output
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.PASSWORD_FORGOT, 0x11) {
///   text("MAIL", "EMAIL OMITTED")
/// }
/// ```
async fn handle_forgot_password(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<ForgotPaswdReq>()?;
    if !is_email(&req.email) {
        return Err(login_error(packet, LoginError::InvalidEmail))
    }
    debug!("Got request for password rest for email: {}", &req.email);
    session.response_empty(packet).await
}

/// Expected to be getting information about the legal docs however the exact meaning
/// of the response content is not yet known and further research is required
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.GET_LEGAL_DOCS_INFO, 0x16) {
///   text("CTRY", "") // Country?
///   text("PTFM", "pc") // Platform?
/// }
/// ```
async fn handle_get_legal_docs_info(session: &Session, packet: &OpaquePacket) -> HandleResult {
    session.response(packet, &LegalDocsInfo).await
}

/// The default terms of service document
const DEFAULT_TERMS_OF_SERVICE: &str = include_str!("../../../resources/defaults/term_of_service.html");

/// Handles serving the contents of the terms of service. This is an HTML document which is
/// rendered inside the game when you click the button for viewing terms of service.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.GET_TERMS_OF_SERVICE_CONTENT, 0x17) {
///   text("CTRY", "")
///   text("LANG", "")
///   text("PTFM", "pc")
///   number("TEXT", 0x1)
/// }
/// ```
///
async fn handle_terms_of_service_content(session: &Session, packet: &OpaquePacket) -> HandleResult {
    // TODO: Attempt to load local terms of service before reverting to default
    session.response(packet, &TermsContent {
        path: "webterms/au/en/pc/default/09082020/02042022",
        content: DEFAULT_TERMS_OF_SERVICE,
        col: 0xdaed
    }).await
}

/// The default privacy policy document
const DEFAULT_PRIVACY_POLICY: &str = include_str!("../../../resources/defaults/privacy_policy.html");

/// Handles serving the contents of the privacy policy. This is an HTML document which is
/// rendered inside the game when you click the button for viewing privacy policy.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.GET_PRIVACY_POLICY_CONTENT, 0x18) {
///   text("CTRY", "")
///   text("LANG", "")
///   text("PTFM", "pc")
///   number("TEXT", 0x1)
/// }
/// ```
///
async fn handle_privacy_policy_content(session: &Session, packet: &OpaquePacket) -> HandleResult {
    // TODO: Attempt to load local privacy policy before reverting to default
    session.response(packet, &TermsContent {
        path: "webprivacy/au/en/pc/default/08202020/02042022",
        content: DEFAULT_PRIVACY_POLICY,
        col: 0xc99c
    }).await
}

packet! {
    struct PasswordRules {
        MAXS max_length: u32,
        MINS min_length: u32,
        VDCH valid_chars: &'static str,
    }
}

/// Handles returning the password rules for creating passwords in the client.
///
/// # Structure
/// *To be recorded*.
async fn handle_get_password_rules(session: &Session, packet: &OpaquePacket) -> HandleResult {
    session.response(packet, &PasswordRules {
        max_length: 99,
        min_length: 4,
        valid_chars: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789[]`!@#$%^&*()_={}:;<>+-',.~?/|\\"
    })
}

async fn handle_get_auth_token(session: &Session, packet: &OpaquePacket) -> HandleResult {

}