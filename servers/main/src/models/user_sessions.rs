use core::blaze::codec::{NetGroups, QosNetworkData};

use blaze_pk::{
    codec::{Codec, CodecError, CodecResult, Reader},
    tagging::expect_tag,
    types::Union,
};

/// Structure for a request to resume a session using a session token
pub struct ResumeSessionRequest {
    /// The session token to use
    pub session_token: String,
}

impl Codec for ResumeSessionRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let session_token = expect_tag(reader, "SKEY")?;
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

impl Codec for UpdateNetworkRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let address = match expect_tag(reader, "ADDR")? {
            Union::Set { value, .. } => value,
            Union::Unset => return Err(CodecError::Other("Client address was unset")),
        };
        let qos = expect_tag(reader, "NQOS")?;
        Ok(Self { address, qos })
    }
}

/// Structure for request to update the hardware flags of the
/// current session
pub struct HardwareFlagRequest {
    pub hardware_flag: u16,
}

impl Codec for HardwareFlagRequest {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let hardware_flag = expect_tag(reader, "HWFG")?;
        Ok(Self { hardware_flag })
    }
}
