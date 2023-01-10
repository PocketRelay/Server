use crate::utils::models::{NetGroups, QosNetworkData};
use blaze_pk::{
    codec::Decodable,
    error::{DecodeError, DecodeResult},
    reader::TdfReader,
    types::Union,
};

/// Structure for a request to resume a session using a session token
pub struct ResumeSessionRequest {
    /// The session token to use
    pub session_token: String,
}

impl Decodable for ResumeSessionRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let session_token: String = reader.tag("SKEY")?;
        Ok(Self { session_token })
    }
}

/// Structure for a request to update the network info of the
/// current session
pub struct UpdateNetworkRequest {
    /// The client address net groups
    pub address: NetGroups,
    /// The client Quality of Service data
    pub qos: QosNetworkData,
}

impl Decodable for UpdateNetworkRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let address: NetGroups = match reader.tag::<Union<NetGroups>>("ADDR")? {
            Union::Set { value, .. } => value,
            Union::Unset => return Err(DecodeError::Other("Client address was unset")),
        };
        let qos: QosNetworkData = reader.tag("NQOS")?;
        Ok(Self { address, qos })
    }
}

/// Structure for request to update the hardware flags of the
/// current session
pub struct HardwareFlagRequest {
    /// The hardware flag value
    pub hardware_flag: u16,
}

impl Decodable for HardwareFlagRequest {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let hardware_flag: u16 = reader.tag("HWFG")?;
        Ok(Self { hardware_flag })
    }
}
