use blaze_pk::{Codec, group, OpaquePacket, packet, tag_empty_blob, tag_group_end, tag_group_start, tag_str, tag_u16, tag_u32, tag_u8, TdfMap};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{debug, warn};
use rust_embed::RustEmbed;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, NotSet, QueryFilter};
use tokio::try_join;
use crate::blaze::components::Util;
use crate::blaze::errors::{BlazeError, BlazeResult, HandleResult};
use crate::blaze::Session;
use crate::blaze::shared::TelemetryRes;
use crate::database::entities::{player_characters, player_classes, PlayerActiveModel, PlayerCharacterActiveModel, PlayerCharacterEntity, PlayerCharacterModel, PlayerClassActiveModel, PlayerClassEntity, PlayerClassModel, PlayerModel, players};
use crate::env;
use crate::env::ADDRESS;
use crate::utils::conv::MEStringParser;
use crate::utils::dmap::load_dmap;

/// Routing function for handling packets with the `Util` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: Util, packet: &OpaquePacket) -> HandleResult {
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
            packet.debug_decode()?;
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
async fn handle_get_telemetry_server(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let ext_host = env::ext_host();
    let res = TelemetryRes { address: ext_host, session_id: session.id };
    session.response(packet, &res).await
}

packet! {
    struct PreAuthReq {
        CINF client_info: ClientInfo,
    }
}

group! {
    struct ClientInfo {
        LOC location: u32,
    }
}

packet! {
    struct PreAuthRes {
        ANON anon: u8,
        ASRC asrc: &'static str,
        CIDS component_ids: Vec<u16>,
        CNGN cngn: &'static str,
        CONF config: PreAuthConfig,
        INST inst: &'static str,
        MINR minr: u8,
        NASP nasp: &'static str,
        PILD pild: &'static str,
        PLAT platform: &'static str,
        PTAG ptag: &'static str,
        QOSS qoss: QOSS,
        RSRC rsrc: &'static str,
        SVER version: &'static str
    }
}

group! {
    struct PreAuthConfig {
        CONF config: TdfMap<&'static str, &'static str>
    }
}

group! {
    struct QOSS {
        BWPS main: QOSSGroup,
        LNP lnp: u8,
        LTPS list: TdfMap<&'static str, QOSSGroup>,
        SVID svid: u32
    }
}

group! {
    struct QOSSGroup {
        PSA address: &'static str,
        PSP port: u16,
        SNA name: &'static str
    }
}

pub const QOSS_KEY: &str = "ea-sjc";

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
async fn handle_pre_auth(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let pre_auth = packet.contents::<PreAuthReq>()?;
    let location = pre_auth.client_info.location;

    {
        let mut session_data = session.data.write().await;
        (*session_data).location = location;
    }

    let mut config = TdfMap::with_capacity(3);
    config.insert("pingPeriod", "15s");
    config.insert("voipHeadsetUpdateRate", "1000");
    config.insert("xlspConnectionIdleTimeout", "300");

    let http_port = env::http_port();

    let qoss_main = QOSSGroup {
        address: ADDRESS,
        port: http_port,
        name: "prod-sjc",
    };

    let mut qoss_list = TdfMap::with_capacity(1);
    qoss_list.insert(QOSS_KEY, QOSSGroup {
        address: ADDRESS,
        port: http_port,
        name: "prod-sjc",
    });

    session.response(packet, &PreAuthRes {
        anon: 0,
        asrc: "303107",
        component_ids: vec![0x1, 0x19, 0x4, 0x1c, 0x7, 0x9, 0xf802, 0x7800, 0xf, 0x7801, 0x7802, 0x7803, 0x7805, 0x7806, 0x7d0],
        cngn: "",
        config: PreAuthConfig { config },
        inst: "masseffect-3-pc",
        minr: 0,
        nasp: "cem_ea_id",
        pild: "",
        platform: "pc",
        ptag: "",
        qoss: QOSS {
            main: qoss_main,
            lnp: 0xA,
            list: qoss_list,
            svid: 0x45410805,
        },
        rsrc: "303107",
        version: "Blaze 3.15.08.0 (CL# 1629389)",
    }).await
}

struct PSSDetails {
    address: String,
    port: u16,
}

impl Codec for PSSDetails {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "ADRS", &self.address);
        tag_empty_blob(output, "CSIG");
        tag_str(output, "PJID", "303107");
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
async fn handle_post_auth(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let ext_host = env::ext_host();
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
async fn handle_ping(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let now = SystemTime::now();
    let server_time = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| BlazeError::Other("Unable to calculate elapsed time"))?
        .as_secs();

    {
        let mut session_data = session.data.write().await;
        (*session_data).last_ping = now;
    }

    session.response(packet, &PingRes {
        server_time
    }).await
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
const ME3_COALESCED: &str = include_str!("../../../resources/data/coalesced.dmap");
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
/// packet(Components.UTIL, Commands.FETCH_CLIENT_CONFIG, 0x0, 0x2) {
///   text("CFID", "ME3_DATA")
/// }
/// ```
async fn handle_fetch_client_config(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let fetch_config = packet.contents::<FetchConfigReq>()?;
    let config = match fetch_config.id.as_ref() {
        "ME3_DATA" => data_config(),
        "ME3_MSG" => TdfMap::empty(),
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
        id => if id.starts_with("ME3_LIVE_TLK_PC_") {
            let lang = &id[16..];
            talk_file(lang)
        } else {
            TdfMap::empty()
        }
    };
    session.response(packet, &FetchConfigRes { config })
        .await
}

/// Contents of the default talk dmap file
const ME3_TLK_DEFAULT: &str = include_str!("../../../resources/data/tlk/default.tlk.dmap");

/// Talk files imported from the resources folder
#[derive(RustEmbed)]
#[folder = "resources/data/tlk"]
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

/// Creates a map for the data configuration ME3_DATA client configuration
/// this configuration includes the addresses for the the Galaxy At War
/// server (GAW_SERVER_BASE_URL) and shop image contents (IMG_MNGR_BASE_URL)
/// these urls are set to (gosredirector.ea.com) because the client will
/// redirect this host and handling proxying itself
fn data_config() -> TdfMap<String, String> {
    let ext_host = env::ext_host();
    let http_port = env::http_port();

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
async fn handle_suspend_user_ping(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<SuspendUserPing>()?;
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
async fn handle_user_settings_save(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let req = packet.contents::<UserSettingsSave>()?;
    set_player_data(session, &req.key, req.value).await?;
    session.response_empty(packet).await
}

async fn set_player_data(session: &Session, key: &str, value: String) -> HandleResult {
    if key.starts_with("class") {
        update_player_class(session, key, value).await
            .map_err(|err| err.context("While updating player class"))
    } else if key.starts_with("char") {
        update_player_character(session, key, value).await
            .map_err(|err| err.context("While updating player character"))
    } else {
        update_player_data(session, key, value).await
            .map_err(|err| err.context("While updating player data"))
    }
}

async fn get_player_character(session: &Session, index: u16) -> BlazeResult<PlayerCharacterActiveModel> {
    let player_class = PlayerCharacterEntity::find()
        .filter(player_characters::Column::Index.eq(index))
        .one(session.db())
        .await?;
    if let Some(value) = player_class {
        return Ok(value.into_active_model());
    }
    let player_id = session.expect_player_id().await?;
    Ok(PlayerCharacterActiveModel {
        id: NotSet,
        player_id: Set(player_id),
        index: Set(index),
        kit_name: NotSet,
        name: NotSet,
        tint1: NotSet,
        tint2: NotSet,
        pattern: NotSet,
        pattern_color: NotSet,
        phong: NotSet,
        emissive: NotSet,
        skin_tone: NotSet,
        seconds_played: NotSet,
        timestamp_year: NotSet,
        timestamp_month: NotSet,
        timestamp_day: NotSet,
        timestamp_seconds: NotSet,
        powers: NotSet,
        hotkeys: NotSet,
        weapons: NotSet,
        weapon_mods: NotSet,
        deployed: NotSet,
        leveled_up: NotSet,
    })
}

async fn update_player_character(session: &Session, key: &str, value: String) -> HandleResult {
    if key.len() > 4 {
        let index = key[4..]
            .parse::<u16>()
            .map_err(|_| BlazeError::Other("Invalid index for player class"))?;
        let mut model = get_player_character(session, index).await?;
        if let None = parse_player_character(&mut model, &value) {
            warn!("Failed to fully parse player character: {key} = {value}")
        }
        model.save(session.db()).await?;
    }
    Ok(())
}

fn encode_player_character(model: &PlayerCharacterModel) -> String {
    format!(
        "20;4;{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
        model.kit_name,
        model.name,
        model.tint1,
        model.tint2,
        model.pattern,
        model.pattern_color,
        model.phong,
        model.emissive,
        model.skin_tone,
        model.seconds_played,
        model.timestamp_year,
        model.timestamp_month,
        model.timestamp_day,
        model.timestamp_seconds,
        model.powers,
        model.hotkeys,
        model.weapons,
        model.weapon_mods,
        if model.deployed { "True" } else { "False" },
        if model.leveled_up { "True" } else { "False" },
    )
}

fn parse_player_character(model: &mut PlayerCharacterActiveModel, value: &str) -> Option<()> {
    let mut parser = MEStringParser::new(value)?;
    model.kit_name = Set(parser.next_str()?);
    model.name = Set(parser.next()?);
    model.tint1 = Set(parser.next()?);
    model.tint2 = Set(parser.next()?);
    model.pattern = Set(parser.next()?);
    model.pattern_color = Set(parser.next()?);
    model.phong = Set(parser.next()?);
    model.emissive = Set(parser.next()?);
    model.skin_tone = Set(parser.next()?);
    model.seconds_played = Set(parser.next()?);
    model.timestamp_year = Set(parser.next()?);
    model.timestamp_month = Set(parser.next()?);
    model.timestamp_day = Set(parser.next()?);
    model.timestamp_seconds = Set(parser.next()?);
    model.powers = Set(parser.next_str()?);
    model.hotkeys = Set(parser.next_str()?);
    model.weapons = Set(parser.next_str()?);
    model.weapon_mods = Set(parser.next_str()?);
    model.deployed = Set(parser.next()?);
    model.leveled_up = Set(parser.next()?);
    Some(())
}

async fn get_player_class(session: &Session, index: u16) -> BlazeResult<PlayerClassActiveModel> {
    let player_class = PlayerClassEntity::find()
        .filter(player_classes::Column::Index.eq(index))
        .one(session.db())
        .await?;
    if let Some(value) = player_class {
        return Ok(value.into_active_model());
    }
    let player_id = session.expect_player_id().await?;
    Ok(PlayerClassActiveModel {
        id: NotSet,
        player_id: Set(player_id),
        index: Set(index),
        name: NotSet,
        level: NotSet,
        exp: NotSet,
        promotions: NotSet,
    })
}

async fn update_player_class(session: &Session, key: &str, value: String) -> HandleResult {
    if key.len() > 5 {
        let index = key[5..]
            .parse::<u16>()
            .map_err(|_| BlazeError::Other("Invalid index for player class"))?;
        let mut model = get_player_class(session, index).await?;
        if let None = parse_player_class(&mut model, &value) {
            warn!("Failed to fully parse player class: {key} = {value}")
        }
        model.save(session.db()).await?;
    }
    Ok(())
}

/// Parses the player class data stored in the provided value and modifies the provided
/// player class model accordingly
///
/// # Structure
/// ```
/// 20;4;Adept;20;0;50
/// ```
///
fn parse_player_class(model: &mut PlayerClassActiveModel, value: &str) -> Option<()> {
    let mut parser = MEStringParser::new(value)?;
    model.name = Set(parser.next_str()?);
    model.level = Set(parser.next()?);
    model.exp = Set(parser.next()?);
    model.promotions = Set(parser.next()?);
    Some(())
}

/// Encodes a player class model into a string format for sending to the client
fn encode_player_class(model: &PlayerClassModel) -> String {
    format!(
        "20;4;{};{};{};{}",
        model.name,
        model.level,
        model.exp,
        model.promotions
    )
}

/// Encodes the base player data into a string format for sending to the client
fn encode_player_base(model: &PlayerModel) -> String {
    format!(
        "20;4;{};-1;0;{};0;{};{};0;{}",
        model.credits,
        model.credits_spent,
        model.games_played,
        model.seconds_played,
        model.inventory
    )
}

/// Parses the player data stored in the provided value and modifies the player
/// active model accordingly.
///
/// # Structure
/// ```
/// 20;4;21474;-1;0;0;0;50;180000;0;fff....(LARGE SEQUENCE OF INVENTORY CHARS)
/// ```
fn parse_player_base(model: &mut PlayerActiveModel, value: &str) -> Option<()> {
    let mut parser = MEStringParser::new(value)?;
    model.credits = Set(parser.next()?);
    parser.skip(2); // Skip -1;0
    model.credits_spent = Set(parser.next()?);
    parser.skip(1)?;
    model.games_played = Set(parser.next()?);
    model.seconds_played = Set(parser.next()?);
    parser.skip(1);
    model.inventory = Set(parser.next_str()?);
    Some(())
}

/// Updates the provided model reflecting the changes stored in the provided
/// pair of key and value
//noinspection SpellCheckingInspection
fn update_player_model(model: &mut PlayerActiveModel, key: &str, value: String) {
    match key {
        "Base" => {
            if let None = parse_player_base(model, &value) {
                warn!("Failed to completely parse player base")
            };
        }
        "FaceCodes" => { model.face_codes = Set(Some(value)) }
        "NewItem" => { model.new_item = Set(Some(value)) }
        "csreward" => {
            let value = value.parse::<u16>()
                .unwrap_or(0);
            model.csreward = Set(value)
        }
        "Completion" => { model.new_item = Set(Some(value)) }
        "Progress" => { model.progress = Set(Some(value)) }
        "cscompletion" => { model.cs_completion = Set(Some(value)) }
        "cstimestamps" => { model.cs_timestamps1 = Set(Some(value)) }
        "cstimestamps2" => { model.cs_timestamps2 = Set(Some(value)) }
        "cstimestamps3" => { model.cs_timestamps3 = Set(Some(value)) }
        _ => {}
    }
}

/// Updates the player model stored on this session with the provided key value data pair
/// persisting the changes to the database and updating the stored model.
async fn update_player_data(session: &Session, key: &str, value: String) -> HandleResult {
    let mut session_data = session.data.write().await;
    let player = session_data.expect_player_owned()?;
    let mut active = player.into_active_model();
    update_player_model(&mut active, key, value);
    let result = active.update(session.db()).await?;
    session_data.player = Some(result);
    Ok(())
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
async fn handle_user_settings_load_all(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let mut settings = TdfMap::<String, String>::new();
    {
        let session_data = session.data.read().await;
        let player = session_data.expect_player()?;


        let classes = player
            .find_related(PlayerClassEntity)
            .all(session.db());

        let characters = player
            .find_related(PlayerCharacterEntity)
            .all(session.db());

        let (classes, characters) = try_join!(classes, characters)?;

        let mut index = 0;
        for class in classes {
            settings.insert(format!("class{}", index), encode_player_class(&class));
            index += 1;
        }

        index = 0;
        for char in characters {
            settings.insert(format!("char{}", index), encode_player_character(&char));
            index += 1;
        }

        if let Some(value) = &player.face_codes { settings.insert("FaceCodes", value) }
        if let Some(value) = &player.new_item { settings.insert("NewItem", value) }
        settings.insert("csreward", player.csreward.to_string());
        if let Some(value) = &player.completion { settings.insert("Completion", value) }
        if let Some(value) = &player.progress { settings.insert("Progress", value) }
        if let Some(value) = &player.cs_completion { settings.insert("cscompletion", value) }
        if let Some(value) = &player.cs_timestamps1 { settings.insert("cstimestamps", value) }
        if let Some(value) = &player.cs_timestamps2 { settings.insert("cstimestamps2", value) }
        if let Some(value) = &player.cs_timestamps3 { settings.insert("cstimestamps3", value) }
        settings.insert("Base", encode_player_base(player));
    }
    session.response(packet, &UserSettingsAll {
        settings
    }).await
}