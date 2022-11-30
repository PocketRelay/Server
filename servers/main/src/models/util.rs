use core::{blaze::codec::Port, constants};

use blaze_pk::{
    codec::{Codec, CodecResult, Reader},
    tag::ValueType,
    tagging::*,
    types::TdfMap,
};
use utils::types::PlayerID;

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
    pub port: u16,
}

impl Codec for TelemetryServer {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "TELE");
        // Last known telemetry address: 159.153.235.32
        tag_str(output, "ADRS", constants::EXTERNAL_HOST);
        tag_zero(output, "ANON");
        tag_str(output, "DISA", TELEMTRY_DISA);
        tag_str(output, "FILT", "-UION/****");
        tag_u32(output, "LOC", 1701727834);
        tag_str(output, "NOOK", "US,CA,MX");
        // Last known telemetry port: 9988
        tag_u16(output, "PORT", self.port);
        tag_u16(output, "SDLY", 15000);
        tag_str(output, "SESS", "pcwdjtOCVpD");
        let key = String::from_utf8_lossy(TELEMETRY_KEY);
        tag_str(output, "SKEY", &key);
        tag_u8(output, "SPCT", 75);
        tag_empty_str(output, "STIM");
        tag_group_end(output);
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

/// Unique identifiyer key for the ticker server
/// PLAYER_ID,TICKER_IP:TICKER_PORT,GAME_NAME,....Other values unknown
const TICKER_KEY: &str = "1,10.23.15.2:8999,masseffect-3-pc,10,50,50,50,50,0,12";

/// Structure for encoding the ticker server details
pub struct TickerServer {
    pub port: u16,
}

impl Codec for TickerServer {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_group_start(output, "TICK");
        // Last known ticker address: 10.23.15.2
        tag_str(output, "ADRS", constants::EXTERNAL_HOST);
        // Last known ticker port: 8999
        tag_u16(output, "PORT", self.port);
        tag_str(output, "SKEY", TICKER_KEY);
        tag_group_end(output);
    }

    fn value_type() -> ValueType {
        ValueType::Group
    }
}

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

impl Codec for PreAuthResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_zero(output, "ANON");
        tag_str(output, "ASRC", SRC_VERSION);
        // This list appears to contain the IDs of the components that the game
        // uses throughout its lifecycle
        tag_value(
            output,
            "CIDS",
            &vec![
                0x1, 0x19, 0x4, 0x1c, 0x7, 0x9, 0xf802, 0x7800, 0xf, 0x7801, 0x7802, 0x7803,
                0x7805, 0x7806, 0x7d0,
            ],
        );
        tag_empty_str(output, "CNGN");
        // Double nested map containing configuration options for
        // ping intervals and VOIP headset update rates
        {
            tag_group_start(output, "CONF");
            {
                tag_map_start(output, "CONF", ValueType::String, ValueType::String, 3);

                "pingPeriod".encode(output);
                PING_PERIOD.encode(output);

                "voipHeadsetUpdateRate".encode(output);
                "1000".encode(output);

                // XLSP (Xbox Live Server Platform)
                "xlspConnectionIdleTimeout".encode(output);
                "300".encode(output);
            }
            tag_group_end(output);
        }
        tag_str(output, "INST", "masseffect-3-pc");
        tag_zero(output, "MINR");
        tag_str(output, "NASP", "cem_ea_id");
        tag_empty_str(output, "PILD");
        tag_str(output, "PLAT", "pc");
        tag_empty_str(output, "PTAG");

        // Quality of service group pre encoded due to it being appended
        // in two locations
        let qoss_group = &mut Vec::new();
        {
            tag_str(qoss_group, "PSA", constants::EXTERNAL_HOST);
            tag_u16(qoss_group, "PSP", self.qos_port);
            tag_str(qoss_group, "SNA", "prod-sjc");
            tag_group_end(qoss_group);
        }

        {
            // Quality Of Service Server details
            tag_group_start(output, "QOSS");
            {
                // Bioware Primary Server
                tag_group_start(output, "BWPS");
                output.extend_from_slice(&qoss_group);
            }

            tag_u8(output, "LNP", 10);

            // List of other Quality Of Service servers? Values present in this
            // list are later included in a ping list
            {
                tag_map_start(output, "LTPS", ValueType::String, ValueType::Group, 1);
                "ea-sjc".encode(output);
                output.extend_from_slice(&qoss_group);
            }

            // Possibly server version ID (1161889797)
            tag_u32(output, "SVID", 0x45410805);
            tag_group_end(output)
        }

        // Server src version
        tag_str(output, "RSRC", SRC_VERSION);
        // Server blaze version
        tag_str(output, "SVER", BLAZE_VERSION)
    }
}

/// Structure for the response to a post authentication request
pub struct PostAuthResponse {
    pub telemetry: TelemetryServer,
    pub ticker: TickerServer,
    pub player_id: PlayerID,
}

impl Codec for PostAuthResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        // Player Sync Service server details
        {
            tag_group_start(output, "PSS");
            tag_str(output, "ADRS", "playersyncservice.ea.com");
            tag_empty_blob(output, "CSIG");
            tag_str(output, "PJID", SRC_VERSION);
            tag_u16(output, "PORT", 443);
            tag_u8(output, "RPRT", 0xF);
            tag_u8(output, "TIID", 0);
            tag_group_end(output);
        }

        tag_value(output, "TELE", &self.telemetry);
        tag_value(output, "TICK", &self.ticker);

        // User options
        {
            tag_group_start(output, "UROP");
            tag_u8(output, "TMOP", 1);
            tag_u32(output, "UID", self.player_id);
            tag_group_end(output);
        }
    }
}

/// Structure for the response to a ping request
pub struct PingResponse {
    /// The number of seconds elapsed since the Unix Epoc
    pub server_time: u64,
}

impl Codec for PingResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_u64(output, "STIM", self.server_time)
    }
}

/// Structure for the request to fetch a specific config
pub struct FetchConfigRequest {
    /// The ID for the config
    pub id: String,
}

impl Codec for FetchConfigRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let id = expect_tag(reader, "CFID")?;
        Ok(Self { id })
    }
}

/// Structure for the response to fetching a config
pub struct FetchConfigResponse {
    pub config: TdfMap<String, String>,
}

impl Codec for FetchConfigResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "CONF", &self.config)
    }
}

/// Structure for the suspend user ping request
pub struct SuspendPingRequest {
    /// The suspend ping value
    pub value: u32,
}

impl Codec for SuspendPingRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let value = expect_tag(reader, "TVAL")?;
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

impl Codec for SettingsSaveRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let value = expect_tag(reader, "DATA")?;
        let key = expect_tag(reader, "KEY")?;
        Ok(Self { key, value })
    }
}

/// Structure for the response to loading all the settings
pub struct SettingsResponse {
    /// The settings map
    pub settings: TdfMap<String, String>,
}

impl Codec for SettingsResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_value(output, "SMAP", &self.settings);
    }
}
