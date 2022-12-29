use crate::{
    blaze::{
        components::Authentication,
        errors::{ServerError, ServerResult},
    },
    servers::main::{models::auth::*, routes::HandleResult, session::SessionAddr},
    state::GlobalState,
    utils::{
        env,
        hashing::{hash_password, verify_password},
        random::generate_random_string,
        types::PlayerID,
        validate::is_email,
    },
};
use blaze_pk::packet::Packet;
use database::{DatabaseConnection, Player};
use log::{debug, error, warn};
use std::borrow::Cow;
use std::path::Path;
use tokio::fs::read_to_string;

/// Routing function for handling packets with the `Authentication` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of the packet recieved
/// `packet`    The recieved packet
pub async fn route(
    session: SessionAddr,
    component: Authentication,
    packet: &Packet,
) -> HandleResult {
    match component {
        Authentication::Logout => handle_logout(session, packet),
        Authentication::SilentLogin | Authentication::OriginLogin | Authentication::Login => {
            handle_auth_request(session, packet).await
        }
        Authentication::LoginPersona => handle_login_persona(session, packet).await,
        Authentication::ListUserEntitlements2 => handle_list_entitlements(packet),
        Authentication::CreateAccount => handle_create_account(session, packet).await,
        Authentication::PasswordForgot => handle_forgot_password(packet),
        Authentication::GetLegalDocsInfo => handle_get_legal_docs_info(packet),
        Authentication::GetTermsOfServiceConent => {
            handle_legal_content(packet, LegalType::TermsOfService).await
        }
        Authentication::GetPrivacyPolicyContent => {
            handle_legal_content(packet, LegalType::PrivacyPolicy).await
        }
        Authentication::GetAuthToken => handle_get_auth_token(session, packet).await,
        _ => Ok(packet.respond_empty()),
    }
}

/// This route handles all the different authentication types, Silent, Origin,
/// and Login parsing  the request and handling the authentication with the
/// correct function.
///
/// # Silent Login
///
/// This is the silent token authentication packet which is when the client
/// authenticates with an already known session token and player ID
///
/// ```
/// Route: Authentication(SilentLogin)
/// ID: 6
/// Content: {
///     "AUTH": "128_CHARACTER_TOKEN", // Authentication token
///     "PID": 1, // Player ID
///     "TYPE": 2 // Authentication type
/// }
/// ```
///
/// # Origin Login
///
/// This is the authentication packet used when the game is launched through
/// origin. This token must be authenticated through the official servers
///
/// ```
/// Route: Authentication(OriginLogin)
/// ID: 0
/// Content: {
///     "AUTH": "ORIGIN_TOKEN", // Origin authentication token
///     "TYPE": 1 // Authentication type
/// }
/// ```
///
/// # Login
///
/// This is login through the in game login menu using a username and
/// password.
///
/// ```
/// Route: Authentication(Login)
/// ID: 14
/// Content: {
///     "DVID": 0,
///     "MAIL": "ACCOUNT_EMAIL", // Email
///     "PASS": "ACCOUNT_PASSWORD", // Password
///     "TOKN": "",
///     "TYPE": 0 // Authentication type
/// }
/// ```
async fn handle_auth_request(session: SessionAddr, packet: &Packet) -> HandleResult {
    let req: AuthRequest = packet.decode()?;
    let silent = req.is_silent();
    let db = GlobalState::database();
    let player: Player = match req {
        AuthRequest::Silent { token, player_id } => handle_login_token(db, token, player_id).await,
        AuthRequest::Login { email, password } => handle_login_email(db, email, password).await,
        AuthRequest::Origin { token } => handle_login_origin(db, token).await,
    }?;

    let (player, session_token) = player
        .with_token(db, generate_random_string)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;

    session.set_player(Some(player.clone()));
    let response = AuthResponse {
        player,
        session_token,
        silent,
    };
    Ok(packet.respond(response))
}

/// Handles finding a player through an authentication token and a player ID
/// returning the player if found
///
/// `db`        The database connection
/// `token`     The authentication token
/// `player_id` The player ID
async fn handle_login_token(
    db: &DatabaseConnection,
    token: String,
    player_id: PlayerID,
) -> ServerResult<Player> {
    Player::by_id_with_token(db, player_id, &token)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::InvalidSession)
}

/// Handles finding a player through the provided email then ensuring the
/// account password hash matches the provided plain text password
///
/// `db`       The database connection
/// `email`    The email to find the account for
/// `password` The password to check the hash against
async fn handle_login_email(
    db: &DatabaseConnection,
    email: String,
    password: String,
) -> ServerResult<Player> {
    // Ensure the email is actually valid
    if !is_email(&email) {
        return Err(ServerError::InvalidEmail);
    }

    // Find a non origin player with that email
    let player: Player = Player::by_email(db, &email, false)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::EmailNotFound)?;

    // Ensure passwords match
    if !verify_password(&password, &player.password) {
        return Err(ServerError::WrongPassword);
    }

    Ok(player)
}

/// Handles finding a player using the origin retriever logic. Connects to the
/// official servers  and uses the provided origin token to login then takes the
/// credentails from the official servers.
///
/// `db`    The database connection
/// `token` The origin authentication token
async fn handle_login_origin(
    db: &'static DatabaseConnection,
    token: String,
) -> ServerResult<Player> {
    // Only continue if Origin Fetch is actually enabled
    if !env::from_env(env::ORIGIN_FETCH) {
        return Err(ServerError::ServerUnavailable);
    }

    // Ensure the retriever is enabled
    let Some(retriever) = GlobalState::retriever() else {
        error!("Unable to authenticate Origin: Retriever is disabled or unavailable");
        return Err(ServerError::ServerUnavailable);
    };

    // Create an origin authentication flow
    let Some(mut flow) = retriever.create_origin_flow().await else {
        error!("Unable to authenticate Origin: Unable to connect to official servers");
        return Err(ServerError::ServerUnavailable);
    };

    // Authenticate with the official servers
    let Some(details) = flow.authenticate(token).await else {
        error!("Unable to authenticate Origin: Failed to retrieve details from official server");
        return Err(ServerError::ServerUnavailable);
    };

    // Lookup the player details to see if the player exists
    let player: Option<Player> = Player::by_email(db, &details.email, true)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;

    if let Some(player) = player {
        return Ok(player);
    }

    let player: Player =
        Player::create(db, details.email, details.display_name, String::new(), true)
            .await
            .map_err(|_| ServerError::ServerUnavailable)?;

    // Early return created player if origin fetching is disabled
    if !env::from_env(env::ORIGIN_FETCH_DATA) {
        return Ok(player);
    }

    // Load the player settings from origin
    let Some(settings) = flow.get_settings().await else {
        warn!(
            "Unable to load origin player settings from official servers (Name: {}, Email: {})",
            &player.display_name, &player.email
        );
        return Ok(player);
    };

    if let Err(err) = player.bulk_insert_data(db, settings.into_iter()).await {
        error!("Failed to set origin data: {err:?}");
        return Err(ServerError::ServerUnavailable);
    }

    Ok(player)
}

/// Handles logging out by the client this removes any current player data from the
/// session and updating anything that depends on the session having a player.
///
/// ```
/// Route: Authentication(Logout)
/// ID: 8
/// Content: {}
/// ```
fn handle_logout(session: SessionAddr, packet: &Packet) -> HandleResult {
    session.set_player(None);
    Ok(packet.respond_empty())
}

/// Handles list user entitlements 2 responses requests which contains information
/// about certain content the user has access two
///
/// ```
/// Route: Authentication(ListUserEntitlements2)
/// ID: 8
/// Content: {
///     "BUID": 0,
///     "EPSN": 1,
///     "EPSZ": 50,
///     "ETAG": "",
///     "GDAY": "",
///     "GNLS": List<String> ["ME3PCOffers", "ME3PCContent", "ME3GenOffers", "ME3GenContent", "ME3GenAncillary"],
///     "HAUP": 0,
///     "PJID": "",
///     "PRID": "",
///     "RECU": 0,
///     "STAT": 0,
///     "TERD": "",
///     "TYPE": 0
/// }
/// ```
fn handle_list_entitlements(packet: &Packet) -> HandleResult {
    let req: ListEntitlementsRequest = packet.decode()?;
    let tag: String = req.tag;
    if !tag.is_empty() {
        return Ok(packet.respond_empty());
    }
    // Skip formatting these entitlement creations
    #[rustfmt::skip]
    let list = vec![
        // Project 10 = $10 Multiplayer Pass (Entitlement Required For Online Access)
        Entitlement::new_pc(0xec5090c43f,"303107",2,"DR:229644400","PROJECT10_CODE_CONSUMED",1),
        Entitlement::new_pc(0xec3e4d793f,"304141",2,"DR:230773600","PROJECT10_CODE_CONSUMED_LE1",1),
        Entitlement::new_pc(0xec3e4d793f,"304141",2,"DR:230773600","PROJECT10_CODE_CONSUMED_LE1",1),
        // Jeeze so many online pass entitlements
        Entitlement::new_pc(0xec50b255ff,"300241",2,"OFB-MASS:44370","ONLINE_ACCESS",1),
        Entitlement::new_pc(0xec50a620ff,"300241",2,"OFB-MASS:49465","ONLINE_ACCESS",1),
        Entitlement::new_pc(0xec508db6ff,"303107",2,"DR:229644400","ONLINE_ACCESS",1),
        Entitlement::new_pc(0xec3e5393bf,"300241",2,"OFB-EAST:46112","ONLINE_ACCESS",1),
        Entitlement::new_pc(0xec3e50867f,"304141",2,"DR:230773600","ONLINE_ACCESS",1),
        Entitlement::new_gen(0xec4495bfff,"303107", 0,"","ONLINE_ACCESS_GAW_PC",1),
        Entitlement::new_gen(0xea234c3e7f,"303107", 2,"","ONLINE_ACCESS_GAW_XBL2",1),
        // Singleplayer DLC
        Entitlement::new_pc(0xec3e62d5ff,"300241",2,"OFB-MASS:51074","ME3_PRC_EXTENDEDCUT",5),
        Entitlement::new_pc(0xec50b5633f,"300241",2,"OFB-MASS:44370","ME3_PRC_PROTHEAN",5),
        Entitlement::new_pc(0xec3e56a0ff,"300241",2,"OFB-EAST:46112","ME3_PRC_PROTHEAN",5),
        Entitlement::new_pc(0xec50b8707f,"300241",2,"OFB-MASS:52001","ME3_PRC_LEVIATHAN",5),
        Entitlement::new_pc(0xec50ac3b7f,"300241",2,"OFB-MASS:55146","ME3_PRC_OMEGA",5),
        Entitlement::new_pc(0xec5093d17f,"300241",2,"OFB-EAST:58040","MET_BONUS_CONTENT_DW",5),
        Entitlement::new_pc(0xec50af48bf,"300241",2,"OFB-EAST:57550","ME3_PRC_CITADEL",5),
        // Singleplayer Packs
        Entitlement::new_pc(0xec50bb7dbf,"300241",2,"OFB-MASS:56984","ME3_MTX_APP01",5),
        Entitlement::new_pc(0xec5099ebff,"300241",2,"OFB-MASS:49032","ME3_MTX_GUN01",5),
        Entitlement::new_pc(0xec50c1983f,"300241",2,"OFB-MASS:55147","ME3_MTX_GUN02",5),
        // Multiplayer DLC
        Entitlement::new_pc(0xec50a0067f,"300241",2,"OFB-MASS:47872","ME3_PRC_RESURGENCE",5),
        Entitlement::new_pc(0xec50a92e3f,"300241",2,"OFB-MASS:49465","ME3_PRC_REBELLION",5),
        Entitlement::new_pc(0xec5096debf,"300241",2,"OFB-MASS:51073","ME3_PRC_EARTH",5),
        Entitlement::new_pc(0xec509cf93f,"300241",2,"OFB-MASS:52000","ME3_PRC_GOBIG",5),
        Entitlement::new_pc(0xec50a313bf,"300241",2,"OFB-MASS:59712","ME3_PRC_MP5",5),
        // Other
        Entitlement::new_pc(0xec81ae023f,"300241",2,"OFB-MASS:46111","ME3_PRO_M90_INDRA",5),
        Entitlement::new_pc(0xec81aaf4ff,"300241",2,"OFB-MASS:46110","ME3_PRO_AT12_RAIDER_PACK",5),
        Entitlement::new_pc(0xec81a7e7bf,"300241",2,"OFB-MASS:46033","ME3_PRO_M55_ARGUS",5),
        Entitlement::new_pc(0xec81a4da7f,"300241",2,"OFB-MASS:46032","ME3_PRO_N7_WARFARE_PACK",5),
        Entitlement::new_pc(0xec81a1cd3f,"300241",2,"OFB-MASS:46489","ME3_PRO_N7_WARFARE_PACK",5),

        
        // Collectors Edition
        Entitlement::new_pc(0xec3e5fc8bf,"300241",2,"OFB-MASS:46484","ME3_MTX_COLLECTORS_EDITION",5),
        Entitlement::new_pc(0xec3e5cbb7f,"300241",2,"OFB-MASS:46483","ME3_MTX_DIGITAL_ART_BOOKS",5),
        Entitlement::new_gen(0xec3e59ae3f,"300241",2,"OFB-MASS:46482","ME3_MTX_SOUNDTRACK",5),
        // Darkhorse Redeem Code (Character boosters and Collector Assault Rifle)
        Entitlement::new_pc(0xec50be8aff,"300241",2,"OFB-MASS:61524","ME3_PRC_DARKHORSECOMIC",5),
    ];
    let response = ListEntitlementsResponse { list };
    Ok(packet.respond(response))
}

/// Handles logging into a persona. This system doesn't implement the persona system so
/// the account details are just used instead
///
/// ```
/// Route: Authentication(LoginPersona),
/// ID: 14
/// Content: {
///     "PMAM": "Jacobtread"
/// }
/// ```
async fn handle_login_persona(session: SessionAddr, packet: &Packet) -> HandleResult {
    let player: Player = session
        .get_player()
        .await
        .ok_or(ServerError::FailedNoLoginAction)?;
    let session_token = player
        .session_token
        .clone()
        .ok_or(ServerError::FailedNoLoginAction)?;
    let response = PersonaResponse::new(player, session_token);
    Ok(packet.respond(response))
}

/// Handles forgot password requests. This normally would send a forgot password
/// email but this server does not yet implement that functionality so it is just
/// logged to debug output
///
/// ```
/// Route: Authentication(PasswordForgot)
/// ID: 17
/// Content: {
///     "MAIL": "ACCOUNT_EMAIL"
/// }
/// ```
fn handle_forgot_password(packet: &Packet) -> HandleResult {
    let req: ForgotPasswordRequest = packet.decode()?;
    if !is_email(&req.email) {
        return Err(ServerError::InvalidEmail.into());
    }
    debug!("Got request for password rest for email: {}", &req.email);
    Ok(packet.respond_empty())
}

/// Handles creating accounts
///
/// ```
/// Route: Authentication(CreateAccount)
/// ID: 18
/// Content: {
///     "BDAY": 0, // Birthday Day
///     "BMON": 0, // Birthday Month
///     "BYR": 0,  // Birthday Year
///     "CTRY": "NZ", // Country Code
///     "DVID": 0,
///     "GEST": 0,
///     "LANG": "en", // Language
///     "MAIL": "ACCOUNT_EMAIL",
///     "OPT": 0,
///     "OPT": 0,
///     "PASS": "ACCOUNT_PASSWORD",
///     "PNAM": "",
///     "PRIV": "webprivacy/au/en/pc/default/08202020/02042022", // Privacy policy path
///     "PRNT": "",
///     "PROF": {
///         "CITY": "",
///         "CTRY": "",
///         "GNDR": 0,
///         "STAT": "",
///         "STRT": "",
///         "ZIP": ""
///     },
///     "TOSV": "webterms/au/en/pc/default/09082020/02042022", // Terms of service path
///     "TSUI": "webterms/au/en/pc/default/09082020/02042022" // Terms of service path
/// }
/// ```
///
async fn handle_create_account(session: SessionAddr, packet: &Packet) -> HandleResult {
    let req: CreateAccountRequest = packet.decode()?;
    let email = req.email;
    if !is_email(&email) {
        return Err(ServerError::InvalidEmail.into());
    }

    let db = GlobalState::database();
    let email_exists = Player::is_email_taken(db, &email).await?;
    if email_exists {
        return Err(ServerError::EmailAlreadyInUse.into());
    }

    let hashed_password = hash_password(&req.password).map_err(|err| {
        error!("Failed to hash passsword: {err:?}");
        ServerError::ServerUnavailable
    })?;

    let display_name = email.chars().take(99).collect::<String>();
    let player: Player = Player::create(db, email, display_name, hashed_password, false).await?;
    let (player, session_token) = player
        .with_token(db, generate_random_string)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;

    session.set_player(Some(player.clone()));

    let response = AuthResponse {
        player,
        session_token,
        silent: false,
    };

    Ok(packet.respond(response))
}

/// Expected to be getting information about the legal docs however the exact meaning
/// of the response content is not yet known and further research is required
///
/// ```
/// Route: Authentication(GetLegalDocsInfo)
/// ID: 22
/// Content: {
///     "CTRY": "",
///     "PTFM": "pc" // Platform
/// }
/// ```
fn handle_get_legal_docs_info(packet: &Packet) -> HandleResult {
    Ok(packet.respond(LegalDocsInfo))
}

/// Type for deciding which legal document to respond with
enum LegalType {
    TermsOfService,
    PrivacyPolicy,
}

impl LegalType {
    async fn load(&self) -> (Cow<'static, str>, &'static str, u16) {
        let (local_path, web_path, col) = match self {
            Self::TermsOfService => (
                "data/terms_of_service.html",
                "webterms/au/en/pc/default/09082020/02042022",
                0xdaed,
            ),
            Self::PrivacyPolicy => (
                "data/privacy_policy.html",
                "webprivacy/au/en/pc/default/08202020/02042022",
                0xc99c,
            ),
        };
        let path = Path::new(local_path);
        if path.exists() && path.is_file() {
            if let Ok(value) = read_to_string(path).await {
                return (Cow::Owned(value), web_path, col);
            }
        }
        let fallback = match self {
            Self::TermsOfService => {
                include_str!("../../../resources/defaults/terms_of_service.html")
            }
            Self::PrivacyPolicy => include_str!("../../../resources/defaults/privacy_policy.html"),
        };

        (Cow::Borrowed(fallback), web_path, col)
    }
}

/// Handles serving the contents of the terms of service and privacy policy.
/// These are HTML documents which is rendered inside the game when you click
/// the button for viewing terms of service or the privacy policy.
///
/// ```
/// Route: Authentication(GetTermsOfServiceContent)
/// ID: 23
/// Content: {
///     "CTRY": "",
///     "LANG": "",
///     "PTFM": "pc",
///     "TEXT": 1
/// }
/// ```
/// ```
/// Route: Authentication(GetPrivacyPolicyContent)
/// ID: 24
/// Content: {
///     "CTRY": "",
///     "LANG": "",
///     "PTFM": "pc",
///     "TEXT": 1
/// }
/// ```
async fn handle_legal_content(packet: &Packet, ty: LegalType) -> HandleResult {
    let (content, path, col) = ty.load().await;
    let response = LegalContent {
        path,
        content: &content,
        col,
    };
    Ok(packet.respond(response))
}

/// Handles retrieving an authentication token for use with the Galaxy At War HTTP service.
/// This implementation uses the session token for the player
///
/// ```
/// Route: Authentication(GetAuthToken),
/// ID: 35
/// Content: {}
/// ```
async fn handle_get_auth_token(session: SessionAddr, packet: &Packet) -> HandleResult {
    let token: String = session
        .get_player()
        .await
        .map(|player| format!("{:X}", player.id))
        .ok_or(ServerError::FailedNoLoginAction)?;
    let response = GetTokenResponse { token };
    Ok(packet.respond(response))
}
