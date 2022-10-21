use blaze_pk::OpaquePacket;
use log::debug;
use crate::blaze::components::Messaging;
use crate::blaze::errors::HandleResult;
use crate::blaze::Session;

/// Routing function for handling packets with the `Stats` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &Session, component: Messaging, packet: &OpaquePacket) -> HandleResult {
    match component {
        component => {
            debug!("Got Messaging({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}