use std::ops::DerefMut;
use blaze_pk::{group, OpaquePacket, packet, TdfMap};
use std::time::{SystemTime, UNIX_EPOCH};
use rust_embed::RustEmbed;
use crate::blaze::components::Util;
use crate::blaze::routes::{HandleError, HandleResult, response};
use crate::blaze::Session;
use crate::env;
use crate::env::ADDRESS;
use crate::utils::dmap::load_dmap;

pub async fn route(session: &Session, component: Util, packet: &OpaquePacket) -> HandleResult {
    match component {
        Util::PreAuth => handle_pre_auth(session, packet).await,
        Util::Ping => handle_ping(session, packet).await,
        Util::FetchClientConfig => handle_fetch_client_config(session, packet).await,
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?;
            Ok(None)
        }
    }
}

packet! {
    struct PreAuthReq {
        CDAT client_data: ClientData,
        CINF client_info: ClientInfo,
        FCCR fccr: FCCR
    }
}

group! {
    struct ClientData {
        IITO iito: u32,
        LANG lang: u32,
        SVCN svcn: String,
        TYPE ty: u8,
    }
}

group! {
    struct ClientInfo {
        BSDK blaze_sdk_version: String,
        BTIM blaze_sdk_time: String,
        CLNT client: String,
        CSKU csku: String,
        CVER client_version: String,
        DSDK dsdk: String,
        ENV env: String,
        LOC location: u32,
        MAC mac: String,
        PLAT platform: String
    }
}

group! {
    struct FCCR {
        CFID cfid: String
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

/// Handles the pre-auth packet as is specified above
async fn handle_pre_auth(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let pre_auth = packet.contents::<PreAuthReq>()?;
    let location = pre_auth.client_info.location;

    {
        let mut session = session.write().await;
        let session = session.deref_mut();
        session.location = location;
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

    response(packet, PreAuthRes {
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

async fn handle_ping(session: &Session, packet: &OpaquePacket) -> HandleResult {
    let now = SystemTime::now();
    let server_time = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HandleError::Other("Unable to calculate elapsed time"))?
        .as_secs();

    {
        let mut session = session.write().await;
        let session = session.deref_mut();
        session.last_ping = now;
    }

    response(packet, PingRes {
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

const ME3_COALESCED: &[u8] = include_bytes!("../../../resources/data/coalesced.dmap");
const ME3_ENT: &[u8] = include_bytes!("../../../resources/data/entitlements.dmap");
const ME3_DIME: &str = include_str!("../../../resources/data/dime.xml");
const ME3_TLK_DEFAULT: &[u8] = include_bytes!("../../../resources/data/tlk/default.tlk.dmap");

#[derive(RustEmbed)]
#[folder = "resources/data/tlk"]
struct TLKFiles;

async fn handle_fetch_client_config(_: &Session, packet: &OpaquePacket) -> HandleResult {
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
        id => {
            if id.starts_with("ME3_LIVE_TLK_PC_") {
                let lang = &id[16..];
                let file_name = format!("{lang}.dmap");
                if let Some(file) = TLKFiles::get(&file_name) {
                    load_dmap(file.data.as_ref())
                } else {
                    load_dmap(ME3_TLK_DEFAULT)
                }
            } else {
                TdfMap::empty()
            }
        }
    };
    response(packet, FetchConfigRes {
        config
    })
}

fn data_config() -> TdfMap<String, String> {
    let http_port = env::http_port();
    let mut config = TdfMap::with_capacity(15);
    config.insert("GAW_SERVER_BASE_URL", format!("http://gosredirector.ea.com:{http_port}/gaw"));
    config.insert("IMG_MNGR_BASE_URL", format!("http://gosredirector.ea.com:{http_port}/content/"));
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
    config.insert("TEL_SERVER", "gosredirector.ea.com");
    config
}