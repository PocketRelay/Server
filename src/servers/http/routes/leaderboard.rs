use crate::{leaderboard::models::*, state::GlobalState, utils::types::PlayerID};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use database::DbErr;
use serde::Deserialize;
use std::fmt::Display;

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: &mut Router) {
    router
        .route("/api/leaderboard/n7", get(get_n7))
        .route("/api/leaderboard/cp", get(get_cp));
}

/// The default number of entries to return in a leaderboard response
const DEFAULT_COUNT: usize = 20;

#[derive(Debug)]
pub enum LeaderboardError {
    /// Some server error occurred like a database failure when computing
    /// the leaderboards
    ServerError,
    /// The requested player was not found in the leaderboard
    PlayerNotFound,
}

#[derive(Deserialize)]
pub struct LeaderboardQuery {
    /// The number of ranks to offset by
    offset: Option<usize>,
    /// Th number of items to query for
    count: Option<usize>,
    /// An optional player ID to filter by
    player: Option<PlayerID>,
}

/// Route for retrieving the N7 Ratings leaderboard
///
/// `query` The leaderboard query
async fn get_n7(query: Query<LeaderboardQuery>) -> Result<Response, LeaderboardError> {
    get_leaderboard(query, LeaderboardType::N7Rating).await
}

/// Route for retreiving the Challenge points leaderboard
///
/// `query` The leaderboard query
async fn get_cp(query: Query<LeaderboardQuery>) -> Result<Response, LeaderboardError> {
    get_leaderboard(query, LeaderboardType::ChallengePoints).await
}

/// Retrieves the leaderboard query for the provided leaderboard
/// type returning the response or any errors
///
/// `query` The leaderboard query
/// `ty`    The leaderboard type
async fn get_leaderboard(
    Query(query): Query<LeaderboardQuery>,
    ty: LeaderboardType,
) -> Result<Response, LeaderboardError> {
    let leaderboard = GlobalState::leaderboard();
    let (_, group) = leaderboard.get(ty).await?;
    let group: &LeaderboardEntityGroup = &*group.read().await;

    if let Some(player) = query.player {
        let entry = group.values.iter().find(|entry| entry.player_id == player);
        if let Some(entry) = entry {
            Ok(Json(entry).into_response())
        } else {
            Err(LeaderboardError::PlayerNotFound)
        }
    } else {
        let offset = query.offset.unwrap_or_default();
        let count = query.count.unwrap_or(DEFAULT_COUNT);

        let start_index = offset;
        let end_index = (offset + count).min(group.values.len());
        let values: Option<&[LeaderboardEntry]> = group.values.get(start_index..end_index);
        if let Some(values) = values {
            Ok(Json(values).into_response())
        } else {
            Err(LeaderboardError::ServerError)
        }
    }
}

impl From<DbErr> for LeaderboardError {
    fn from(_: DbErr) -> Self {
        Self::ServerError
    }
}

impl Display for LeaderboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayerNotFound => f.write_str("Player not found"),
            Self::ServerError => f.write_str("Server Error Occurred"),
        }
    }
}

impl LeaderboardError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for LeaderboardError {
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
