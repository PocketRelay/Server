use crate::{
    config::{Config, VERSION},
    database::entities::PlayerData,
    services::config::{
        fallback_coalesced_file, fallback_talk_file, local_coalesced_file, local_talk_file,
    },
    session::{
        models::{
            errors::{BlazeError, GlobalError, ServerResult},
            util::*,
            IpPairAddress, NetworkAddress,
        },
        router::{Blaze, Extension, SessionAuth},
        SessionLink,
    },
    utils::encoding::{create_base64_map, generate_coalesced, ChunkMap},
};
use log::{debug, error};
use me3_coalesced_parser::{serialize_coalesced, Coalesced};
use sea_orm::DatabaseConnection;
use std::{
    borrow::Cow,
    cmp::Ordering,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tdf::TdfMap;

/// Handles retrieving the details about the telemetry server
///
/// ```
/// Route: Util(GetTelemetryServer)
/// ID: 0
/// Content: {}
/// ```
///
pub async fn handle_get_telemetry_server() -> Blaze<TelemetryServer> {
    Blaze(TelemetryServer)
}

/// Handles retrieving the details about the ticker server
///
/// ```
/// Route: Util(GetTickerServer)
/// ID: 0
/// Content: {}
/// ```
///
pub async fn handle_get_ticker_server() -> Blaze<TickerServer> {
    Blaze(TickerServer)
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
pub async fn handle_pre_auth(
    Extension(config): Extension<Arc<Config>>,
) -> ServerResult<Blaze<PreAuthResponse>> {
    Ok(Blaze(PreAuthResponse { config }))
}

/// Handles post authentication requests. This provides information about other
/// servers that are used by Mass Effect such as the Telemetry and Ticker servers.
///
/// ```
/// Route: Util(PostAuth)
/// ID: 27
/// Content: {}
/// ```
pub async fn handle_post_auth(
    session: SessionLink,
    SessionAuth(player): SessionAuth,
) -> ServerResult<Blaze<PostAuthResponse>> {
    // Subscribe to the session with itself
    session
        .data
        .add_subscriber(player.id, Arc::downgrade(&session));

    Ok(Blaze(PostAuthResponse {
        telemetry: TelemetryServer,
        ticker: TickerServer,
        player_id: player.id,
    }))
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
pub async fn handle_ping(session: SessionLink) -> Blaze<PingResponse> {
    session.data.set_alive();

    let server_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    Blaze(PingResponse { server_time })
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
    Blaze(FetchConfigRequest { id }): Blaze<FetchConfigRequest>,
) -> ServerResult<Blaze<FetchConfigResponse>> {
    let config = match id.as_str() {
        "ME3_DATA" => data_config(),
        "ME3_MSG" => messages(),
        "ME3_ENT" => load_entitlements(),
        "ME3_DIME" => {
            let mut map = TdfMap::with_capacity(1);
            map.insert("Config".to_string(), ME3_DIME.to_string());
            map
        }
        "ME3_BINI_VERSION" => {
            let mut map = TdfMap::with_capacity(2);
            map.insert("SECTION".to_string(), "BINI_PC_COMPRESSED".to_string());
            map.insert("VERSION".to_string(), "40128".to_string());
            map
        }
        "ME3_BINI_PC_COMPRESSED" => match create_coalesced_map().await {
            Ok(map) => map,
            Err(err) => {
                error!("Failed to load server coalesced: {}", err);
                return Err(GlobalError::System.into());
            }
        },
        id => {
            if let Some(lang) = id.strip_prefix("ME3_LIVE_TLK_PC_") {
                talk_file(lang).await
            } else {
                TdfMap::default()
            }
        }
    };

    Ok(Blaze(FetchConfigResponse { config }))
}

/// Loads the entitlements from the entitlements file and parses
/// it as a
fn load_entitlements() -> TdfMap<String, String> {
    let vec = ME3_ENT
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    TdfMap::from_presorted_elements(vec)
}

async fn load_coalesced() -> Coalesced {
    match local_coalesced_file().await {
        Ok(result) => result,
        Err(err) => {
            // Log errors if the file existed
            if !matches!(err.kind(), std::io::ErrorKind::NotFound) {
                error!(
                    "Unable to load local coalesced file falling back to default: {}",
                    err
                );
            }

            // Fallback to default
            fallback_coalesced_file()
        }
    }
}

/// Loads the local coalesced if one is present falling back
/// to the default one on error or if its missing
async fn create_coalesced_map() -> std::io::Result<ChunkMap> {
    // Load the coalesced from JSON
    let coalesced = load_coalesced().await;

    // Serialize the coalesced to bytes
    let serialized = serialize_coalesced(&coalesced);

    // Encode and compress the coalesced
    generate_coalesced(&serialized)
}

/// Retrieves a talk file for the specified language code falling back
/// to the `ME3_TLK_DEFAULT` default talk file if it could not be found
///
/// `lang` The talk file language
async fn talk_file(lang: &str) -> ChunkMap {
    let bytes: Cow<'static, [u8]> = match local_talk_file(lang).await {
        Ok(result) => Cow::Owned(result),
        Err(err) => {
            // Log errors if the file existed
            if !matches!(err.kind(), std::io::ErrorKind::NotFound) {
                error!(
                    "Unable to load local talk file falling back to default: {}",
                    err
                );
            }

            // Fallback to default
            Cow::Borrowed(fallback_talk_file(lang))
        }
    };

    create_base64_map(&bytes)
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
            VERSION,
        ),
        priority: 1,
        tracking_id: Some(1),
        ty: MessageType::MenuTerminal,
    };

    let mut config = TdfMap::new();

    intro.append(1, &mut config);

    config
}

/// Structure for a message
struct Message {
    /// The end date of this message
    end_date: Option<String>,
    /// Path to the message image dds
    /// if left blank the game will use
    /// a default image
    image: Option<String>,
    /// The title of the message
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
    /// Large multiplayer full-screen notification
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

        map.insert(format!("{prefix}message"), self.message.to_string());
        for lang in &langs {
            map.insert(format!("{prefix}message_{lang}"), self.message.to_string());
        }

        map.insert(format!("{prefix}priority"), self.priority.to_string());

        if let Some(title) = &self.title {
            map.insert(format!("{prefix}title"), title.to_string());
            for lang in &langs {
                map.insert(format!("{prefix}title_{lang}"), title.to_string());
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
/// redirect this host and handle the request proxy itself
///
///
/// Last known original server values:
///
/// Galaxy At War: https://wal.tools.gos.ea.com/wal/masseffect-gaw-pc
/// Image Server: http://eaassets-a.akamaihd.net/gameplayservices/prod/MassEffect/3/
/// Telemetry Server: 159.153.235.32:9988
///
fn data_config() -> TdfMap<String, String> {
    let prefix = format!("http://127.0.0.1:{}", LOCAL_HTTP_PORT);

    let tele_port = TELEMETRY_PORT;

    let mut config = TdfMap::with_capacity(15);
    config.insert("GAW_SERVER_BASE_URL".to_string(), format!("{prefix}/"));
    config.insert(
        "IMG_MNGR_BASE_URL".to_string(),
        format!("{prefix}/content/"),
    );
    config.insert("IMG_MNGR_MAX_BYTES".to_string(), "1048576".to_string());
    config.insert("IMG_MNGR_MAX_IMAGES".to_string(), "5".to_string());
    config.insert("JOB_THROTTLE_0".to_string(), "10000".to_string());
    config.insert("JOB_THROTTLE_1".to_string(), "5000".to_string());
    config.insert("JOB_THROTTLE_2".to_string(), "1000".to_string());
    config.insert("MATCH_MAKING_RULES_VERSION".to_string(), "5".to_string());
    config.insert("MULTIPLAYER_PROTOCOL_VERSION".to_string(), "3".to_string());
    config.insert("TEL_DISABLE".to_string(), TELEMETRY_DISA.to_string());
    config.insert(
        "TEL_DOMAIN".to_string(),
        "pc/masseffect-3-pc-anon".to_string(),
    );
    config.insert("TEL_FILTER".to_string(), "-UION/****".to_string());
    config.insert("TEL_PORT".to_string(), tele_port.to_string());
    config.insert("TEL_SEND_DELAY".to_string(), "15000".to_string());
    config.insert("TEL_SEND_PCT".to_string(), "75".to_string());
    config.insert("TEL_SERVER".to_string(), "127.0.0.1".to_string());
    config
}

/// Handles suspend user ping packets. The usage of this is unknown and needs
/// further research
///
/// Handles suspending user ping timeout for a specific period of time. The client
/// provides a time in microseconds and the server responds with whether it will
/// allow the time
///
/// [UtilError::]
///
///
/// ```
/// Route: Util(SuspendUserPing)
/// ID: 31
/// Content: {
///     "TVAL": 90000000
/// }
/// ```
pub async fn handle_suspend_user_ping(
    session: SessionLink,
    Blaze(SuspendPingRequest { time_value }): Blaze<SuspendPingRequest>,
) -> BlazeError {
    let res = match time_value.cmp(&90000000) {
        Ordering::Less => UtilError::SuspendPingTimeTooSmall,
        Ordering::Greater => UtilError::SuspendPingTimeTooLarge,
        Ordering::Equal => {
            session
                .data
                .set_keep_alive_grace(Duration::from_micros(time_value as u64));

            UtilError::PingSuspended
        }
    };
    res.into()
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
    SessionAuth(player): SessionAuth,
    Extension(db): Extension<DatabaseConnection>,
    Blaze(SettingsSaveRequest { value, key }): Blaze<SettingsSaveRequest>,
) -> ServerResult<()> {
    PlayerData::set(&db, player.id, key, value).await?;
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
pub async fn handle_load_settings(
    SessionAuth(player): SessionAuth,
    Extension(db): Extension<DatabaseConnection>,
) -> ServerResult<Blaze<SettingsResponse>> {
    // Load the player data from the database
    let settings: TdfMap<String, String> = PlayerData::all(&db, player.id)
        .await?
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect();

    Ok(Blaze(SettingsResponse { settings }))
}

/// Handles client updating networking through Upnp changes
///
/// ```
/// Request (27): Util->SetClientMetrics (0x0009->0x0016)
/// Content: {
///     "UBFL": 2,
///     "UDEV": "DEVICE NAME",
///     "UFLG": 31,
///     "ULRC": 0,
///     "UNAT": 4,
///     "USTA": 2,
///     "UWAN": 0 /* WAN IP ADDRESS FROM UPNP */,
/// }
/// ```
pub async fn handle_set_client_metrics(
    session: SessionLink,
    Blaze(SetClientMetricsRequest {
        blaze_flags,
        device_info,
        flags,
        nat_type,
        status,
        wan,
        ..
    }): Blaze<SetClientMetricsRequest>,
) {
    debug!(
        "Handling UPNP (Device: {}, BlazeFlags: {:?} Flags: {:?}, NAT: {:?}, WAN: {}, STATUS: {:?})",
        device_info, blaze_flags, flags, nat_type, wan, status
    );

    // Don't do anything if Upnp failed
    if !matches!(status, UpnpStatus::Enabled) {
        return;
    }

    // Set external address using Upnp specified
    if !wan.is_unspecified() && !blaze_flags.contains(UpnpFlags::DOUBLE_NAT) {
        debug!("Using client Upnp WAN address override: {}", wan);

        let network_info = session.data.network_info().unwrap_or_default();
        let ping_site_latency = network_info.ping_site_latency.clone();
        let qos = network_info.qos;
        let mut pair_addr = match &network_info.addr {
            NetworkAddress::AddressPair(pair) => pair.clone(),
            // Fallback handle behavior for unset or default address
            _ => IpPairAddress::default(),
        };

        // Update WAN address with Upnp address
        pair_addr.external.addr = wan;

        // Update network info with new details
        session.data.set_network_info(
            NetworkAddress::AddressPair(pair_addr),
            qos,
            ping_site_latency,
        );
    }
}
