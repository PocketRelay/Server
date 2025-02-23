use super::Port;
use crate::{
    config::{Config, QosServerConfig},
    session::data::KEEP_ALIVE_DELAY,
    utils::types::PlayerID,
};
use bitflags::bitflags;
use std::{borrow::Cow, net::Ipv4Addr, sync::Arc};
use tdf::{TdfDeserialize, TdfMap, TdfSerialize, TdfType, TdfTyped};

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum UtilError {
    SuspendPingTimeTooLarge = 0x12c,
    SuspendPingTimeTooSmall = 0x12d,
    PingSuspended = 0x12e,
}

/// Possibly regions that the telemetry server is disabled for?
pub const TELEMETRY_DISA: &str = "AD,AF,AG,AI,AL,AM,AN,AO,AQ,AR,AS,AW,AX,AZ,BA,BB,BD,BF,BH,BI,BJ,BM,BN,BO,BR,BS,BT,BV,BW,BY,BZ,CC,CD,CF,CG,CI,CK,CL,CM,CN,CO,CR,CU,CV,CX,DJ,DM,DO,DZ,EC,EG,EH,ER,ET,FJ,FK,FM,FO,GA,GD,GE,GF,GG,GH,GI,GL,GM,GN,GP,GQ,GS,GT,GU,GW,GY,HM,HN,HT,ID,IL,IM,IN,IO,IQ,IR,IS,JE,JM,JO,KE,KG,KH,KI,KM,KN,KP,KR,KW,KY,KZ,LA,LB,LC,LI,LK,LR,LS,LY,MA,MC,MD,ME,MG,MH,ML,MM,MN,MO,MP,MQ,MR,MS,MU,MV,MW,MY,MZ,NA,NC,NE,NF,NG,NI,NP,NR,NU,OM,PA,PE,PF,PG,PH,PK,PM,PN,PS,PW,PY,QA,RE,RS,RW,SA,SB,SC,SD,SG,SH,SJ,SL,SM,SN,SO,SR,ST,SV,SY,SZ,TC,TD,TF,TG,TH,TJ,TK,TL,TM,TN,TO,TT,TV,TZ,UA,UG,UM,UY,UZ,VA,VC,VE,VG,VN,VU,WF,WS,YE,YT,ZM,ZW,ZZ";
/// Bytes for the telemetry server key
const TELEMETRY_KEY: &[u8] = &[
    0x5E, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
    0xCE, 0xA4, 0x20, 0xDE, 0x8A, 0x20, 0x20, 0xDC, 0xB0, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
    0x20, 0xE4, 0xB1, 0x99, 0x20, 0xC7, 0x8E, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
    0x20, 0xC2, 0x89, 0x20, 0xC3, 0xAC, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
    0x20, 0x20, 0xCC, 0x89, 0x20, 0x20, 0xCC, 0x9A, 0x20, 0xC8, 0x99, 0x20, 0x20, 0xD8, 0x97, 0x20,
    0x20, 0xCD, 0x9B, 0x20, 0xCC, 0x99, 0x20, 0x20, 0x20, 0x20, 0xEB, 0xB2, 0xA6, 0x20, 0x20, 0x20,
    0x20, 0xC4, 0xA1, 0x20, 0xC6, 0x8C, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0xCB, 0xB2, 0xEE,
    0x88, 0x95, 0xD2, 0x80, 0x20,
];

/// The constant port for the ticker server
pub const TICKER_PORT: Port = 8999;
/// The constant port for the telemetry server
pub const TELEMETRY_PORT: Port = 42129;
// The constant port for the local http server
pub const LOCAL_HTTP_PORT: Port = 42131;

// English locale NZ
pub const LOCALE_NZ: u32 = u32::from_be_bytes(*b"enNZ");

/// Structure for encoding the telemetry server details
pub struct TelemetryServer;

impl TdfSerialize for TelemetryServer {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group(b"TELE", |w| {
            // Last known telemetry addresses: 159.153.235.32, gostelemetry.blaze3.ea.com
            w.tag_str(b"ADRS", "127.0.0.1");
            w.tag_zero(b"ANON");
            w.tag_str(b"DISA", TELEMETRY_DISA);
            w.tag_str(b"FILT", "-UION/****");
            // Encoded locale actually BE encoded string bytes (enNZ)
            w.tag_u32(b"LOC", LOCALE_NZ);
            w.tag_str(b"NOOK", "US,CA,MX");
            // Last known telemetry port: 9988
            w.tag_owned(b"PORT", TELEMETRY_PORT);
            w.tag_u16(b"SDLY", 15000);
            w.tag_str(b"SESS", "pcwdjtOCVpD");
            let key: Cow<str> = String::from_utf8_lossy(TELEMETRY_KEY);

            w.tag_str(b"SKEY", &key);
            w.tag_u8(b"SPCT", 75);
            w.tag_str_empty(b"STIM");
        });
    }
}

/// Unique identifier key for the ticker server
/// PLAYER_ID,TICKER_IP:TICKER_PORT,GAME_NAME,....Other values unknown
const TICKER_KEY: &str = "1,10.23.15.2:8999,masseffect-3-pc,10,50,50,50,50,0,12";

/// Structure for encoding the ticker server details
pub struct TickerServer;

impl TdfSerialize for TickerServer {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.group(b"TICK", |writer| {
            // Last known ticker addresses: 10.23.15.2, 10.10.78.150
            writer.tag_str(b"ADRS", "127.0.0.1");
            // Last known ticker port: 8999
            writer.tag_u16(b"PORT", TICKER_PORT);
            writer.tag_str(b"SKEY", TICKER_KEY);
        });
    }
}

/// Origin auth source?
pub const AUTH_SOURCE: &str = "303107";
pub const BLAZE_VERSION: &str = "Blaze 3.15.08.0 (CL# 1905397)\n";

/// Alias used for ping sites
pub const PING_SITE_ALIAS: &str = "ea-sjc";

/// Structure for the response to a pre authentication request
pub struct PreAuthResponse {
    pub config: Arc<Config>,
}

impl TdfSerialize for PreAuthResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_zero(b"ANON");
        // Authentication source
        w.tag_str(b"ASRC", AUTH_SOURCE);
        // List of components configured on the server.
        w.tag_list_slice(
            b"CIDS",
            &[
                0x1, 0x19, 0x4, 0x1c, 0x7, 0x9, 0xf802, 0x7800, 0xf, 0x7801, 0x7802, 0x7803,
                0x7805, 0x7806, 0x7d0,
            ],
        );
        w.tag_str_empty(b"CNGN");

        let ping_period = format!("{}s", KEEP_ALIVE_DELAY.as_secs());

        // Client configuration provided by the server
        w.group(b"CONF", |w| {
            w.tag_map_tuples(
                b"CONF",
                &[
                    // Client to server ping period
                    ("pingPeriod", ping_period.as_str()),
                    // VOIP headset update rate
                    ("voipHeadsetUpdateRate", "1000"),
                    // XLSP (Xbox Live Server Platform)
                    ("xlspConnectionIdleTimeout", "300"),
                ],
            );
        });

        // Service name.
        w.tag_str(b"INST", "masseffect-3-pc");
        // Underage support
        w.tag_bool(b"MINR", false);
        // Persona namespace
        w.tag_str(b"NASP", "cem_ea_id");
        // Title-specific identifier for legal documents retrieval
        w.tag_str_empty(b"PILD");
        // Server platform.
        w.tag_str(b"PLAT", "pc");

        w.tag_str_empty(b"PTAG");

        // Quality Of Service Server details
        w.group(b"QOSS", |w| {
            let qos = &self.config.qos;

            let mut disabled = false;

            let (http_host, http_port) = match qos {
                QosServerConfig::Official => ("gossjcprod-qos01.ea.com", 17502),
                QosServerConfig::Local => ("127.0.0.1", LOCAL_HTTP_PORT),
                QosServerConfig::Custom { host, port } => (host.as_str(), *port),
                QosServerConfig::Disabled | QosServerConfig::Hamachi { .. } => {
                    disabled = true;
                    ("0", 0)
                }
            };

            // let http_host = "127.0.0.1";
            // let http_port = 17499;

            // (qtyp=2)
            w.group(b"BWPS", |w| {
                w.tag_str(b"PSA", http_host);
                w.tag_u16(b"PSP", http_port);
                w.tag_str(b"SNA", "prod-sjc");
            });

            // Number of probes to send to BWPS
            w.tag_u8(b"LNP", 1);

            // List of other Quality Of Service servers? Values present in this
            // list are later included in a ping list
            {
                w.tag_map_start(
                    b"LTPS",
                    TdfType::String,
                    TdfType::Group,
                    if disabled { 0 } else { 1 },
                );

                if !disabled {
                    // Key for the server
                    PING_SITE_ALIAS.serialize(w);

                    // (qtyp=1)
                    w.group_body(|w| {
                        // Same as the Bioware primary server
                        w.tag_str(b"PSA", http_host);
                        w.tag_u16(b"PSP", http_port);
                        w.tag_str(b"SNA", "prod-sjc");
                    });
                }
            }

            // Possibly server version ID (1161889797)
            w.tag_u32(b"SVID", 0x45410805);
        });

        // Registration source
        w.tag_str(b"RSRC", AUTH_SOURCE);
        // Server version.
        w.tag_str(b"SVER", BLAZE_VERSION)
    }
}

/// Structure for the response to a post authentication request
pub struct PostAuthResponse {
    /// The telemetry server details
    pub telemetry: TelemetryServer,
    /// The ticker server details
    pub ticker: TickerServer,
    /// The player ID of the player who this is for
    pub player_id: PlayerID,
}

impl TdfSerialize for PostAuthResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        // Player Sync Service server details
        w.group(b"PSS", |w| {
            w.tag_str(b"ADRS", "playersyncservice.ea.com");
            w.tag_blob_empty(b"CSIG");
            w.tag_str(b"PJID", AUTH_SOURCE);
            w.tag_u16(b"PORT", 443);
            // Purchases (1) | FriendsList (2) | Achievements (4) | Consumables (8) = 0xF
            w.tag_u8(b"RPRT", 0xF);
            w.tag_u8(b"TIID", 0);
        });

        // Ticker & Telemetry server options
        self.telemetry.serialize(w);
        self.ticker.serialize(w);

        // User options
        w.group(b"UROP", |w| {
            w.tag_u8(b"TMOP", 1);
            w.tag_u32(b"UID", self.player_id);
        });
    }
}

/// Structure for the response to a ping request
#[derive(TdfSerialize)]
pub struct PingResponse {
    /// The number of seconds elapsed since the Unix Epoc
    #[tdf(tag = "STIM")]
    pub server_time: u64,
}

/// Structure for the request to fetch a specific config
#[derive(TdfDeserialize)]
pub struct FetchConfigRequest {
    /// The ID for the config
    #[tdf(tag = "CFID")]
    pub id: String,
}

/// Structure for the response to fetching a config
#[derive(TdfSerialize)]
pub struct FetchConfigResponse {
    /// The configuration map
    #[tdf(tag = "CONF")]
    pub config: TdfMap<String, String>,
}

/// Structure for the suspend user ping request
#[derive(TdfDeserialize)]
pub struct SuspendPingRequest {
    /// The suspend ping value (Suspend time in microseconds)
    #[tdf(tag = "TVAL")]
    pub time_value: u32,
}

/// Structure for the request to update the settings for
/// the current player

#[derive(TdfDeserialize)]
pub struct SettingsSaveRequest {
    /// The new value for the key
    #[tdf(tag = "DATA")]
    pub value: String,
    /// The key to update
    #[tdf(tag = "KEY")]
    pub key: String,
}

/// Structure for the response to loading all the settings
#[derive(TdfDeserialize, TdfSerialize)]
pub struct SettingsResponse {
    /// The settings map
    #[tdf(tag = "SMAP")]
    pub settings: TdfMap<String, String>,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct UpnpFlags: u16 {
        /// NAT type promoted from Moderate to Open due to UPnP success result.
        const NAT_PROMOTED = 0x1;
        /// WAN IP address does not match IP address seen by Blaze server.
        const DOUBLE_NAT = 0x2;
        /// External port derived by QoS was overridden by UPnP external port.
        const PORT_OVERRIDE = 0x4;
    }
}

impl From<u16> for UpnpFlags {
    fn from(value: u16) -> Self {
        Self::from_bits_retain(value)
    }
}

#[derive(Default, Debug, Clone, Copy, TdfDeserialize, TdfTyped)]
#[repr(u8)]
pub enum UpnpStatus {
    /// Upnp status unknown.
    #[default]
    #[tdf(default)]
    Unknown = 0,
    /// Upnp found, but not fully working.
    Found = 1,
    /// Upnp is enabled (found and port mapping added).
    Enabled = 2,
}

/// Contains UPnP data such as status flags, device info, etc.
#[derive(TdfDeserialize)]
pub struct SetClientMetricsRequest {
    /// pnp Blaze status flags.
    #[tdf(tag = "UBFL", into = u16)]
    pub blaze_flags: UpnpFlags,

    /// Upnp device info.
    #[tdf(tag = "UDEV")]
    pub device_info: String,

    /// Upnp status flags.
    #[tdf(tag = "UFLG")]
    pub flags: u16,

    /// Upnp last result code.
    #[tdf(tag = "ULRC")]
    #[allow(unused)]
    pub last_result_code: i32,

    /// Upnp metrics report NAT type.
    #[tdf(tag = "UNAT")]
    pub nat_type: u16,

    /// Upnp status.
    #[tdf(tag = "USTA")]
    pub status: UpnpStatus,

    /// WAN IP address
    #[tdf(tag = "UWAN", into = u32)]
    pub wan: Ipv4Addr,
}
