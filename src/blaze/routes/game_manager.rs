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

// Create Game
// packet(4, 1) {
//   "ATTR": Map<String, String> {
//     "ME3_dlc2300": "required"
//     "ME3_dlc2500": "required"
//     "ME3_dlc2700": "required"
//     "ME3_dlc3050": "required"
//     "ME3_dlc3225": "required"
//     "ME3gameDifficulty": "difficulty1"
//     "ME3gameEnemyType": "enemy1"
//     "ME3map": "map2"
//     "ME3privacy": "PUBLIC"
//   },
//   "BTPL": (0, 0, 0),
//   "GCTR": "",
//   "GENT": 0,
//   "GNAM": "",
//   "GSET": 287,
//   "GTYP": "",
//   "GURL": "",
//   "HNET": List<Group> [
//     Group {
//       "EXIP": Group {
//         "IP": 0,
//         "PORT": 0,
//       },
//       "INIP": Group {
//         "IP": 3232258299,
//         "PORT": 3659,
//       },
//     }
//   ],
//   "IGNO": 0,
//   "NRES": 0,
//   "NTOP": 0,
//   "PCAP": List<VarInt>[40],
//   "PGID": "",
//   "PGSC": Blob[],
//   "PMAX": 4,
//   "PRES": 1,
//   "QCAP": 0,
//   "RGID": 0,
//   "SLOT": 0,
//   "TCAP": 0,
//   "TIDX": 65535,
//   "VOIP": 2,
//   "VSTR": "ME3-295976325-179181965240128",
// }