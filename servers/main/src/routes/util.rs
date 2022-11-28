use crate::models::util::{
    FetchConfigRequest, FetchConfigResponse, PingResponse, PostAuthResponse, PreAuthResponse,
    SettingsResponse, SettingsSaveRequest, SuspendPingRequest, TelemetryServer, TickerServer,
};
use crate::session::Session;
use crate::HandleResult;
use blaze_pk::{packet::Packet, types::TdfMap};
use core::blaze::components::Util;
use core::blaze::errors::ServerError;
use core::constants::{self, VERSION};
use core::env;
use core::state::GlobalState;
use database::{PlayerCharacter, PlayerClass};
use log::{debug, warn};
use rust_embed::RustEmbed;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::try_join;
use utils::dmap::load_dmap;

/// Routing function for handling packets with the `Util` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of the packet recieved
/// `packet`    The recieved packet
pub async fn route(session: &mut Session, component: Util, packet: &Packet) -> HandleResult {
    match component {
        Util::PreAuth => handle_pre_auth(packet),
        Util::PostAuth => handle_post_auth(session, packet),
        Util::Ping => handle_ping(packet),
        Util::FetchClientConfig => handle_fetch_client_config(packet),
        Util::SuspendUserPing => handle_suspend_user_ping(packet),
        Util::UserSettingsSave => handle_user_settings_save(session, packet).await,
        Util::GetTelemetryServer => handle_get_telemetry_server(packet),
        Util::GetTickerServer => handle_get_ticker_server(packet),
        Util::UserSettingsLoadAll => handle_user_settings_load_all(session, packet).await,
        _ => Ok(packet.respond_empty()),
    }
}

/// Handles retrieving the details about the telemetry server
///
/// ```
/// Route: Util(GetTelemetryServer)
/// ID: 0
/// Content: {}
/// ```
///
fn handle_get_telemetry_server(packet: &Packet) -> HandleResult {
    let response = TelemetryServer { port: 9988 };
    Ok(packet.respond(response))
}

/// Handles retrieving the details about the ticker server
///
/// ```
/// Route: Util(GetTickerServer)
/// ID: 0
/// Content: {}
/// ```
///
fn handle_get_ticker_server(packet: &Packet) -> HandleResult {
    let response = TickerServer { port: 8999 };
    Ok(packet.respond(response))
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
fn handle_pre_auth(packet: &Packet) -> HandleResult {
    let qos_port = env::from_env(env::HTTP_PORT);
    let response = PreAuthResponse { qos_port };
    Ok(packet.respond(response))
}

/// Handles post authentication requests. This provides information about other
/// servers that are used by Mass Effect such as the Telemetry and Ticker servers.
///
/// ```
/// Route: Util(PostAuth)
/// ID: 27
/// Content: {}
/// ```
fn handle_post_auth(session: &mut Session, packet: &Packet) -> HandleResult {
    let player_id = session
        .player
        .as_ref()
        .map(|value| value.id)
        .ok_or(ServerError::FailedNoLoginAction)?;
    session.update_self();
    let response = PostAuthResponse {
        telemetry: TelemetryServer { port: 9988 },
        ticker: TickerServer { port: 8999 },
        player_id,
    };
    Ok(packet.respond(response))
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
fn handle_ping(packet: &Packet) -> HandleResult {
    let now = SystemTime::now();
    let server_time = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    let response = PingResponse { server_time };
    Ok(packet.respond(response))
}

/// Contents of the compressed coalesced dmap file
const ME3_COALESCED: &str = include_str!("../resources/data/coalesced.dmap");
/// Contents of the entitlements dmap file
const ME3_ENT: &str = include_str!("../resources/data/entitlements.dmap");
/// Contents of the dime.xml file
const ME3_DIME: &str = include_str!("../resources/data/dime.xml");

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
fn handle_fetch_client_config(packet: &Packet) -> HandleResult {
    let fetch_config: FetchConfigRequest = packet.decode()?;
    let config = match fetch_config.id.as_ref() {
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
        "ME3_BINI_PC_COMPRESSED" => load_dmap(ME3_COALESCED),
        id => {
            if id.starts_with("ME3_LIVE_TLK_PC_") {
                let lang = &id[16..];
                talk_file(lang)
            } else {
                TdfMap::empty()
            }
        }
    };

    let response = FetchConfigResponse { config };
    Ok(packet.respond(response))
}

/// Contents of the default talk dmap file
const ME3_TLK_DEFAULT: &str = include_str!("../resources/data/tlk/default.tlk.dmap");

/// Talk files imported from the resources folder
#[derive(RustEmbed)]
#[folder = "src/resources/data/tlk"]
struct TLKFiles;

/// Retrieves a talk file for the specified language code falling back
/// to the `ME3_TLK_DEFAULT` default talk file if it could not be found
fn talk_file(lang: &str) -> TdfMap<String, String> {
    let file_name = format!("{lang}.dmap");
    if let Some(file) = TLKFiles::get(&file_name) {
        let contents = String::from_utf8_lossy(file.data.as_ref());
        load_dmap(contents.as_ref())
    } else {
        load_dmap(ME3_TLK_DEFAULT)
    }
}

/// Loads the messages that should be displayed to the client and
/// returns them in a list.
fn messages() -> TdfMap<String, String> {
    let mut config = TdfMap::new();

    let intro = Message {
        end_date: None,
        image: None,
        title: Some("Pocket Relay".to_owned()),
        message: format!(
            "You are connected to Pocket Relay <font color='#FFFF66'>(v{})</font>",
            VERSION,
        ),
        priority: 1,
        tracking_id: None,
        ty: MessageType::MenuTerminal,
    };

    let messages = vec![intro];

    let mut index = 1;
    for message in messages {
        message.append(index, &mut config);
        index += 1;
    }

    config.order();
    config
}

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
fn handle_suspend_user_ping(packet: &Packet) -> HandleResult {
    let req: SuspendPingRequest = packet.decode()?;
    match req.value {
        20000000 => Err(ServerError::Suspend12D.into()),
        90000000 => Err(ServerError::Suspend12E.into()),
        _ => Ok(packet.respond_empty()),
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
async fn handle_user_settings_save(session: &mut Session, packet: &Packet) -> HandleResult {
    let req: SettingsSaveRequest = packet.decode()?;
    let key = &req.key;
    let value = req.value;

    if key.starts_with("class") {
        debug!("Updating player class data: {key}");
        let db = GlobalState::database();
        let player = session
            .player
            .as_ref()
            .ok_or(ServerError::FailedNoLoginAction)?;

        PlayerClass::update(db, player, key, &value)
            .await
            .map_err(|err| {
                warn!("Failed to update player class: {err:?}");
                ServerError::ServerUnavailable
            })?;

        debug!("Updating player character data: {key}");
    } else if key.starts_with("char") {
        debug!("Updating player character data: {key}");
        let db = GlobalState::database();
        let player = session
            .player
            .as_ref()
            .ok_or(ServerError::FailedNoLoginAction)?;

        PlayerCharacter::update(db, player, key, &value)
            .await
            .map_err(|err| {
                warn!("Failed to update player character: {err:?}");
                ServerError::ServerUnavailable
            })?;

        debug!("Updated player character data: {key}");
    } else {
        debug!("Updating player base data");
        let player = session
            .player
            .take()
            .ok_or(ServerError::FailedNoLoginAction)?;
        let db = GlobalState::database();
        let player = player.update(db, key, value).await.map_err(|err| {
            warn!("Failed to update player data: {err:?}");
            ServerError::ServerUnavailable
        })?;
        session.player = Some(player);
        debug!("Updated player base data");
    }
    Ok(packet.respond_empty())
}

/// Handles loading all the user details for the current account and sending them to the
/// client
///
/// ```
/// Route: Util(UserSettingsLoadAll)
/// ID: 23
/// Content: {}
/// ```
async fn handle_user_settings_load_all(session: &mut Session, packet: &Packet) -> HandleResult {
    let mut settings = TdfMap::<String, String>::new();
    {
        let player = session
            .player
            .as_ref()
            .ok_or(ServerError::FailedNoLoginAction)?;

        settings.insert("Base", player.encode_base());

        let db = GlobalState::database();

        let classes = PlayerClass::find_all(db, player);
        let characters = PlayerCharacter::find_all(db, player);

        let (classes, characters) = try_join!(classes, characters)?;

        let mut index = 0;
        for char in characters {
            settings.insert(format!("char{}", index), char.encode());
            index += 1;
        }

        index = 0;
        for class in classes {
            settings.insert(format!("class{}", index), class.encode());
            index += 1;
        }

        #[inline]
        fn insert_optional(map: &mut TdfMap<String, String>, key: &str, value: &Option<String>) {
            if let Some(value) = value {
                map.insert(key, value);
            }
        }
        insert_optional(&mut settings, "Completion", &player.completion);
        insert_optional(&mut settings, "cscompletion", &player.cs_completion);
        settings.insert("csreward", player.csreward.to_string());
        insert_optional(&mut settings, "cstimestamps", &player.cs_timestamps1);
        insert_optional(&mut settings, "cstimestamps2", &player.cs_timestamps2);
        insert_optional(&mut settings, "cstimestamps3", &player.cs_timestamps3);
        insert_optional(&mut settings, "FaceCodes", &player.face_codes);
        insert_optional(&mut settings, "NewItem", &player.new_item);
        insert_optional(&mut settings, "Progress", &player.progress);
    }
    let response = SettingsResponse { settings };
    Ok(packet.respond(response))
}
