use blaze_pk::{codec::Codec, packet, packet::Packet, tag::ValueType, tagging::*, types::TdfMap};
use core::blaze::components::Util;
use core::blaze::errors::{HandleResult, ServerError};
use core::blaze::session::SessionArc;
use core::blaze::shared::TelemetryRes;
use core::env::{self, VERSION};
use database::{PlayerCharactersInterface, PlayerClassesInterface, PlayersInterface};
use log::{debug, warn};
use rust_embed::RustEmbed;
use tokio::try_join;
use utils::dmap::load_dmap;
use utils::time::server_unix_time;

/// Routing function for handling packets with the `Util` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &SessionArc, component: Util, packet: &Packet) -> HandleResult {
    match component {
        Util::PreAuth => handle_pre_auth(session, packet).await,
        Util::PostAuth => handle_post_auth(session, packet).await,
        Util::Ping => handle_ping(session, packet).await,
        Util::FetchClientConfig => handle_fetch_client_config(session, packet).await,
        Util::SuspendUserPing => handle_suspend_user_ping(session, packet).await,
        Util::UserSettingsSave => handle_user_settings_save(session, packet).await,
        Util::GetTelemetryServer => handle_get_telemetry_server(session, packet).await,
        Util::UserSettingsLoadAll => handle_user_settings_load_all(session, packet).await,
        component => {
            debug!("Got Util({component:?})");
            session.response_empty(packet).await
        }
    }
}

/// Handles retrieving the details about the telemetry server
///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.GET_TELEMETRY_SERVER, 0x0) {}
/// ```
///
async fn handle_get_telemetry_server(session: &SessionArc, packet: &Packet) -> HandleResult {
    let ext_host = env::str_env(env::EXT_HOST);
    let res = TelemetryRes {
        address: ext_host,
        session_id: session.id,
    };
    session.response(packet, &res).await
}

pub struct PreAuthRes {
    host: String,
    port: u16,
    config: TdfMap<String, String>,
}

/// Key identifying the QOSS server used.
pub const QOSS_KEY: &str = "ea-sjc";
/// Server SRC version
pub const SRC_VERSION: &str = "303107";
pub const BLAZE_VERSION: &str = "Blaze 3.15.08.0 (CL# 1629389)";
pub const PING_PERIOD: &str = "15s";
pub const VOIP_HEADSET_UPDATE_RATE: &str = "1000";
/// XLSP (Xbox Live Server Platform)
pub const XLSP_CONNECTION_IDLE_TIMEOUT: &str = "300";

//noinspection SpellCheckingInspection
impl Codec for PreAuthRes {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_zero(output, "ANON");
        tag_str(output, "ASRC", SRC_VERSION);
        tag_list(
            output,
            "CIDS",
            vec![
                0x1, 0x19, 0x4, 0x1c, 0x7, 0x9, 0xf802, 0x7800, 0xf, 0x7801, 0x7802, 0x7803,
                0x7805, 0x7806, 0x7d0,
            ],
        );
        tag_empty_str(output, "CNGN");
        {
            tag_group_start(output, "CONF");
            tag_value(output, "CONF", &self.config);
            tag_group_end(output);
        }
        tag_str(output, "INST", "masseffect-3-pc");
        tag_zero(output, "MINR");
        tag_str(output, "NASP", "cem_ea_id");
        tag_empty_str(output, "PILD");
        tag_str(output, "PLAT", "pc");
        tag_empty_str(output, "PTAG");

        #[inline]
        fn encode_qoss_group(output: &mut Vec<u8>, host: &str, port: u16) {
            tag_str(output, "PSA", host);
            tag_u16(output, "PSP", port);
            tag_str(output, "SNA", "prod-sjc");
            tag_group_end(output);
        }

        {
            tag_group_start(output, "QOSS");
            {
                tag_group_start(output, "BWPS");
                encode_qoss_group(output, &self.host, self.port);
            }
            tag_u8(output, "LNP", 0xA);

            {
                tag_map_start(output, "LTPS", ValueType::String, ValueType::Group, 1);
                QOSS_KEY.encode(output);
                encode_qoss_group(output, &self.host, self.port);
            }

            tag_u32(output, "SVID", 0x45410805);
            tag_group_end(output)
        }

        tag_str(output, "RSRC", SRC_VERSION);
        tag_str(output, "SVER", BLAZE_VERSION)
    }
}

/// Handles responding to pre-auth requests which is the first request
/// that clients will send. The response to this contains information
/// about the server the client is connecting to.
///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.PRE_AUTH, 0x0, 0x0) {
///   group("CDAT") {
///     number("IITO", 0x0)
///     number("LANG", 0x656e4e5a)
///     text("SVCN", "masseffect-3-pc")
///     number("TYPE", 0x0)
///   }
///   group("CINF") {
///     text("BSDK", "3.15.6.0")
///     text("BTIM", "Dec 21 2012 12:46:51")
///     text("CLNT", "MassEffect3-pc")
///     text("CSKU", "134845")
///     text("CVER", "05427.124")
///     text("DSDK", "8.14.7.1")
///     text("ENV", "prod")
///     number("LOC", 0x656e4e5a)
///     text("MAC", "7c:10:c9:28:33:35")
///     text("PLAT", "Windows")
///   }
///   group("FCCR") {
///     text("CFID", "BlazeSDK")
///   }
/// }
/// ```
async fn handle_pre_auth(session: &SessionArc, packet: &Packet) -> HandleResult {
    let mut config = TdfMap::with_capacity(3);
    config.insert("pingPeriod", PING_PERIOD);
    config.insert("voipHeadsetUpdateRate", VOIP_HEADSET_UPDATE_RATE);
    config.insert("xlspConnectionIdleTimeout", XLSP_CONNECTION_IDLE_TIMEOUT);

    let host = env::str_env(env::EXT_HOST);
    let port = env::u16_env(env::HTTP_PORT);

    session
        .response(packet, &PreAuthRes { host, port, config })
        .await
}

struct PSSDetails {
    address: String,
    port: u16,
}

//noinspection SpellCheckingInspection
impl Codec for PSSDetails {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "ADRS", &self.address);
        tag_empty_blob(output, "CSIG");
        tag_str(output, "PJID", SRC_VERSION);
        tag_u16(output, "PORT", self.port);
        tag_u8(output, "RPRT", 0xF);
        tag_u8(output, "TIID", 0);
    }
}

struct TickerDetails {
    host: String,
    port: u16,
    key: &'static str,
}

//noinspection SpellCheckingInspection
impl Codec for TickerDetails {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "ADRS", &self.host);
        tag_u16(output, "PORT", self.port);
        tag_str(output, "SKEY", self.key);
    }
}

struct PostAuthRes {
    pss: PSSDetails,
    ticker: TickerDetails,
    session_id: u32,
}

//noinspection SpellCheckingInspection
impl Codec for PostAuthRes {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "PSS");
        self.pss.encode(output);
        tag_group_end(output);

        tag_group_start(output, "TICK");
        self.ticker.encode(output);
        tag_group_end(output);

        tag_group_start(output, "UROP");
        tag_u8(output, "TMOP", 0x1);
        tag_u32(output, "UID", self.session_id);
        tag_group_end(output);
    }
}

///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.POST_AUTH, 0x1b) {}
/// ```
async fn handle_post_auth(session: &SessionArc, packet: &Packet) -> HandleResult {
    let ext_host = env::str_env(env::EXT_HOST);
    let res = PostAuthRes {
        session_id: session.id,
        ticker: TickerDetails {
            host: ext_host,
            port: 9988,
            key: "823287263,10.23.15.2:8999,masseffect-3-pc,10,50,50,50,50,0,12",
        },
        pss: PSSDetails {
            address: "playersyncservice.ea.com".to_string(),
            port: 443,
        },
    };
    session.response(packet, &res).await
}

packet! {
    struct PingRes {
        STIM server_time: u64
    }
}

/// Handles ping update requests. These are sent by the client at the interval
/// specified in the pre-auth response. The server replies to this messages with
/// the current server unix timestamp in seconds.
///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.PING, 0x0, 0x1) {}
/// ```
///
async fn handle_ping(session: &SessionArc, packet: &Packet) -> HandleResult {
    let server_time = server_unix_time();
    session.response(packet, &PingRes { server_time }).await
}

packet! {
    struct FetchConfigReq {
        CFID id: String
    }
}

packet! {
    struct FetchConfigRes {
        CONF config: TdfMap<String, String>
    }
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
/// packet(Components.UTIL, Commands.FETCH_CLIENT_CONFIG, 0x0, 0x2) {
///   text("CFID", "ME3_DATA")
/// }
/// ```
async fn handle_fetch_client_config(session: &SessionArc, packet: &Packet) -> HandleResult {
    let fetch_config = packet.decode::<FetchConfigReq>()?;
    let config = match fetch_config.id.as_ref() {
        "ME3_DATA" => data_config(),
        "ME3_MSG" => messages().await,
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
    session.response(packet, &FetchConfigRes { config }).await
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
async fn messages() -> TdfMap<String, String> {
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
fn data_config() -> TdfMap<String, String> {
    let ext_host = env::str_env(env::EXT_HOST);
    let http_port = env::u16_env(env::HTTP_PORT);

    let prefix = format!("http://{ext_host}:{http_port}");

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
    config.insert("TEL_SERVER", ext_host);
    config
}

packet! {
    struct SuspendUserPing {
        TVAL value: u32,
    }
}

/// Handles suspend user ping packets. The usage of this is unknown and needs
/// further research
///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.SUSPEND_USER_PING, 0x1f) {
///   number("TVAL", 0x55d4a80)
/// }
/// ```
///
///
async fn handle_suspend_user_ping(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<SuspendUserPing>()?;
    match req.value {
        0x1312D00 => session.response_error_empty(packet, 0x12Du16).await,
        0x55D4A80 => session.response_error_empty(packet, 0x12Eu16).await,
        _ => session.response_empty(packet).await,
    }
}

packet! {
    struct UserSettingsSave {
        DATA value: String,
        KEY key: String,
    }

}

/// Handles updating the stored data for this account
///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.USER_SETTINGS_SAVE, 0x0, 0x2d) {
///   text("DATA", "20;4;Adept;20;0.0000;50")
///   text("KEY", "class1")
///   number("UID", 0x0)
/// }
/// ```
async fn handle_user_settings_save(session: &SessionArc, packet: &Packet) -> HandleResult {
    let req = packet.decode::<UserSettingsSave>()?;
    let key = &req.key;
    let value = req.value;

    let db = session.db();
    if key.starts_with("class") {
        debug!("Updating player class data: {key}");
        let session_data = &*session.data.read().await;
        let player = match session_data.player.as_ref() {
            Some(value) => value,
            None => {
                return session
                    .response_error_empty(packet, ServerError::FailedNoLoginAction)
                    .await;
            }
        };
        match PlayerClassesInterface::update(db, player, key, &value).await {
            Ok(_) => {}
            Err(err) => {
                warn!("Failed to update player class: {err:?}");
                return session
                    .response_error_empty(packet, ServerError::ServerUnavailable)
                    .await;
            }
        }
        debug!("Updating player character data: {key}");
    } else if key.starts_with("char") {
        debug!("Updating player character data: {key}");
        let session_data = &*session.data.read().await;
        let player = match session_data.player.as_ref() {
            Some(value) => value,
            None => {
                return session
                    .response_error_empty(packet, ServerError::FailedNoLoginAction)
                    .await;
            }
        };
        match PlayerCharactersInterface::update(db, player, key, &value).await {
            Ok(_) => {}
            Err(err) => {
                warn!("Failed to update player character: {err:?}");
                return session
                    .response_error_empty(packet, ServerError::ServerUnavailable)
                    .await;
            }
        }
        debug!("Updated player character data: {key}");
    } else {
        debug!("Updating player base data");
        let session_data = &mut *session.data.write().await;
        let player = match session_data.player.take() {
            Some(value) => value,
            None => {
                return session
                    .response_error_empty(packet, ServerError::FailedNoLoginAction)
                    .await;
            }
        };
        match PlayersInterface::update(db, player, key, value).await {
            Ok(player) => {
                session_data.player = Some(player);
                debug!("Updated player base data");
            }
            Err(err) => {
                warn!("Failed to update player data: {err:?}");
                return session
                    .response_error_empty(packet, ServerError::ServerUnavailable)
                    .await;
            }
        };
    }
    session.response_empty(packet).await
}

packet! {
    struct UserSettingsAll {
        SMAP settings: TdfMap<String, String>
    }
}

/// Handles loading all the user details for the current account and sending them to the
/// client
///
/// # Structure
/// ```
/// packet(Components.UTIL, Commands.USER_SETTINGS_LOAD_ALL, 0x17) {}
/// ```
async fn handle_user_settings_load_all(session: &SessionArc, packet: &Packet) -> HandleResult {
    let mut settings = TdfMap::<String, String>::new();
    {
        let session_data = session.data.read().await;

        let Some(player) = session_data.player.as_ref() else {
            warn!("Client attempted to load settings without being authenticated. (SID: {})", session.id);
            return session.response_error_empty(packet, ServerError::FailedNoLoginAction).await;
        };

        settings.insert("Base", PlayersInterface::encode_base(player));

        let db = session.db();

        let classes = PlayerClassesInterface::find_all(db, player);
        let characters = PlayerCharactersInterface::find_all(db, player);

        let (classes, characters) = try_join!(classes, characters)?;

        let mut index = 0;
        for char in characters {
            settings.insert(
                format!("char{}", index),
                PlayerCharactersInterface::encode(&char),
            );
            index += 1;
        }

        index = 0;
        for class in classes {
            settings.insert(
                format!("class{}", index),
                PlayerClassesInterface::encode(&class),
            );
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
    session
        .response(packet, &UserSettingsAll { settings })
        .await
}
