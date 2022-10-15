use std::ops::DerefMut;
use blaze_pk::{group, OpaquePacket, packet, Packets, TdfMap};
use crate::blaze::components::Util;
use crate::blaze::routes::HandleResult;
use crate::blaze::{Session, write_packet};
use crate::env;
use crate::env::ADDRESS;

pub async fn route(session: Session, component: Util, packet: OpaquePacket) -> HandleResult {
    match component {
        Util::PreAuth => handle_pre_auth(session, packet).await?,
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?
        }
    }
    Ok(())
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
async fn handle_pre_auth(session: Session, packet: OpaquePacket) -> HandleResult {
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

    let response = PreAuthRes {
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
    };
    let response = Packets::response(&packet, response);
    write_packet(&session, response).await?;
    Ok(())
}