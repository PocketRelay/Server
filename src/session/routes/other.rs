use crate::{
    session::{models::other::*, packet::Packet, PushExt, SessionLink},
    utils::components::game_reporting,
};

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
pub async fn handle_submit_offline(session: &mut SessionLink) {
    session.push(Packet::notify(
        game_reporting::COMPONENT,
        game_reporting::GAME_REPORT_SUBMITTED,
        GameReportResponse,
    ));
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
pub async fn handle_get_lists(_: &mut SessionLink) -> AssocListResponse {
    AssocListResponse
}
