use blaze_pk::{tag_str, Codec};

/// Packet encoding for Redirector GetServerInstance packets
pub struct RedirectGet;

const BLAZE_SDK_VERSION: &str = "3.15.6.0";

impl Codec for RedirectGet {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_str(output, "BSDK", BLAZE_SDK_VERSION)
    }
}
