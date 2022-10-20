use blaze_pk::{group, OpaquePacket, packet, TdfMap};
use std::time::{SystemTime, UNIX_EPOCH};
use rust_embed::RustEmbed;
use crate::blaze::components::Util;
use crate::blaze::errors::{BlazeError, HandleResult};
use crate::blaze::routes::response;
use crate::blaze::Session;
use crate::env;
use crate::env::ADDRESS;
use crate::utils::dmap::load_dmap;

/// Routing function for handling packets with the `Util` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: Util, packet: &OpaquePacket) -> HandleResult {
    match component {
        Util::PreAuth => handle_pre_auth(session, packet).await,
        Util::Ping => handle_ping(session, packet).await,
        Util::FetchClientConfig => handle_fetch_client_config(session, packet).await,
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?;
            Ok(())
        }
    }
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
    qoss_list.insert("ea-sjc", QOSSGroup {
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
    })
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

    session.response(packet, PingRes {
        server_time
    })
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