use crate::{
    blaze::{
        codec::Port,
        components::{Components as C, Util as U},
        errors::{BlazeResult, ServerError, ServerResult},
    },
    servers::main::{models::util::*, session::SessionAddr},
    state::GlobalState,
    utils::{constants, dmap::load_dmap, env},
};
use base64;
use blaze_pk::{router::Router, types::TdfMap};
use flate2::{write::ZlibEncoder, Compression};
use log::{error, warn};
use rust_embed::RustEmbed;
use std::{
    io::Write,
    path::Path,
    str::Chars,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::fs::read;

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, SessionAddr>) {
    router.route(C::Util(U::PreAuth), handle_pre_auth);
    router.route_stateful(C::Util(U::PostAuth), handle_post_auth);
    router.route(C::Util(U::Ping), handle_ping);
    router.route(C::Util(U::FetchClientConfig), handle_fetch_client_config);
    router.route(C::Util(U::SuspendUserPing), handle_suspend_user_ping);
    router.route_stateful(C::Util(U::UserSettingsSave), handle_user_settings_save);
    router.route(C::Util(U::GetTelemetryServer), handle_get_telemetry_server);
    router.route(C::Util(U::GetTickerServer), handle_get_ticker_server);
    router.route_stateful(
        C::Util(U::UserSettingsLoadAll),
        handle_user_settings_load_all,
    );
}

/// Handles retrieving the details about the telemetry server
///
/// ```
/// Route: Util(GetTelemetryServer)
/// ID: 0
/// Content: {}
/// ```
///
async fn handle_get_telemetry_server() -> TelemetryServer {
    TelemetryServer { port: 9988 }
}

/// Handles retrieving the details about the ticker server
///
/// ```
/// Route: Util(GetTickerServer)
/// ID: 0
/// Content: {}
/// ```
///
async fn handle_get_ticker_server() -> TickerServer {
    TickerServer { port: 8999 }
}

/// Handles responding to pre-auth requests which is the first request
/// that clients will send. The response to this contains information
/// about the server the client is connecting to.
///
/// ```
/// Route: Util(PreAuth)
/// ID: 0
/// Content: {
///     "CDAT": {
///         "IITO": 0,
///         "LANG": 0x656e4e5a,
///         "SVCN": "masseffect-3-pc",
///         "TYPE": 0
///     },
///     "CINF": {
///         "BSDK": "3.15.6.0", // Blaze SDK version
///         "BTIM": "Dec 21 2012 12:46:51", // Likely Blaze SDK build time
///         "CLNT": "MassEffect3-pc", // Type of client
///         "CSKU": "134845",
///         "CVER": "05427.124",
///         "DSDK": "8.14.7.1",
///         "ENV": "prod", // Client build environment
///         "LOC": 0x656e4e5a,
///         "MAC": "7c:10:c9:28:33:35", // Client mac address
///         "PLAT": "Windows"
///     },
///     "FCCR": {
///         "CFID": "BlazeSDK"
///     }
/// }
/// ```
async fn handle_pre_auth() -> PreAuthResponse {
    let qos_port: Port = env::from_env(env::HTTP_PORT);
    PreAuthResponse { qos_port }
}

/// Handles post authentication requests. This provides information about other
/// servers that are used by Mass Effect such as the Telemetry and Ticker servers.
///
/// ```
/// Route: Util(PostAuth)
/// ID: 27
/// Content: {}
/// ```
async fn handle_post_auth(session: SessionAddr) -> ServerResult<PostAuthResponse> {
    let player_id = session
        .get_player()
        .await
        .map(|value| value.id)
        .ok_or(ServerError::FailedNoLoginAction)?;

    session.update_self();
    Ok(PostAuthResponse {
        telemetry: TelemetryServer { port: 9988 },
        ticker: TickerServer { port: 8999 },
        player_id,
    })
}

/// Handles ping update requests. These are sent by the client at the interval
/// specified in the pre-auth response. The server replies to this messages with
/// the current server unix timestamp in seconds.
///
/// ```
/// Route: Util(Ping)
/// ID: 1
/// Content: {}
/// ```
///
async fn handle_ping() -> PingResponse {
    let now = SystemTime::now();
    let server_time = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    PingResponse { server_time }
}

/// Contents of the entitlements dmap file
const ME3_ENT: &str = include_str!("../../../resources/data/entitlements.dmap");
/// Contents of the dime.xml file
const ME3_DIME: &str = include_str!("../../../resources/data/dime.xml");

/// Handles the client requesting to fetch a configuration from the server. The different
/// types of configuration are as follows:
/// - **ME3_DATA**: See `data_config` for more details
/// - **ME3_MSG**: Initial messages for the client
/// - **ME3_DIME**: Appears to be data relating to the in game shop configuration
/// - **ME3_BINI_VERSION**: Version and name for the server Coalesced
/// - **ME3_BINI_PC_COMPRESSED**: The server Coalesced file contents packed into a compressed format
/// - **ME3_LIVE_TLK_PC_{LANG}**: Game talk files for the specified language
/// # Structure
/// ```
/// Route: Util(FetchClientConfig)
/// ID: 2
/// Content: {
///     "CFID": "ME3_DATA"
/// }
/// ```
async fn handle_fetch_client_config(req: FetchConfigRequest) -> ServerResult<FetchConfigResponse> {
    let config = match req.id.as_ref() {
        "ME3_DATA" => data_config(),
        "ME3_MSG" => messages(),
        "ME3_ENT" => load_dmap(ME3_ENT),
        "ME3_DIME" => {
            let mut map = TdfMap::with_capacity(1);
            map.insert("Config", ME3_DIME);
            map
        }
        "ME3_BINI_VERSION" => {
            let mut map = TdfMap::with_capacity(2);
            map.insert("SECTION", "BINI_PC_COMPRESSED");
            map.insert("VERSION", "40128");
            map
        }
        "ME3_BINI_PC_COMPRESSED" => load_coalesced().await?,
        id => {
            if let Some(lang) = id.strip_prefix("ME3_LIVE_TLK_PC_") {
                talk_file(lang).await?
            } else {
                TdfMap::default()
            }
        }
    };

    Ok(FetchConfigResponse { config })
}

/// Loads the local coalesced if one is present falling back
/// to the default one on error or if its missing
async fn load_coalesced() -> ServerResult<ChunkMap> {
    let local_path = Path::new("data/coalesced.bin");
    if local_path.is_file() {
        let bytes = match read(local_path).await {
            Ok(value) => value,
            Err(_) => {
                error!("Unable to load local coalesced from data/coalesced.bin falling back to default.");
                return default_coalesced();
            }
        };
        match generate_coalesced(&bytes) {
            Ok(value) => Ok(value),
            Err(_) => {
                error!("Unable to compress local coalesced from data/coalesced.bin falling back to default.");
                default_coalesced()
            }
        }
    } else {
        default_coalesced()
    }
}

/// Generates the compressed version of the default coalesced
/// this default coalesced file is stored at
///
/// src/resources/data/coalesced.bin
fn default_coalesced() -> ServerResult<ChunkMap> {
    let bytes: &[u8] = include_bytes!("../../../resources/data/coalesced.bin");
    generate_coalesced(bytes)
}

/// Generates a compressed caolesced from the provided bytes
///
/// `bytes` The coalesced bytes
fn generate_coalesced(bytes: &[u8]) -> ServerResult<ChunkMap> {
    let compressed: Vec<u8> = {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
        encoder.write_all(bytes).map_err(|_| {
            error!("Failed to encode coalesced with ZLib (write stage)");
            ServerError::ServerUnavailable
        })?;
        encoder.finish().map_err(|_| {
            error!("Failed to encode coalesced with ZLib (finish stage)");
            ServerError::ServerUnavailable
        })?
    };

    let mut encoded = Vec::with_capacity(16 + compressed.len());
    encoded.extend_from_slice(b"NIBC");
    encoded.extend_from_slice(&1u32.to_le_bytes());
    encoded.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&compressed);
    Ok(create_base64_map(&encoded))
}

/// Type of a base64 chunks map
type ChunkMap = TdfMap<String, String>;

/// Converts to provided slice of bytes into an ordered TdfMap where
/// the keys are the chunk index and the values are the bytes encoded
/// as base64 chunks. The map contains a CHUNK_SIZE key which states
/// how large each chunk is and a DATA_SIZE key indicating the total
/// length of the chunked value
///
/// `bytes` The bytes to convert
fn create_base64_map(bytes: &[u8]) -> ChunkMap {
    // The size of the chunks
    const CHUNK_LENGTH: usize = 255;

    let encoded: String = base64::encode(bytes);
    let length = encoded.len();

    let mut output: ChunkMap = TdfMap::with_capacity(length / CHUNK_LENGTH);

    let mut chars: Chars = encoded.chars();
    let mut index = 0;

    loop {
        let mut value = String::with_capacity(CHUNK_LENGTH);
        let mut i = 0;
        while i < CHUNK_LENGTH {
            let next_char = match chars.next() {
                Some(value) => value,
                None => break,
            };
            value.push(next_char);
            i += 1;
        }
        output.insert(format!("CHUNK_{}", index), value);
        if i < CHUNK_LENGTH {
            break;
        }
        index += 1;
    }

    output.insert("CHUNK_SIZE", CHUNK_LENGTH.to_string());
    output.insert("DATA_SIZE", length.to_string());
    output.order();
    output
}

/// Retrieves a talk file for the specified language code falling back
/// to the `ME3_TLK_DEFAULT` default talk file if it could not be found
///
/// `lang` The talk file language
async fn talk_file(lang: &str) -> ServerResult<ChunkMap> {
    let file_name = format!("data/{}.tlk", lang);
    let local_path = Path::new(&file_name);

    if local_path.is_file() {
        let bytes = match read(local_path).await {
            Ok(value) => value,
            Err(_) => {
                error!("Unable to load local coalesced from data/coalesced.bin falling back to default.");
                return default_coalesced();
            }
        };
        Ok(create_base64_map(&bytes))
    } else {
        Ok(default_talk_file(lang))
    }
}

/// Default talk file values
#[derive(RustEmbed)]
#[folder = "src/resources/data/tlk"]
struct DefaultTlkFiles;

/// Generates the base64 map for the default talk file for the
/// provided langauge. Will default to the default.tlk file if
/// the language is not found
///
/// `lang` The language to get the default for
fn default_talk_file(lang: &str) -> ChunkMap {
    let file_name = format!("{}.tlk", lang);
    if let Some(file) = DefaultTlkFiles::get(&file_name) {
        create_base64_map(&file.data)
    } else {
        let bytes: &[u8] = include_bytes!("../../../resources/data/tlk/default.tlk");
        create_base64_map(bytes)
    }
}

/// Loads the messages that should be displayed to the client and
/// returns them in a list.
fn messages() -> TdfMap<String, String> {
    let intro = Message {
        end_date: None,
        image: None,
        title: Some("Pocket Relay".to_owned()),
        message: format!(
            "You are connected to Pocket Relay <font color='#FFFF66'>(v{})</font>",
            constants::VERSION,
        ),
        priority: 1,
        tracking_id: None,
        ty: MessageType::MenuTerminal,
    };

    let messages = vec![intro];

    let mut config = TdfMap::new();
    let mut index = 1;
    for message in messages {
        message.append(index, &mut config);
        index += 1;
    }

    config.order();
    config
}

/// Structure for a message
struct Message {
    /// The end date of this message
    end_date: Option<String>,
    /// Path to the message image dds
    /// if left blank the game will use
    /// a default imagee
    image: Option<String>,
    /// The title of the mesage
    title: Option<String>,
    /// The message text content
    message: String,
    /// The message priority
    priority: u32,
    /// Unique identifier for this message so that when dismissed it wont
    /// be shown to the same user again
    tracking_id: Option<u32>,
    /// The type of message
    ty: MessageType,
}

/// Known types of messages
#[allow(unused)]
enum MessageType {
    /// Displayed on the main menu in the next tab on the terminal
    MenuTerminal,
    /// Displayed on the main menu in the scrolling text
    MenuScrolling,
    /// Large multiplayer fullscreen notification
    /// with store button
    Multiplayer,
    /// Other unknown value
    Other(u8),
}

impl MessageType {
    fn value(&self) -> u8 {
        match self {
            Self::MenuTerminal => 0x0,
            Self::MenuScrolling => 0x3,
            Self::Multiplayer => 0x8,
            Self::Other(value) => *value,
        }
    }
}

impl Message {
    /// Appends this message to the provided map
    pub fn append(self, index: usize, map: &mut TdfMap<String, String>) {
        let langs = ["de", "es", "fr", "it", "ja", "pl", "ru"];
        let prefix = format!("MSG_{index}_");

        if let Some(end_date) = self.end_date {
            map.insert(format!("{prefix}endDate"), end_date);
        }

        if let Some(image) = self.image {
            map.insert(format!("{prefix}image"), image);
        }

        map.insert(format!("{prefix}message"), &self.message);
        for lang in &langs {
            map.insert(format!("{prefix}message_{lang}"), &self.message);
        }

        map.insert(format!("{prefix}priority"), self.priority.to_string());

        if let Some(title) = &self.title {
            map.insert(format!("{prefix}title"), title);
            for lang in &langs {
                map.insert(format!("{prefix}title_{lang}"), title);
            }
        }

        if let Some(tracking_id) = self.tracking_id {
            map.insert(format!("{prefix}trackingId"), tracking_id.to_string());
        }

        map.insert(format!("{prefix}type"), self.ty.value().to_string());
    }
}

/// Creates a map for the data configuration ME3_DATA client configuration
/// this configuration includes the addresses for the the Galaxy At War
/// server (GAW_SERVER_BASE_URL) and shop image contents (IMG_MNGR_BASE_URL)
/// these urls are set to (gosredirector.ea.com) because the client will
/// redirect this host and handling proxying itself
///
///
/// Last known original server values:
///
/// Galaxy At War: https://wal.tools.gos.ea.com/wal/masseffect-gaw-pc
/// Image Server: http://eaassets-a.akamaihd.net/gameplayservices/prod/MassEffect/3/
/// Telemetry Server: 159.153.235.32:9988
///
fn data_config() -> TdfMap<String, String> {
    let http_port = env::from_env(env::HTTP_PORT);
    let prefix = format!("http://{}:{}", constants::EXTERNAL_HOST, http_port);

    let mut config = TdfMap::with_capacity(15);
    config.insert("GAW_SERVER_BASE_URL", format!("{prefix}/gaw"));
    config.insert("IMG_MNGR_BASE_URL", format!("{prefix}/content/"));
    config.insert("IMG_MNGR_MAX_BYTES", "1048576");
    config.insert("IMG_MNGR_MAX_IMAGES", "5");
    config.insert("JOB_THROTTLE_0", "0");
    config.insert("JOB_THROTTLE_1", "0");
    config.insert("JOB_THROTTLE_2", "0");
    config.insert("MATCH_MAKING_RULES_VERSION", "5");
    config.insert("MULTIPLAYER_PROTOCOL_VERSION", "3");
    config.insert("TEL_DISABLE", "**");
    config.insert("TEL_DOMAIN", "pc/masseffect-3-pc-anon");
    config.insert("TEL_FILTER", "-UION/****");
    config.insert("TEL_PORT", "9988");
    config.insert("TEL_SEND_DELAY", "15000");
    config.insert("TEL_SEND_PCT", "75");
    config.insert("TEL_SERVER", constants::EXTERNAL_HOST);
    config
}

/// Handles suspend user ping packets. The usage of this is unknown and needs
/// further research
///
/// ```
/// Route: Util(SuspendUserPing)
/// ID: 31
/// Content: {
///     "TVAL": 90000000
/// }
/// ```
async fn handle_suspend_user_ping(req: SuspendPingRequest) -> ServerResult<()> {
    match req.value {
        20000000 => Err(ServerError::Suspend12D),
        90000000 => Err(ServerError::Suspend12E),
        _ => Ok(()),
    }
}

/// Handles updating the stored data for this account
///
/// ```
/// Route: Util(UserSettingsSave)
/// ID: 45
/// Content: {
///     "DATA": "20;4;Adept;20;0.0000;50",
///     "KEY": "class1",
///     "UID": 0
/// }
/// ```
async fn handle_user_settings_save(
    session: SessionAddr,
    req: SettingsSaveRequest,
) -> ServerResult<()> {
    let db = GlobalState::database();

    let player = session
        .get_player()
        .await
        .ok_or(ServerError::FailedNoLoginAction)?;

    player
        .set_data(db, req.key, req.value)
        .await
        .map_err(|err| {
            warn!("Failed to update player data: {err:?}");
            ServerError::ServerUnavailable
        })?;
    Ok(())
}

/// Handles loading all the user details for the current account and sending them to the
/// client
///
/// ```
/// Route: Util(UserSettingsLoadAll)
/// ID: 23
/// Content: {}
/// ```
async fn handle_user_settings_load_all(session: SessionAddr) -> BlazeResult<SettingsResponse> {
    let player = session
        .get_player()
        .await
        .ok_or(ServerError::FailedNoLoginAction)?;
    let db = GlobalState::database();
    let data = player.all_data(db).await?;
    let mut settings = TdfMap::<String, String>::with_capacity(data.len());
    for value in data {
        settings.insert(value.key, value.value)
    }
    settings.order();
    Ok(SettingsResponse { settings })
}
