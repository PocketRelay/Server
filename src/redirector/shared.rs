use blaze_pk::{
    tag_bool, tag_group_end, tag_group_start, tag_optional_start, tag_str, tag_u16, tag_u32, Codec,
};

use crate::blaze::shared::NetAddress;

#[derive(Debug, Clone)]
pub enum InstanceType {
    Host(String),
    Address(NetAddress),
}

impl InstanceType {
    pub fn from_host(value: String) -> Self {
        if let Some(address) = NetAddress::try_from_ipv4(&value) {
            Self::Address(address)
        } else {
            Self::Host(value)
        }
    }
}

#[derive(Debug, Clone)]
pub struct RedirectorInstance {
    value: InstanceType,
    port: u16,
}

impl RedirectorInstance {
    pub fn new(value: InstanceType, port: u16) -> Self {
        Self { value, port }
    }
}

impl Codec for RedirectorInstance {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_optional_start(output, "ADDR", 0x0);
        {
            tag_group_start(output, "VALU");
            match &self.value {
                InstanceType::Host(host) => tag_str(output, "HOST", host),
                InstanceType::Address(address) => tag_u32(output, "IP", address.0),
            }
            tag_u16(output, "PORT", self.port);
            tag_group_end(output);
        }
        tag_bool(output, "SECU", false);
        tag_bool(output, "XDNS", false);
    }
}
