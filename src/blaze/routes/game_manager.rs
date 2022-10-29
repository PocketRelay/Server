use blaze_pk::OpaquePacket;
use log::debug;
use crate::blaze::components::GameManager;
use crate::blaze::errors::HandleResult;
use crate::blaze::SessionArc;

/// Routing function for handling packets with the `GameManager` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(session: &SessionArc, component: GameManager, packet: &OpaquePacket) -> HandleResult {
    match component {
        component => {
            debug!("Got GameManager({component:?})");
            packet.debug_decode()?;
            session.response_empty(packet).await
        }
    }
}