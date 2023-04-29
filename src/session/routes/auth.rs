use crate::{
    database::{entities::Player, DatabaseConnection},
    services::{
        retriever::{origin::OriginFlowService, Retriever},
        tokens::Tokens,
        Services,
    },
    session::{
        models::{
            auth::*,
            errors::{ServerError, ServerResult},
        },
        GetPlayerIdMessage, GetPlayerMessage, SessionLink, SetPlayerMessage,
    },
    state::App,
    utils::hashing::{hash_password, verify_password},
};
use email_address::EmailAddress;
use log::{debug, error};
use std::borrow::Cow;
use tokio::fs::read_to_string;

pub async fn handle_login(
    session: &mut SessionLink,
    req: LoginRequest,
) -> ServerResult<AuthResponse> {
    let db: &DatabaseConnection = App::database();

    let LoginRequest { email, password } = &req;

    // Ensure the email is actually valid
    if !EmailAddress::is_valid(email) {
        return Err(ServerError::InvalidEmail);
    }

    // Find a non origin player with that email
    let player: Player = Player::by_email(db, email)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::EmailNotFound)?;

    // Get the attached password (Passwordless accounts fail as invalid)
    let player_password: &str = player
        .password
        .as_ref()
        .ok_or(ServerError::InvalidAccount)?;

    // Ensure passwords match
    if !verify_password(password, player_password) {
        return Err(ServerError::WrongPassword);
    }

    // Update the session stored player
    session
        .send(SetPlayerMessage(Some(player.clone())))
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;

    let session_token: String = Tokens::service_claim(player.id);

    Ok(AuthResponse {
        player,
        session_token,
        silent: false,
    })
}

pub async fn handle_silent_login(
    session: &mut SessionLink,
    req: SilentLoginRequest,
) -> ServerResult<AuthResponse> {
    let db: &DatabaseConnection = App::database();

    // Verify the authentication token
    let player: Player = Tokens::service_verify(db, &req.token)
        .await
        .map_err(|_| ServerError::InvalidSession)?;

    // Update the session stored player
    session
        .send(SetPlayerMessage(Some(player.clone())))
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;

    Ok(AuthResponse {
        player,
        session_token: req.token,
        silent: true,
    })
}

pub async fn handle_origin_login(
    session: &mut SessionLink,
    req: OriginLoginRequest,
) -> ServerResult<AuthResponse> {
    let db: &DatabaseConnection = App::database();

    let services: &Services = App::services();

    // Ensure the retriever is enabled
    let retriever: &Retriever = match &services.retriever {
        Some(value) => value,
        None => {
            error!("Unable to authenticate Origin: Retriever is disabled or unavailable");
            return Err(ServerError::ServerUnavailable);
        }
    };

    // Ensure origin authentication is enabled
    let service: &OriginFlowService = match &retriever.origin_flow {
        Some(value) => value,
        None => {
            error!("Origin authentication is disabled cannot authenticate origin client");
            return Err(ServerError::ServerUnavailable);
        }
    };

    // Create an origin authentication flow
    let mut flow = match service.create(retriever).await {
        Some(value) => value,
        None => {
            error!("Unable to authenticate Origin: Unable to connect to official servers");
            return Err(ServerError::ServerUnavailable);
        }
    };

    let player: Player = match flow.login(db, req.token).await {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to login with origin: {}", err);
            return Err(ServerError::ServerUnavailable);
        }
    };

    // Update the session stored player
    session
        .send(SetPlayerMessage(Some(player.clone())))
        .await
        .map_err(|_| ServerError::ServerUnavailable)?;

    let session_token: String = Tokens::service_claim(player.id);

    Ok(AuthResponse {
        player,
        session_token,
        silent: true,
    })
}

/// Handles logging out by the client this removes any current player data from the
/// session and updating anything that depends on the session having a player.
///
/// ```
/// Route: Authentication(Logout)
/// ID: 8
/// Content: {}
/// ```
pub async fn handle_logout(session: &mut SessionLink) {
    let _ = session.send(SetPlayerMessage(None)).await;
}

// Skip formatting these entitlement creations
#[rustfmt::skip]
static ENTITLEMENTS: &[Entitlement; 34] = &[
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
pub async fn handle_list_entitlements(
    req: ListEntitlementsRequest,
) -> Option<ListEntitlementsResponse> {
    let tag: String = req.tag;
    if !tag.is_empty() {
        return None;
    }

    Some(ListEntitlementsResponse { list: ENTITLEMENTS })
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
pub async fn handle_login_persona(session: &mut SessionLink) -> ServerResult<PersonaResponse> {
    let player: Player = session
        .send(GetPlayerMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;
    Ok(PersonaResponse { player })
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
pub async fn handle_forgot_password(req: ForgotPasswordRequest) -> ServerResult<()> {
    debug!("Password reset request (Email: {})", req.email);
    Ok(())
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
pub async fn handle_create_account(
    session: &mut SessionLink,
    req: CreateAccountRequest,
) -> ServerResult<AuthResponse> {
    let email = req.email;
    if !EmailAddress::is_valid(&email) {
        return Err(ServerError::InvalidEmail);
    }

    let db = App::database();

    match Player::by_email(db, &email).await {
        // Continue normally for non taken emails
        Ok(None) => {}
        // Handle email address is already in use
        Ok(Some(_)) => return Err(ServerError::EmailAlreadyInUse),
        // Handle database error while checking taken
        Err(err) => {
            error!("Unable to check if email '{email}' is already taken: {err:?}");
            return Err(ServerError::ServerUnavailable);
        }
    }

    // Hash the proivded plain text password using Argon2
    let hashed_password: String = match hash_password(&req.password) {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to hash password for creating account: {err:?}");
            return Err(ServerError::ServerUnavailable);
        }
    };

    // Create a default display name from the first 99 chars of the email
    let display_name: String = email.chars().take(99).collect::<String>();

    // Create a new player
    let player: Player = match Player::create(db, email, display_name, Some(hashed_password)).await
    {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to create player: {err:?}");
            return Err(ServerError::ServerUnavailable);
        }
    };

    // Failing to set the player likely the player disconnected or
    // the server is shutting down
    if session
        .send(SetPlayerMessage(Some(player.clone())))
        .await
        .is_err()
    {
        return Err(ServerError::ServerUnavailable);
    }

    let session_token = Tokens::service_claim(player.id);

    Ok(AuthResponse {
        player,
        session_token,
        silent: false,
    })
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
pub async fn handle_get_legal_docs_info() -> LegalDocsInfo {
    LegalDocsInfo
}

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
pub async fn handle_tos() -> LegalContent {
    let content = match read_to_string("data/terms_of_service.html").await {
        Ok(value) => Cow::Owned(value),
        Err(_) => Cow::Borrowed("<h1>This is a terms of service placeholder</h1>"),
    };

    LegalContent {
        col: 0xdaed,
        content,
        path: "webterms/au/en/pc/default/09082020/02042022",
    }
}

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
pub async fn handle_privacy_policy() -> LegalContent {
    let content = match read_to_string("data/privacy_policy.html").await {
        Ok(value) => Cow::Owned(value),
        Err(_) => Cow::Borrowed("<h1>This is a privacy policy placeholder</h1>"),
    };

    LegalContent {
        col: 0xc99c,
        content,
        path: "webprivacy/au/en/pc/default/08202020/02042022",
    }
}

/// Handles retrieving an authentication token for use with the Galaxy At War HTTP service.
/// This implementation uses the session token for the player
///
/// ```
/// Route: Authentication(GetAuthToken),
/// ID: 35
/// Content: {}
/// ```
pub async fn handle_get_auth_token(session: &mut SessionLink) -> ServerResult<GetTokenResponse> {
    let player_id = session
        .send(GetPlayerIdMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;
    // Create a new token claim for the player to use with the API
    let token = Tokens::service_claim(player_id);
    Ok(GetTokenResponse { token })
}
