use std::sync::Arc;

use crate::{
    services::leaderboard::{models::*, Leaderboard},
    utils::types::PlayerID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type used in leaderboard routes to handle errors
/// such as database errors and player not founds when
/// searching for a specific player.
#[derive(Debug, Error)]
pub enum LeaderboardError {
    /// The provided query range was out of bounds on the underlying query
    #[error("Unacceptable query range")]
    InvalidRange,
    /// The requested player was not found in the leaderboard
    #[error("Player not found")]
    PlayerNotFound,
}

/// Structure of a query requesting a specific leaderboard contains
/// options like offset, count and a player for finding a specific players
/// ranking
#[derive(Deserialize)]
pub struct LeaderboardQuery {
    /// The number of ranks to offset by
    #[serde(default)]
    offset: usize,
    /// The number of items to query for count has a maximum limit
    /// of 255 entries to prevent server strain from querying the
    /// entire list of leaderboard entries
    count: Option<u8>,
}

/// The different types of respones that can be created
/// from a leaderboard request
#[derive(Serialize)]
pub struct LeaderboardResponse<'a> {
    /// The total number of players in the entire leaderboard
    total: usize,
    /// The entries retrieved at the provided offset
    entries: &'a [LeaderboardEntry],
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
    Extension(leaderboard): Extension<Arc<Leaderboard>>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Response, LeaderboardError> {
    let LeaderboardQuery { offset, count } = query;

    /// The default number of entries to return in a leaderboard response
    const DEFAULT_COUNT: u8 = 40;

    // The number of entries to return
    let count: usize = count.unwrap_or(DEFAULT_COUNT) as usize;
    // Calculate the start and ending indexes
    let start: usize = offset * count;

    let group = leaderboard.query(ty, &db).await;

    let (entries, more) = group
        .get_normal(start, count)
        .ok_or(LeaderboardError::InvalidRange)?;

    let response = Json(LeaderboardResponse {
        total: group.values.len(),
        entries,
        more,
    });

    Ok(response.into_response())
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
    Extension(leaderboard): Extension<Arc<Leaderboard>>,
) -> Result<Response, LeaderboardError> {
    let group = leaderboard.query(ty, &db).await;

    let entry = match group.get_entry(player_id) {
        Some(value) => value,
        None => return Err(LeaderboardError::PlayerNotFound),
    };

    let response = Json(entry);
    Ok(response.into_response())
}

/// IntoResponse implementation for LeaderboardError to allow it to be
/// used within the result type as a error response
impl IntoResponse for LeaderboardError {
    #[inline]
    fn into_response(self) -> Response {
        let status = match &self {
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::InvalidRange => StatusCode::BAD_REQUEST,
        };
        (status, self.to_string()).into_response()
    }
}
