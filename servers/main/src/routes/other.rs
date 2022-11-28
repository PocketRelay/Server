use crate::models::other::{AssocListResponse, GameReportResponse};
use crate::session::Session;
use blaze_pk::packet::Packet;
use core::blaze::components::{AssociationLists, Components, GameReporting};
use core::blaze::errors::HandleResult;

/// Routing function for handling packets with the `GameReporting` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of the packet recieved
/// `packet`    The recieved packet
pub async fn route_game_reporting(
    session: &mut Session,
    component: GameReporting,
    packet: &Packet,
) -> HandleResult {
    match component {
        GameReporting::SubmitOfflineGameReport => handle_submit_offline(session, packet).await,
        _ => session.response_empty(packet).await,
    }
}

/// Handles submission of offline game reports from clients.
///
/// ```
/// Route: GameReporting(SubmitOfflineGameReport)
/// ID: 133
/// Content: {
///     "FNSH": 0,
///     "PRVT": VarList [],
///     "RPVT": {
///         "GAME": VarList [1],
///         "GAME": {
///             "GAME": {},
///             "PLYR": Map {
///                 1: {
///                     "CTRY": 16725,
///                     "NCHP": 0,
///                     "NRAT": 1
///                 }
///             }
///         }
///     },
///     "GRID": 0,
///     "GTYP": "massEffectReport"
/// }
/// ```
async fn handle_submit_offline(session: &mut Session, packet: &Packet) -> HandleResult {
    session.response_empty(packet).await?;
    session
        .notify_immediate(
            Components::GameReporting(GameReporting::GameReportSubmitted),
            &GameReportResponse,
        )
        .await
}

/// Routing function for handling packets with the `AssociationLists` component and routing them
/// to the correct routing function. If no routing function is found then the packet
/// is printed to the output and an empty response is sent.
///
/// `session`   The session that the packet was recieved by
/// `component` The component of the packet recieved
/// `packet`    The recieved packet
pub async fn route_association_lists(
    session: &mut Session,
    component: AssociationLists,
    packet: &Packet,
) -> HandleResult {
    match component {
        AssociationLists::GetLists => handle_get_lists(session, packet).await,
        _ => session.response_empty(packet).await,
    }
}
/// Handles getting associated lists for the player
///
/// ```
/// Route: AssociationLists(GetLists)
/// ID: 33
/// Content: {
///     "ALST": [
///         {
///             "BOID": (0, 0, 0),
///             "FLGS": 1,
///             "LID": {
///                 "LNM": "",
///                 "TYPE": 1
///             },
///             "LMS": 0,
///             "PRID": 0
///         }
///     ],
///     "MXRC": 0xFFFFFFFF,
///     "OFRC": 0
/// }
/// ```
async fn handle_get_lists(session: &mut Session, packet: &Packet) -> HandleResult {
    session.response(packet, &AssocListResponse).await
}
