use crate::{
    blaze::codec::Port,
    utils::{constants, types::PlayerID},
};
use blaze_pk::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
    tag::TdfType,
    types::TdfMap,
    value_type,
    writer::TdfWriter,
};
use std::borrow::Cow;

/// Possibly regions that the telemetry server is disabled for?
const TELEMTRY_DISA: &str = "AD,AF,AG,AI,AL,AM,AN,AO,AQ,AR,AS,AW,AX,AZ,BA,BB,BD,BF,BH,BI,BJ,BM,BN,BO,BR,BS,BT,BV,BW,BY,BZ,CC,CD,CF,CG,CI,CK,CL,CM,CN,CO,CR,CU,CV,CX,DJ,DM,DO,DZ,EC,EG,EH,ER,ET,FJ,FK,FM,FO,GA,GD,GE,GF,GG,GH,GI,GL,GM,GN,GP,GQ,GS,GT,GU,GW,GY,HM,HN,HT,ID,IL,IM,IN,IO,IQ,IR,IS,JE,JM,JO,KE,KG,KH,KI,KM,KN,KP,KR,KW,KY,KZ,LA,LB,LC,LI,LK,LR,LS,LY,MA,MC,MD,ME,MG,MH,ML,MM,MN,MO,MP,MQ,MR,MS,MU,MV,MW,MY,MZ,NA,NC,NE,NF,NG,NI,NP,NR,NU,OM,PA,PE,PF,PG,PH,PK,PM,PN,PS,PW,PY,QA,RE,RS,RW,SA,SB,SC,SD,SG,SH,SJ,SL,SM,SN,SO,SR,ST,SV,SY,SZ,TC,TD,TF,TG,TH,TJ,TK,TL,TM,TN,TO,TT,TV,TZ,UA,UG,UM,UY,UZ,VA,VC,VE,VG,VN,VU,WF,WS,YE,YT,ZM,ZW,ZZ";
/// Bytes for the telemetry server key
const TELEMETRY_KEY: &[u8] = &[
    0x5E, 0x8A, 0xCB, 0xDD, 0xF8, 0xEC, 0xC1, 0x95, 0x98, 0x99, 0xF9, 0x94, 0xC0, 0xAD, 0xEE, 0xFC,
    0xCE, 0xA4, 0x87, 0xDE, 0x8A, 0xA6, 0xCE, 0xDC, 0xB0, 0xEE, 0xE8, 0xE5, 0xB3, 0xF5, 0xAD, 0x9A,
    0xB2, 0xE5, 0xE4, 0xB1, 0x99, 0x86, 0xC7, 0x8E, 0x9B, 0xB0, 0xF4, 0xC0, 0x81, 0xA3, 0xA7, 0x8D,
    0x9C, 0xBA, 0xC2, 0x89, 0xD3, 0xC3, 0xAC, 0x98, 0x96, 0xA4, 0xE0, 0xC0, 0x81, 0x83, 0x86, 0x8C,
    0x98, 0xB0, 0xE0, 0xCC, 0x89, 0x93, 0xC6, 0xCC, 0x9A, 0xE4, 0xC8, 0x99, 0xE3, 0x82, 0xEE, 0xD8,
    0x97, 0xED, 0xC2, 0xCD, 0x9B, 0xD7, 0xCC, 0x99, 0xB3, 0xE5, 0xC6, 0xD1, 0xEB, 0xB2, 0xA6, 0x8B,
    0xB8, 0xE3, 0xD8, 0xC4, 0xA1, 0x83, 0xC6, 0x8C, 0x9C, 0xB6, 0xF0, 0xD0, 0xC1, 0x93, 0x87, 0xCB,
    0xB2, 0xEE, 0x88, 0x95, 0xD2, 0x80, 0x80,
];

/// Structure for encoding the telemetry server details
pub struct TelemetryServer {
    /// The port for the telemetry server
    pub port: u16,
}

impl Encodable for TelemetryServer {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"TELE");
        // Last known telemetry address: 159.153.235.32
        writer.tag_str(b"ADRS", constants::EXTERNAL_HOST);
        writer.tag_zero(b"ANON");
        writer.tag_str(b"DISA", TELEMTRY_DISA);
        writer.tag_str(b"FILT", "-UION/****");
        writer.tag_u32(b"LOC", 1701727834);
        writer.tag_str(b"NOOK", "US,CA,MX");
        // Last known telemetry port: 9988
        writer.tag_u16(b"PORT", self.port);
        writer.tag_u16(b"SDLY", 15000);
        writer.tag_str(b"SESS", "pcwdjtOCVpD");
        let key: Cow<str> = String::from_utf8_lossy(TELEMETRY_KEY);
        writer.tag_str(b"SKEY", &key);
        writer.tag_u8(b"SPCT", 75);
        writer.tag_str_empty(b"STIM");
        writer.tag_group_end();
    }
}

value_type!(TelemetryServer, TdfType::Group);

/// Unique identifiyer key for the ticker server
/// PLAYER_ID,TICKER_IP:TICKER_PORT,GAME_NAME,....Other values unknown
const TICKER_KEY: &str = "1,10.23.15.2:8999,masseffect-3-pc,10,50,50,50,50,0,12";

/// Structure for encoding the ticker server details
pub struct TickerServer {
    /// The port for the ticker server
    pub port: u16,
}

impl Encodable for TickerServer {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_group(b"TICK");
        // Last known ticker address: 10.23.15.2
        writer.tag_str(b"ADRS", constants::EXTERNAL_HOST);
        // Last known ticker port: 8999
        writer.tag_u16(b"PORT", self.port);
        writer.tag_str(b"SKEY", TICKER_KEY);
        writer.tag_group_end();
    }
}

value_type!(TickerServer, TdfType::Group);

/// Server SRC version
pub const SRC_VERSION: &str = "303107";
pub const BLAZE_VERSION: &str = "Blaze 3.15.08.0 (CL# 1629389)";
pub const PING_PERIOD: &str = "15s";

/// Structure for the response to a pre authentication request
pub struct PreAuthResponse {
    /// Port for the Quality Of Service server in our case this is
    /// the HTTP server port
    pub qos_port: Port,
}

impl Encodable for PreAuthResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_zero(b"ANON");
        writer.tag_str(b"ASRC", SRC_VERSION);
        // This list appears to contain the IDs of the components that the game
        // uses throughout its lifecycle
        writer.tag_value(
            b"CIDS",
            &vec![
                0x1, 0x19, 0x4, 0x1c, 0x7, 0x9, 0xf802, 0x7800, 0xf, 0x7801, 0x7802, 0x7803,
                0x7805, 0x7806, 0x7d0,
            ],
        );
        writer.tag_str_empty(b"CNGN");
        // Double nested map containing configuration options for
        // ping intervals and VOIP headset update rates
        {
            writer.tag_group(b"CONF");
            {
                writer.tag_map_start(b"CONF", TdfType::String, TdfType::String, 3);

                writer.write_str("pingPeriod");
                writer.write_str(PING_PERIOD);

                writer.write_str("voipHeadsetUpdateRate");
                writer.write_str("1000");

                // XLSP (Xbox Live Server Platform)
                writer.write_str("xlspConnectionIdleTimeout");
                writer.write_str("300");
            }
            writer.tag_group_end();
        }
        writer.tag_str(b"INST", "masseffect-3-pc");
        writer.tag_zero(b"MINR");
        writer.tag_str(b"NASP", "cem_ea_id");
        writer.tag_str_empty(b"PILD");
        writer.tag_str(b"PLAT", "pc");
        writer.tag_str_empty(b"PTAG");

        // Quality of service group pre encoded due to it being appended
        // in two locations
        let mut qoss_group: TdfWriter = TdfWriter::default();
        {
            qoss_group.tag_str(b"PSA", constants::EXTERNAL_HOST);
            qoss_group.tag_u16(b"PSP", self.qos_port);
            qoss_group.tag_str(b"SNA", "prod-sjc");
            qoss_group.tag_group_end();
        }

        {
            // Quality Of Service Server details
            writer.tag_group(b"QOSS");
            {
                // Bioware Primary Server
                writer.tag_group(b"BWPS");
                writer.write_slice(&qoss_group.buffer);
            }

            writer.tag_u8(b"LNP", 10);

            // List of other Quality Of Service servers? Values present in this
            // list are later included in a ping list
            {
                writer.tag_map_start(b"LTPS", TdfType::String, TdfType::Group, 1);
                writer.write_str("ea-sjc");
                writer.write_slice(&qoss_group.buffer);
            }

            // Possibly server version ID (1161889797)
            writer.tag_u32(b"SVID", 0x45410805);
            writer.tag_group_end()
        }

        // Server src version
        writer.tag_str(b"RSRC", SRC_VERSION);
        // Server blaze version
        writer.tag_str(b"SVER", BLAZE_VERSION)
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
impl Encodable for PostAuthResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        // Player Sync Service server details
        {
            writer.tag_group(b"PSS");
            writer.tag_str(b"ADRS", "playersyncservice.ea.com");
            writer.tag_empty_blob(b"CSIG");
            writer.tag_str(b"PJID", SRC_VERSION);
            writer.tag_u16(b"PORT", 443);
            writer.tag_u8(b"RPRT", 0xF);
            writer.tag_u8(b"TIID", 0);
            writer.tag_group_end();
        }

        // Ticker & Telemtry server options
        writer.tag_value(b"TELE", &self.telemetry);
        writer.tag_value(b"TICK", &self.ticker);

        // User options
        {
            writer.tag_group(b"UROP");
            writer.tag_u8(b"TMOP", 1);
            writer.tag_u32(b"UID", self.player_id);
            writer.tag_group_end();
        }
    }
}

/// Structure for the response to a ping request
pub struct PingResponse {
    /// The number of seconds elapsed since the Unix Epoc
    pub server_time: u64,
}

impl Encodable for PingResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_u64(b"STIM", self.server_time)
    }
}

/// Structure for the request to fetch a specific config
pub struct FetchConfigRequest {
    /// The ID for the config
    pub id: String,
}

impl Decodable for FetchConfigRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let id: String = reader.tag("CFID")?;
        Ok(Self { id })
    }
}

/// Structure for the response to fetching a config
pub struct FetchConfigResponse {
    /// The configuration map
    pub config: TdfMap<String, String>,
}

impl Encodable for FetchConfigResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"CONF", &self.config)
    }
}

/// Structure for the suspend user ping request
pub struct SuspendPingRequest {
    /// The suspend ping value
    pub value: u32,
}

impl Decodable for SuspendPingRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value: u32 = reader.tag("TVAL")?;
        Ok(Self { value })
    }
}

/// Structure for the request to update the settings for
/// the current player
pub struct SettingsSaveRequest {
    /// The key to update
    pub key: String,
    /// The new value for the key
    pub value: String,
}
impl Decodable for SettingsSaveRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value: String = reader.tag("DATA")?;
        let key: String = reader.tag("KEY")?;
        Ok(Self { key, value })
    }
}

/// Structure for the response to loading all the settings
pub struct SettingsResponse {
    /// The settings map
    pub settings: TdfMap<String, String>,
}

impl Encodable for SettingsResponse {
    fn encode(&self, writer: &mut TdfWriter) {
        writer.tag_value(b"SMAP", &self.settings);
    }
}
