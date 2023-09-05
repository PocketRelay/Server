use crate::{
    database::entities::PlayerData,
    session::{
        models::{
            errors::{ServerError, ServerResult},
            util::*,
        },
        DetailsMessage, GetHostTarget, GetPlayerIdMessage, SessionLink,
    },
    state::{self, App},
};
use base64ct::{Base64, Encoding};
use embeddy::Embedded;
use flate2::{write::ZlibEncoder, Compression};
use interlink::prelude::Link;
use log::{error, warn};
use std::{
    io::Write,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tdf::TdfMap;
use tokio::fs::read;

/// Handles retrieving the details about the telemetry server
///
/// ```
/// Route: Util(GetTelemetryServer)
/// ID: 0
/// Content: {}
/// ```
///
pub async fn handle_get_telemetry_server() -> TelemetryServer {
    TelemetryServer
}

/// Handles retrieving the details about the ticker server
///
/// ```
/// Route: Util(GetTickerServer)
/// ID: 0
/// Content: {}
/// ```
///
pub async fn handle_get_ticker_server() -> TickerServer {
    TickerServer
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
pub async fn handle_pre_auth(session: &mut SessionLink) -> ServerResult<PreAuthResponse> {
    let host_target = match session.send(GetHostTarget {}).await {
        Ok(value) => value,
        Err(_) => return Err(ServerError::InvalidInformation),
    };

    Ok(PreAuthResponse { host_target })
}

/// Handles post authentication requests. This provides information about other
/// servers that are used by Mass Effect such as the Telemetry and Ticker servers.
///
/// ```
/// Route: Util(PostAuth)
/// ID: 27
/// Content: {}
/// ```
pub async fn handle_post_auth(session: &mut SessionLink) -> ServerResult<PostAuthResponse> {
    let player_id = session
        .send(GetPlayerIdMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;

    // Queue the session details to be sent to this client
    let _ = session.do_send(DetailsMessage {
        link: Link::clone(&*session),
    });

    Ok(PostAuthResponse {
        telemetry: TelemetryServer,
        ticker: TickerServer,
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
pub async fn handle_ping() -> PingResponse {
    let now = SystemTime::now();
    let server_time = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    PingResponse { server_time }
}

/// Contents of the entitlements dmap file
const ME3_ENT: &str = include_str!("../../resources/data/entitlements.dmap");
/// Contents of the dime.xml file
const ME3_DIME: &str = include_str!("../../resources/data/dime.xml");

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
pub async fn handle_fetch_client_config(
    session: &mut SessionLink,
    req: FetchConfigRequest,
) -> ServerResult<FetchConfigResponse> {
    let config = match req.id.as_ref() {
        "ME3_DATA" => data_config(session).await,
        "ME3_MSG" => messages(),
        "ME3_ENT" => load_entitlements(),
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

/// Loads the entitlements from the entitlements file and parses
/// it as a
fn load_entitlements() -> TdfMap<String, String> {
    let mut map = TdfMap::<String, String>::new();
    for (key, value) in ME3_ENT.lines().filter_map(|line| line.split_once('=')) {
        map.insert(key.to_string(), value.to_string())
    }
    map
}

/// Loads the local coalesced if one is present falling back
/// to the default one on error or if its missing
async fn load_coalesced() -> ServerResult<ChunkMap> {
    let local_path = Path::new("data/coalesced.bin");
    if local_path.is_file() {
        if let Ok(bytes) = read(local_path).await {
            if let Ok(map) = generate_coalesced(&bytes) {
                return Ok(map);
            }
        }

        error!(
            "Unable to compress local coalesced from data/coalesced.bin falling back to default."
        );
    }

    // Fallback to embedded default coalesced.bin
    let bytes: &[u8] = include_bytes!("../../resources/data/coalesced.bin");
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

    let encoded: String = Base64::encode_string(bytes);
    let length = encoded.len();
    let mut output: ChunkMap = TdfMap::with_capacity((length / CHUNK_LENGTH) + 2);

    let mut index = 0;
    let mut offset = 0;

    while offset < length {
        let o1 = offset;
        offset += CHUNK_LENGTH;

        let slice = if offset < length {
            &encoded[o1..offset]
        } else {
            &encoded[o1..]
        };

        output.insert(format!("CHUNK_{}", index), slice.to_string());
        index += 1;
    }

    output.insert("CHUNK_SIZE".to_string(), CHUNK_LENGTH.to_string());
    output.insert("DATA_SIZE".to_string(), length.to_string());
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
        if let Ok(bytes) = read(local_path).await {
            return Ok(create_base64_map(&bytes));
        }
        error!("Unable to load local talk file falling back to default.");
    }

    // Load default talk file
    let file_name = format!("{}.tlk", lang);
    Ok(if let Some(file) = DefaultTlkFiles::get(&file_name) {
        create_base64_map(file)
    } else {
        let bytes: &[u8] = include_bytes!("../../resources/data/tlk/default.tlk");
        create_base64_map(bytes)
    })
}

/// Default talk file values
#[derive(Embedded)]
#[folder = "src/resources/data/tlk"]
struct DefaultTlkFiles;

/// Loads the messages that should be displayed to the client and
/// returns them in a list.
fn messages() -> TdfMap<String, String> {
    let intro = Message {
        end_date: None,
        image: None,
        title: Some("Pocket Relay".to_owned()),
        message: format!(
            "You are connected to Pocket Relay <font color='#FFFF66'>(v{})</font>",
            state::VERSION,
        ),
        priority: 1,
        tracking_id: Some(1),
        ty: MessageType::MenuTerminal,
    };

    let mut config = TdfMap::new();

    intro.append(1, &mut config);

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
async fn data_config(session: &SessionLink) -> TdfMap<String, String> {
    let host_target = match session.send(GetHostTarget).await {
        Ok(value) => value,
        Err(_) => return TdfMap::with_capacity(0),
    };

    let prefix = if host_target.local_http {
        format!("http://127.0.0.1:{}", LOCAL_HTTP_PORT)
    } else {
        format!(
            "{}{}:{}",
            host_target.scheme.value(),
            host_target.host,
            host_target.port
        )
    };

    let tele_port = TELEMETRY_PORT;

    let mut config = TdfMap::with_capacity(15);
    config.insert("GAW_SERVER_BASE_URL", format!("{prefix}/"));
    config.insert("IMG_MNGR_BASE_URL", format!("{prefix}/content/"));
    config.insert("IMG_MNGR_MAX_BYTES", "1048576".to_string());
    config.insert("IMG_MNGR_MAX_IMAGES", "5".to_string());
    config.insert("JOB_THROTTLE_0", "10000".to_string());
    config.insert("JOB_THROTTLE_1", "5000".to_string());
    config.insert("JOB_THROTTLE_2", "1000".to_string());
    config.insert("MATCH_MAKING_RULES_VERSION", "5".to_string());
    config.insert("MULTIPLAYER_PROTOCOL_VERSION", "3".to_string());
    config.insert("TEL_DISABLE", TELEMTRY_DISA.to_string());
    config.insert("TEL_DOMAIN", "pc/masseffect-3-pc-anon".to_string());
    config.insert("TEL_FILTER", "-UION/****".to_string());
    config.insert("TEL_PORT", tele_port.to_string());
    config.insert("TEL_SEND_DELAY", "15000".to_string());
    config.insert("TEL_SEND_PCT", "75".to_string());
    config.insert("TEL_SERVER", "127.0.0.1".to_string());
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
pub async fn handle_suspend_user_ping(req: SuspendPingRequest) -> ServerResult<()> {
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
pub async fn handle_user_settings_save(
    session: &mut SessionLink,
    req: SettingsSaveRequest,
) -> ServerResult<()> {
    let player = session
        .send(GetPlayerIdMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;

    let db = App::database();
    if let Err(err) = PlayerData::set(db, player, req.key, req.value).await {
        warn!("Failed to update player data: {err:?}");
        Err(ServerError::ServerUnavailable)
    } else {
        Ok(())
    }
}

/// Handles loading all the user details for the current account and sending them to the
/// client
///
/// ```
/// Route: Util(UserSettingsLoadAll)
/// ID: 23
/// Content: {}
/// ```
pub async fn handle_load_settings(session: &mut SessionLink) -> ServerResult<SettingsResponse> {
    let player = session
        .send(GetPlayerIdMessage)
        .await
        .map_err(|_| ServerError::ServerUnavailable)?
        .ok_or(ServerError::FailedNoLoginAction)?;

    let db = App::database();

    // Load the player data from the database
    let data: Vec<PlayerData> = match PlayerData::all(db, player).await {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to load player data: {err:?}");
            return Err(ServerError::ServerUnavailable);
        }
    };

    // Encode the player data into a settings map and order it
    let mut settings = TdfMap::<String, String>::with_capacity(data.len());
    for value in data {
        settings.insert(value.key, value.value)
    }
    settings.order();
    Ok(SettingsResponse { settings })
}
