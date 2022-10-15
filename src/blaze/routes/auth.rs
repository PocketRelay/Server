use blaze_pk::OpaquePacket;
use crate::blaze::components::Authentication;
use crate::blaze::routes::HandleResult;
use crate::blaze::Session;

pub async fn route(_session: Session, component: Authentication, packet: OpaquePacket) -> HandleResult {
    match component {
        component => {
            println!("Got {component:?}");
            packet.debug_decode()?
        }
    }
    Ok(())
}