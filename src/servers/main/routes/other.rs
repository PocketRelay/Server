use crate::{
    blaze::components::{AssociationLists as A, Components as C, GameReporting as G},
    servers::main::{models::other::*, session::SessionAddr},
};
use blaze_pk::{packet::Packet, router::Router};

/// Routing function for adding all the routes in this file to the
/// provided router
///
/// `router` The router to add to
pub fn route(router: &mut Router<C, SessionAddr>) {
    router.route_stateful(
        C::GameReporting(G::SubmitOfflineGameReport),
        handle_submit_offline,
    );
    router.route(C::AssociationLists(A::GetLists), handle_get_lists);
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
async fn handle_submit_offline(session: SessionAddr) {
    let notify = Packet::notify(C::GameReporting(G::GameReportSubmitted), GameReportResponse);
    session.push(notify);
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
async fn handle_get_lists() -> AssocListResponse {
    AssocListResponse
}
