use crate::{
    database::entities::{
        leaderboard_data::{LeaderboardDataAndRank, LeaderboardType},
        LeaderboardData,
    },
    utils::types::PlayerID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use sea_orm::{DatabaseConnection, DbErr};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type used in leaderboard routes to handle errors
/// such as database errors and player not founds when
/// searching for a specific player.
#[derive(Debug, Error)]
pub enum LeaderboardError {
    /// The requested player was not found in the leaderboard
    #[error("Player not found")]
    PlayerNotFound,

    /// Database error occurred
    #[error("Internal server error")]
    Database(#[from] DbErr),
}

/// Structure of a query requesting a specific leaderboard contains
/// options like offset, count and a player for finding a specific players
/// ranking
#[derive(Deserialize)]
pub struct LeaderboardQuery {
    /// The number of ranks to offset by
    #[serde(default)]
    offset: u32,
    /// The number of items to query for count has a maximum limit
    /// of 255 entries to prevent server strain from querying the
    /// entire list of leaderboard entries
    count: Option<u8>,
}

/// The different types of response that can be created
/// from a leaderboard request
#[derive(Serialize)]
pub struct LeaderboardResponse {
    /// The total number of players in the entire leaderboard
    total: usize,
    /// The entries retrieved at the provided offset
    entries: Vec<LeaderboardDataAndRank>,
    /// Whether there is more entries past the provided offset
    more: bool,
}

/// GET /api/leaderboard/:name
///
/// Retrieves the leaderboard query for the provided leaderboard
/// type returning the response or any errors
///
/// `name`  The name of the leaderboard type to query
/// `query` The leaderboard query
pub async fn get_leaderboard(
    Path(ty): Path<LeaderboardType>,
    Extension(db): Extension<DatabaseConnection>,
    Query(LeaderboardQuery { offset, count }): Query<LeaderboardQuery>,
) -> Result<Json<LeaderboardResponse>, LeaderboardError> {
    /// The default number of entries to return in a leaderboard response
    const DEFAULT_COUNT: u8 = 40;

    // The number of entries to return
    let count: u32 = count.unwrap_or(DEFAULT_COUNT) as u32;
    // Calculate the start and ending indexes
    let start: u32 = offset * count;

    let values = LeaderboardData::get_offset(&db, ty, start, count).await?;
    let total = LeaderboardData::count(&db, ty).await? as u32;

    // There are more if the end < the total number of values
    let more = (start + count) < (total + 1);

    Ok(Json(LeaderboardResponse {
        total: total as usize,
        entries: values,
        more,
    }))
}

/// GET /api/leaderboard/:name/:player_id
///
/// Retrieves the leaderboard entry for the player with the
/// provided player_id
///
/// `name`      The name of the leaderboard type to query
/// `player_id` The ID of the player to find the leaderboard ranking of
pub async fn get_player_ranking(
    Path((ty, player_id)): Path<(LeaderboardType, PlayerID)>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<LeaderboardDataAndRank>, LeaderboardError> {
    let entry = match LeaderboardData::get_entry(&db, ty, player_id).await? {
        Some(value) => value,
        None => return Err(LeaderboardError::PlayerNotFound),
    };

    Ok(Json(entry))
}

/// IntoResponse implementation for LeaderboardError to allow it to be
/// used within the result type as a error response
impl IntoResponse for LeaderboardError {
    #[inline]
    fn into_response(self) -> Response {
        let status = match &self {
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
