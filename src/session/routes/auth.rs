use crate::{
    config::RuntimeConfig,
    database::{entities::Player, DatabaseConnection},
    services::{
        retriever::Retriever,
        sessions::{Sessions, VerifyError},
    },
    session::{
        models::{
            auth::*,
            errors::{GlobalError, ServerResult},
        },
        router::{Blaze, Extension, SessionAuth},
        SessionLink,
    },
    utils::hashing::{hash_password, verify_password},
};
use email_address::EmailAddress;
use log::{debug, error};
use std::{borrow::Cow, sync::Arc};
use tokio::fs::read_to_string;

pub async fn handle_login(
    session: SessionLink,
    Extension(db): Extension<DatabaseConnection>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Blaze(LoginRequest { email, password }): Blaze<LoginRequest>,
) -> ServerResult<Blaze<AuthResponse>> {
    // Ensure the email is actually valid
    if !EmailAddress::is_valid(&email) {
        return Err(AuthenticationError::InvalidEmail.into());
    }

    // Find a non origin player with that email
    let player: Player = Player::by_email(&db, &email)
        .await?
        .ok_or(AuthenticationError::InvalidUser)?;

    // Get the attached password (Passwordless accounts fail as invalid)
    let player_password: &str = player
        .password
        .as_ref()
        .ok_or(AuthenticationError::InvalidUser)?;

    // Ensure passwords match
    if !verify_password(&password, player_password) {
        return Err(AuthenticationError::InvalidPassword.into());
    }

    // Update the session stored player

    let player = session.set_player(player);
    sessions.add_session(player.id, Arc::downgrade(&session));

    let session_token: String = sessions.create_token(player.id);

    Ok(Blaze(AuthResponse {
        player,
        session_token,
        silent: false,
    }))
}

pub async fn handle_silent_login(
    session: SessionLink,
    Extension(db): Extension<DatabaseConnection>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Blaze(SilentLoginRequest { token }): Blaze<SilentLoginRequest>,
) -> ServerResult<Blaze<AuthResponse>> {
    // Verify the authentication token
    let player_id = sessions.verify_token(&token).map_err(|err| match err {
        VerifyError::Expired => AuthenticationError::ExpiredToken,
        VerifyError::Invalid => AuthenticationError::InvalidToken,
    })?;

    let player = Player::by_id(&db, player_id)
        .await?
        .ok_or(AuthenticationError::InvalidToken)?;

    // Update the session stored player
    let player = session.set_player(player);
    sessions.add_session(player.id, Arc::downgrade(&session));

    Ok(Blaze(AuthResponse {
        player,
        session_token: token,
        silent: true,
    }))
}

pub async fn handle_origin_login(
    session: SessionLink,
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<RuntimeConfig>>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Extension(retriever): Extension<Arc<Retriever>>,
    Blaze(OriginLoginRequest { token, .. }): Blaze<OriginLoginRequest>,
) -> ServerResult<Blaze<AuthResponse>> {
    // Obtain an origin flow
    let mut flow = retriever.origin_flow().await.map_err(|err| {
        error!("Failed to obtain origin flow: {}", err);
        GlobalError::System
    })?;

    let player: Player = flow.login(&db, token, &config).await.map_err(|err| {
        error!("Failed to login with origin: {}", err);
        GlobalError::System
    })?;

    // Update the session stored player
    let player = session.set_player(player);
    sessions.add_session(player.id, Arc::downgrade(&session));

    let session_token: String = sessions.create_token(player.id);

    Ok(Blaze(AuthResponse {
        player,
        session_token,
        silent: true,
    }))
}

/// Handles logging out by the client this removes any current player data from the
/// session and updating anything that depends on the session having a player.
///
/// ```
/// Route: Authentication(Logout)
/// ID: 8
/// Content: {}
/// ```
pub async fn handle_logout(
    session: SessionLink,
    SessionAuth(player): SessionAuth,
    Extension(sessions): Extension<Arc<Sessions>>,
) {
    session.clear_player();
    sessions.remove_session(player.id);
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
    Blaze(ListEntitlementsRequest { tag }): Blaze<ListEntitlementsRequest>,
) -> Option<Blaze<ListEntitlementsResponse>> {
    if !tag.is_empty() {
        return None;
    }

    Some(Blaze(ListEntitlementsResponse { list: ENTITLEMENTS }))
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
pub async fn handle_login_persona(SessionAuth(player): SessionAuth) -> Blaze<PersonaResponse> {
    Blaze(PersonaResponse { player })
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
pub async fn handle_forgot_password(Blaze(req): Blaze<ForgotPasswordRequest>) {
    debug!("Password reset request (Email: {})", req.email);
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
///     "OPT1": 0,
///     "OPT3": 0,
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
    session: SessionLink,
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<RuntimeConfig>>,
    Extension(sessions): Extension<Arc<Sessions>>,
    Blaze(CreateAccountRequest { email, password }): Blaze<CreateAccountRequest>,
) -> ServerResult<Blaze<AuthResponse>> {
    if !EmailAddress::is_valid(&email) {
        return Err(AuthenticationError::InvalidEmail.into());
    }

    if Player::by_email(&db, &email).await?.is_some() {
        // Handle email address is already in use
        return Err(AuthenticationError::Exists.into());
    }

    // Hash the proivded plain text password using Argon2
    let hashed_password: String = hash_password(&password).map_err(|err| {
        error!("Failed to hash password for creating account: {}", err);
        GlobalError::System
    })?;

    // Create a default display name from the first 99 chars of the email
    let display_name: String = email.chars().take(99).collect::<String>();

    // Create a new player
    let player: Player =
        Player::create(&db, email, display_name, Some(hashed_password), &config).await?;

    let player = session.set_player(player);
    sessions.add_session(player.id, Arc::downgrade(&session));

    let session_token = sessions.create_token(player.id);

    Ok(Blaze(AuthResponse {
        player,
        session_token,
        silent: false,
    }))
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
pub async fn handle_get_legal_docs_info() -> Blaze<LegalDocsInfo> {
    Blaze(LegalDocsInfo)
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
pub async fn handle_tos() -> Blaze<LegalContent> {
    let content = read_to_string("data/terms_of_service.html")
        .await
        .map(Cow::Owned)
        .unwrap_or(Cow::Borrowed(
            "<h1>This is a terms of service placeholder</h1>",
        ));

    Blaze(LegalContent {
        col: 0xdaed,
        content,
        path: "webterms/au/en/pc/default/09082020/02042022",
    })
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
pub async fn handle_privacy_policy() -> Blaze<LegalContent> {
    let content = read_to_string("data/privacy_policy.html")
        .await
        .map(Cow::Owned)
        .unwrap_or(Cow::Borrowed(
            "<h1>This is a privacy policy placeholder</h1>",
        ));

    Blaze(LegalContent {
        col: 0xc99c,
        content,
        path: "webprivacy/au/en/pc/default/08202020/02042022",
    })
}

/// Handles retrieving an authentication token for use with the Galaxy At War HTTP service.
/// This implementation uses the session token for the player
///
/// ```
/// Route: Authentication(GetAuthToken),
/// ID: 35
/// Content: {}
/// ```
pub async fn handle_get_auth_token(
    SessionAuth(player): SessionAuth,
    Extension(sessions): Extension<Arc<Sessions>>,
) -> Blaze<GetTokenResponse> {
    // Create a new token claim for the player to use with the API
    let token = sessions.create_token(player.id);
    Blaze(GetTokenResponse { token })
}
