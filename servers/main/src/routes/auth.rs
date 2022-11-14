use blaze_pk::{codec::Codec, packet, packet::Packet, tagging::tag_value};

use core::blaze::components::Authentication;
use core::blaze::errors::{BlazeError, HandleResult, ServerError};
use core::blaze::session::SessionArc;
use core::blaze::shared::{AuthRes, Entitlement, LegalDocsInfo, Sess, TermsContent};
use database::{players, PlayersInterface};
use log::{debug, error, warn};
use utils::{
    hashing::{hash_password, verify_password},
    validate::is_email,
};

/// Routing function for handling packets with the `Authentication` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(
    session: &SessionArc,
    component: Authentication,
    packet: &Packet,
) -> HandleResult {
    match component {
        Authentication::SilentLogin => handle_silent_login(session, packet).await,
        Authentication::Logout => handle_logout(session, packet).await,
        Authentication::Login => handle_login(session, packet).await,
        Authentication::ListUserEntitlements2 => {
            handle_list_user_entitlements_2(session, packet).await
        }
        Authentication::CreateAccount => handle_create_account(session, packet).await,
        Authentication::LoginPersona => handle_login_persona(session, packet).await,
        Authentication::PasswordForgot => handle_forgot_password(session, packet).await,
        Authentication::GetLegalDocsInfo => handle_get_legal_docs_info(session, packet).await,
        Authentication::GetTermsOfServiceConent => {
            handle_terms_of_service_content(session, packet).await
        }
        Authentication::GetPrivacyPolicyContent => {
            handle_privacy_policy_content(session, packet).await
        }
        Authentication::GetPasswordRules => handle_get_password_rules(session, packet).await,
        Authentication::GetAuthToken => handle_get_auth_token(session, packet).await,
        Authentication::OriginLogin => handle_origin_login(session, packet).await,
        component => {
            debug!("Got Authentication({component:?})");
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
async fn handle_silent_login(session: &SessionArc, packet: &Packet) -> HandleResult {
    let silent_login = packet.decode::<SilentLoginReq>()?;
    let id = silent_login.id;
    let token = silent_login.token;

    debug!("Attempted silent authentication: {id} ({token})");

    let Some(player) = PlayersInterface::by_id(session.db(), id).await? else {
        return session.response_error_empty(packet, ServerError::InvalidSession).await;
    };

    if player.session_token.ne(&Some(token)) {
        return session
            .response_error_empty(packet, ServerError::InvalidSession)
            .await;
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
pub async fn complete_auth(
    session: &SessionArc,
    packet: &Packet,
    player: players::Model,
    silent: bool,
) -> HandleResult {
    debug!("Completing authentication");
    session.set_player(Some(player)).await;
    debug!("Set player");
    let session_token = session.session_token().await?;
    debug!("Session token: {}", session_token);
    let session_data = session.data.read().await;
    let Some(player) = session_data.player.as_ref() else {
        error!("Failed to complete auth player was somehow missing");
        return session.response_empty(packet).await;
    };

    let response = AuthRes {
        sess: Sess {
            session_token,
            player,
        },
        silent,
    };

    debug!("Sending session response");
    session.response(packet, &response).await
}

/// Handles logging out by the client this removes any current player data from the
/// session and updating anything that depends on the session having a player.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.LOGOUT, 0x0, 0x7) {}
/// ```
async fn handle_logout(session: &SessionArc, packet: &Packet) -> HandleResult {
    debug!("Logging out for session: (ID: {})", &session.id);
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
async fn handle_login(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<AccountReq>()?;
    let email = req.email;
    let password = req.password;

    if !is_email(&email) {
        debug!(
            "Client attempted to login with invalid email address: {}",
            &email
        );
        return session
            .response_error_empty(packet, ServerError::InvalidEmail)
            .await;
    }

    let Some(player) = PlayersInterface::by_email(session.db(), &email, false).await? else {
        return session
            .response_error_empty(packet, ServerError::EmailNotFound)
            .await;
    };

    debug!("Attempting login for {}", player.email);

    if !verify_password(&password, &player.password) {
        debug!("Client provided password did not match stored hash");
        return session
            .response_error_empty(packet, ServerError::WrongPassword)
            .await;
    }

    complete_auth(session, packet, player, false).await?;
    Ok(())
}

/// Handles creating accounts
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.CREATE_ACCOUNT, 0x12) {
///   number("BDAY", 0x0)
///   number("BMON", 0x0)
///   number("BYR", 0x0)
///   text("CTRY", "NZ")
///   number("DVID", 0x0)
///   number("GEST", 0x0)
///   text("LANG", "en")
///   text("MAIL", "EMAIL OMITTED")
///   number("OPT", 0x0)
///   number("OPT", 0x0)
///   text("PASS", "PASSWORD OMITTED")
///   text("PNAM")
///   text("PRIV", "webprivacy/au/en/pc/default/08202020/02042022")
///   text("PRNT")
///   +group("PROF") {
///     text("CITY")
///     text("CTRY")
///     number("GNDR", 0x0)
///     text("STAT")
///     text("STRT")
///     text("ZIP")
///   }
///   text("TOSV", "webterms/au/en/pc/default/09082020/02042022")
///   text("TSUI", "webterms/au/en/pc/default/09082020/02042022")
/// }
/// ```
///
async fn handle_create_account(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<AccountReq>()?;
    let email = req.email;
    let password = req.password;

    if !is_email(&email) {
        return session
            .response_error_empty(packet, ServerError::InvalidEmail)
            .await;
    }

    let email_exists = PlayersInterface::is_email_taken(session.db(), &email).await?;

    if email_exists {
        return session
            .response_error_empty(packet, ServerError::EmailAlreadyInUse)
            .await;
    }

    let hashed_password =
        hash_password(&password).map_err(|_| BlazeError::Other("Failed to hash user password"))?;

    let display_name = if email.len() > 99 {
        email[0..99].to_string()
    } else {
        email.clone()
    };

    let player =
        PlayersInterface::create(session.db(), email, display_name, hashed_password, false).await?;

    complete_auth(session, packet, player, false).await?;
    Ok(())
}

packet! {
    struct OriginLoginReq {
        AUTH token: String
    }
}

/// Handles logging in with a session token provided by Origin rather than with email
/// and password. This requires connecting to the official server to get the correct
/// credentials.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.ORIGIN_LOGIN, INCOMING_TYPE, 0x0) {
///   text("AUTH", "ORIGIN TOKEN OMITTED")
///   number("TYPE", 0x1)
/// }
/// ```
async fn handle_origin_login(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<OriginLoginReq>()?;
    debug!("Origin login request with token: {}", &req.token);
    let Some(retriever) = session.retriever() else {
        debug!("Unable to authenticate Origin user retriever is disabled or unavailable.");
        return session.response_empty(packet).await
    };

    let player = retriever.get_origin_player(session.db(), req.token).await;
    let Some(player) = player else {
        debug!("Unable to authenticate Origin failed to retrieve user");
        return session.response_empty(packet).await
    };

    debug!("Origin authentication success");
    debug!("ID = {}", &player.id);
    debug!("Username = {}", &player.display_name);
    debug!("Email = {}", &player.email);

    complete_auth(session, packet, player, true).await?;
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
        tag_value(output, "NLST", &self.list);
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
async fn handle_list_user_entitlements_2(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<LUEReq>()?;
    let tag = req.tag;
    if !tag.is_empty() {
        return session.response_empty(packet).await;
    }
    const PC_TAG: &str = "ME3PCOffers";
    const GEN_TAG: &str = "ME3GenOffers";
    let list = vec![
        // Project 10 = $10 Multiplayer Pass (Entitlement Required For Online Access)
        Entitlement::new_pc(
            0xec5090c43f,
            "303107",
            2,
            "DR:229644400",
            "PROJECT10_CODE_CONSUMED",
            1,
        ),
        Entitlement::new_pc(
            0xec3e4d793f,
            "304141",
            2,
            "DR:230773600",
            "PROJECT10_CODE_CONSUMED_LE1",
            1,
        ),
        Entitlement::new_pc(
            0xec3e4d793f,
            "304141",
            2,
            "DR:230773600",
            "PROJECT10_CODE_CONSUMED_LE1",
            1,
        ),
        // Jeeze so many online pass entitlements
        Entitlement::new_pc(
            0xec50b255ff,
            "300241",
            2,
            "OFB-MASS:44370",
            "ONLINE_ACCESS",
            1,
        ),
        Entitlement::new_pc(
            0xec50a620ff,
            "300241",
            2,
            "OFB-MASS:49465",
            "ONLINE_ACCESS",
            1,
        ),
        Entitlement::new_pc(
            0xec508db6ff,
            "303107",
            2,
            "DR:229644400",
            "ONLINE_ACCESS",
            1,
        ),
        Entitlement::new_pc(
            0xec3e5393bf,
            "300241",
            2,
            "OFB-EAST:46112",
            "ONLINE_ACCESS",
            1,
        ),
        Entitlement::new_pc(
            0xec3e50867f,
            "304141",
            2,
            "DR:230773600",
            "ONLINE_ACCESS",
            1,
        ),
        Entitlement::new_gen(0xec4495bfff, "303107", 0, "", "ONLINE_ACCESS_GAW_PC", 1),
        Entitlement::new_gen(0xea234c3e7f, "303107", 2, "", "ONLINE_ACCESS_GAW_XBL2", 1),
        // Singleplayer DLC
        Entitlement::new_pc(
            0xec3e62d5ff,
            "300241",
            2,
            "OFB-MASS:51074",
            "ME3_PRC_EXTENDEDCUT",
            5,
        ),
        Entitlement::new_pc(
            0xec50b5633f,
            "300241",
            2,
            "OFB-MASS:44370",
            "ME3_PRC_PROTHEAN",
            5,
        ),
        Entitlement::new_pc(
            0xec3e56a0ff,
            "300241",
            2,
            "OFB-EAST:46112",
            "ME3_PRC_PROTHEAN",
            5,
        ),
        Entitlement::new_pc(
            0xec50b8707f,
            "300241",
            2,
            "OFB-MASS:52001",
            "ME3_PRC_LEVIATHAN",
            5,
        ),
        Entitlement::new_pc(
            0xec50ac3b7f,
            "300241",
            2,
            "OFB-MASS:55146",
            "ME3_PRC_OMEGA",
            5,
        ),
        Entitlement::new_pc(
            0xec5093d17f,
            "300241",
            2,
            "OFB-EAST:58040",
            "MET_BONUS_CONTENT_DW",
            5,
        ),
        // Singleplayer Packs
        Entitlement::new_pc(
            0xec50bb7dbf,
            "300241",
            2,
            "OFB-MASS:56984",
            "ME3_MTX_APP01",
            5,
        ),
        Entitlement::new_pc(
            0xec5099ebff,
            "300241",
            2,
            "OFB-MASS:49032",
            "ME3_MTX_GUN01",
            5,
        ),
        Entitlement::new_pc(
            0xec50c1983f,
            "300241",
            2,
            "OFB-MASS:55147",
            "ME3_MTX_GUN02",
            5,
        ),
        // Multiplayer DLC
        Entitlement::new_pc(
            0xec50a0067f,
            "300241",
            2,
            "OFB-MASS:47872",
            "ME3_PRC_RESURGENCE",
            5,
        ),
        Entitlement::new_pc(
            0xec50a92e3f,
            "300241",
            2,
            "OFB-MASS:49465",
            "ME3_PRC_REBELLION",
            5,
        ),
        Entitlement::new_pc(
            0xec5096debf,
            "300241",
            2,
            "OFB-MASS:51073",
            "ME3_PRC_EARTH",
            5,
        ),
        Entitlement::new_pc(
            0xec509cf93f,
            "300241",
            2,
            "OFB-MASS:52000",
            "ME3_PRC_GOBIG",
            5,
        ),
        Entitlement::new_pc(
            0xec50a313bf,
            "300241",
            2,
            "OFB-MASS:59712",
            "ME3_PRC_MP5",
            5,
        ),
        // Collectors Edition
        Entitlement::new_pc(
            0xec3e5fc8bf,
            "300241",
            2,
            "OFB-MASS:46484",
            "ME3_MTX_COLLECTORS_EDITION",
            5,
        ),
        Entitlement::new_pc(
            0xec3e5cbb7f,
            "300241",
            2,
            "OFB-MASS:46483",
            "ME3_MTX_DIGITAL_ART_BOOKS",
            5,
        ),
        Entitlement::new_gen(
            0xec3e59ae3f,
            "300241",
            2,
            "OFB-MASS:46482",
            "ME3_MTX_SOUNDTRACK",
            5,
        ),
        // Darkhorse Redeem Code (Character boosters and Collector Assault Rifle)
        Entitlement::new_pc(
            0xec50be8aff,
            "300241",
            2,
            "OFB-MASS:61524",
            "ME3_PRC_DARKHORSECOMIC",
            5,
        ),
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
async fn handle_login_persona(session: &SessionArc, packet: &Packet) -> HandleResult {
    debug!("Logging into persona");
    let session_token = session.session_token().await?;
    let session_data = session.data.read().await;

    let Some(player) = session_data.player.as_ref() else {
        warn!("Client attempted to login to persona without being authenticated");
        return session
            .response_error_empty(packet, ServerError::FailedNoLoginAction)
            .await;
    };
    let response = Sess {
        session_token,
        player,
    };
    debug!("Persona login complete");
    session.response(packet, &response).await
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
async fn handle_forgot_password(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<ForgotPaswdReq>()?;
    if !is_email(&req.email) {
        return session
            .response_error_empty(packet, ServerError::InvalidEmail)
            .await;
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
async fn handle_get_legal_docs_info(session: &SessionArc, packet: &Packet) -> HandleResult {
    session.response(packet, &LegalDocsInfo).await
}

/// The default terms of service document
const DEFAULT_TERMS_OF_SERVICE: &str = include_str!("../resources/defaults/term_of_service.html");

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
async fn handle_terms_of_service_content(session: &SessionArc, packet: &Packet) -> HandleResult {
    // TODO: Attempt to load local terms of service before reverting to default
    session
        .response(
            packet,
            &TermsContent {
                path: "webterms/au/en/pc/default/09082020/02042022",
                content: DEFAULT_TERMS_OF_SERVICE,
                col: 0xdaed,
            },
        )
        .await
}

/// The default privacy policy document
const DEFAULT_PRIVACY_POLICY: &str = include_str!("../resources/defaults/privacy_policy.html");

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
async fn handle_privacy_policy_content(session: &SessionArc, packet: &Packet) -> HandleResult {
    // TODO: Attempt to load local privacy policy before reverting to default
    session
        .response(
            packet,
            &TermsContent {
                path: "webprivacy/au/en/pc/default/08202020/02042022",
                content: DEFAULT_PRIVACY_POLICY,
                col: 0xc99c,
            },
        )
        .await
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
async fn handle_get_password_rules(session: &SessionArc, packet: &Packet) -> HandleResult {
    session.response(packet, &PasswordRules {
        max_length: 99,
        min_length: 4,
        valid_chars: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789[]`!@#$%^&*()_={}:;<>+-',.~?/|\\",
    }).await
}

packet! {
    struct GetAuthRes {
        AUTH auth: String
    }
}

/// Handles retrieving an authentication token for use with the Galaxy At War HTTP service
/// however in this case we are just using the player ID in hex format as the token.
///
/// # Structure
/// ```
/// packet(Components.AUTHENTICATION, Commands.GET_AUTH_TOKEN, 0x23) {}
/// ```
async fn handle_get_auth_token(session: &SessionArc, packet: &Packet) -> HandleResult {
    let session_data = session.data.read().await;
    let Some(player) = session_data.player.as_ref() else {
        warn!("Client attempted to get auth token while not authenticated. (SID: {})", session.id);
        return session
            .response_error_empty(packet, ServerError::FailedNoLoginAction)
            .await;
    };
    let value = format!("{:X}", player.id);
    session.response(packet, &GetAuthRes { auth: value }).await
}
