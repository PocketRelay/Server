use blaze_pk::OpaquePacket;
use crate::blaze::components::GameManager;
use crate::blaze::routes::HandleResult;
use crate::blaze::Session;

/// Routing function for handling packets with the `GameManager` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
pub async fn route(_session: &Session, component: GameManager, packet: &OpaquePacket) -> HandleResult{
    match component {
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?;
            Ok(None)
        }
    }
}