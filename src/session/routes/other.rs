use log::error;
use sea_orm::DatabaseConnection;
use tokio::try_join;

use crate::{
    database::entities::{leaderboard_data::LeaderboardType, LeaderboardData},
    session::{
        models::{other::*, stats::SubmitGameReportRequest},
        packet::Packet,
        router::{Blaze, Extension, SessionAuth},
        SessionLink,
    },
    utils::components::game_reporting,
};

/// Handles submission of offline game reports from clients. This contains
/// the new leaderboard information for the player
///
/// ```
/// Route: GameReporting(SubmitOfflineGameReport)
/// ID: 133
/// Content: {
///     "FNSH": 0,
///     "PRVT": VarList [],
///     "RPVT": {
///         "GAME": VarList [1 /* Game ID */],
///         "GAME": {
///             "GAME": {},
///             "PLYR": Map {
///                 1 /* The player */: {
///                     "CTRY": 16725, /* Player country */
///                     "NCHP": 0, /* Challenge points */
///                     "NRAT": 1 /* N7 Rating */
///                 }
///             }
///         }
///     },
///     "GRID": 0,
///     "GTYP": "massEffectReport"
/// }
/// ```
pub async fn handle_submit_offline(
    session: SessionLink,
    SessionAuth(_): SessionAuth,
    Extension(db): Extension<DatabaseConnection>,
    Blaze(SubmitGameReportRequest { report }): Blaze<SubmitGameReportRequest>,
) {
    let game = report.game;
    let players = game.players;

    let n7_data = players
        .iter()
        .map(|(player_id, player_data)| (*player_id, player_data.n7_rating));
    let cp_data = players
        .iter()
        .map(|(player_id, player_data)| (*player_id, player_data.challenge_points));

    if let Err(err) = try_join!(
        LeaderboardData::set_ty_bulk(&db, LeaderboardType::N7Rating, n7_data),
        LeaderboardData::set_ty_bulk(&db, LeaderboardType::ChallengePoints, cp_data),
    ) {
        // TODO: Handle failed to update leaderboards
        error!("Failed to update leaderboards: {}", err);
        return;
    }

    session.notify_handle().notify(Packet::notify(
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
pub async fn handle_get_lists() -> Blaze<AssocListResponse> {
    Blaze(AssocListResponse)
}
