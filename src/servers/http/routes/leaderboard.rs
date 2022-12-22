use crate::{
    leaderboard::{models::*, Leaderboard},
    servers::http::ext::ErrorStatusCode,
    state::GlobalState,
    utils::types::PlayerID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/leaderboard
pub fn router() -> Router {
    Router::new()
        .route("/:name", get(get_leaderboard))
        .route("/:name/:player_id", get(get_player_ranking))
}
/// Error type used in leaderboard routes to handle errors
/// such as database errors and player not founds when
/// searching for a specific player.
#[derive(Debug)]
enum LeaderboardError {
    /// Some server error occurred like a database failure when computing
    /// the leaderboards
    ServerError,
    /// The requested player was not found in the leaderboard
    PlayerNotFound,
    /// Error for when a unknown leaderboard is requested
    UnknownLeaderboard,
}

/// Structure of a query requesting a specific leaderboard contains
/// options like offset, count and a player for finding a specific players
/// ranking
#[derive(Deserialize)]
struct LeaderboardQuery {
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
struct LeaderboardResponse<'a> {
    /// The entries retrieved at the provided offset
    entries: &'a [LeaderboardEntry],
    /// Whether there is more entries past the provided offset
    more: bool,
}

/// Retrieves the leaderboard query for the provided leaderboard
/// type returning the response or any errors
///
/// `name`  The name of the leaderboard type to query
/// `query` The leaderboard query
async fn get_leaderboard(
    Path(name): Path<String>,
    Query(query): Query<LeaderboardQuery>,
) -> Result<Response, LeaderboardError> {
    let ty: LeaderboardType =
        LeaderboardType::try_parse(&name).ok_or(LeaderboardError::UnknownLeaderboard)?;

    let leaderboard: &Leaderboard = GlobalState::leaderboard();

    let (_, group) = leaderboard
        .get(ty)
        .await
        .map_err(|_| LeaderboardError::ServerError)?;
    let group: &LeaderboardEntityGroup = &*group.read().await;
    let values: &Vec<LeaderboardEntry> = &group.values;

    /// The default number of entries to return in a leaderboard response
    const DEFAULT_COUNT: u8 = 40;

    let count: u8 = query.count.unwrap_or(DEFAULT_COUNT);

    // Calculate the start and ending indexes
    let start_index: usize = query.offset * (count as usize);
    let end_index: usize = (start_index + (count as usize)).min(values.len());

    // Calculate if there are more entries after the current offset
    let more: bool = group.values.len() > end_index;

    let entries: &[LeaderboardEntry] = values
        .get(start_index..end_index)
        .ok_or(LeaderboardError::ServerError)?;

    let response = LeaderboardResponse { entries, more };

    Ok(Json(response).into_response())
}

/// Retrieves the leaderboard entry for the player with the
/// provided player_id
///
/// `name`      The name of the leaderboard type to query
/// `player_id` The ID of the player to find the leaderboard ranking of
async fn get_player_ranking(
    Path((name, player_id)): Path<(String, PlayerID)>,
) -> Result<Response, LeaderboardError> {
    let ty: LeaderboardType =
        LeaderboardType::try_parse(&name).ok_or(LeaderboardError::UnknownLeaderboard)?;
    let leaderboard: &Leaderboard = GlobalState::leaderboard();
    let (_, group) = leaderboard
        .get(ty)
        .await
        .map_err(|_| LeaderboardError::ServerError)?;
    let group: &LeaderboardEntityGroup = &*group.read().await;
    let values: &Vec<LeaderboardEntry> = &group.values;
    // Find the entry by the player ID
    let entry = values
        .iter()
        .find(|entry| entry.player_id == player_id)
        .ok_or(LeaderboardError::PlayerNotFound)?;
    Ok(Json(entry).into_response())
}

/// Display implementation for the LeaderboardError this will be displayed
/// as the error response message.
impl Display for LeaderboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for LeaderboardError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::PlayerNotFound | Self::UnknownLeaderboard => StatusCode::NOT_FOUND,
            Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// IntoResponse implementation for LeaderboardError to allow it to be
/// used within the result type as a error response
impl IntoResponse for LeaderboardError {
    #[inline]
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
