use crate::{
    servers::http::middleware::auth::AdminAuth, services::game::GameSnapshot, state::GlobalState,
    utils::types::GameID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/games
pub fn router() -> Router {
    Router::new()
        .route("/", get(get_games))
        .route("/:id", get(get_game))
}

/// The query structure for a players query
#[derive(Deserialize)]
struct GamesQuery {
    /// The page offset (offset = offset * count)
    #[serde(default)]
    offset: usize,
    /// The number of games to query for count has a maximum limit
    /// of 255 entries to prevent server strain from querying the
    /// entire list of leaderboard entries
    count: Option<u8>,
}

#[derive(Debug, Error)]
pub enum GamesError {
    #[error("GameNotFound")]
    NotFound,
    #[error("InternalServerError")]
    Server,
}

/// Response from the players endpoint which contains a list of
/// players and whether there is more players after
#[derive(Serialize)]
struct GamesResponse {
    /// The list of players retrieved
    games: Vec<GameSnapshot>,
    /// Whether there is more players left in the database
    more: bool,
}

/// Route for retrieving a list of all the games that are currently running.
/// Will take a snapshot of all the games.
///
/// `query` The query containing the offset and count
async fn get_games(
    Query(query): Query<GamesQuery>,
    _: AdminAuth,
) -> Result<Json<GamesResponse>, GamesError> {
    /// The default number of games to return in a leaderboard response
    const DEFAULT_COUNT: u8 = 20;

    let count: usize = query.count.unwrap_or(DEFAULT_COUNT) as usize;

    // Calculate the start and ending indexes
    let start_index: usize = query.offset * count;

    let services = GlobalState::services();
    // Retrieve the game snapshots
    let (games, more) = services
        .game_manager
        .snapshot_query(start_index, count)
        .await
        .ok_or(GamesError::Server)?;

    Ok(Json(GamesResponse { games, more }))
}

/// Route for retrieving the details of a game with a specific game ID
///
/// `game_id` The ID of the game
async fn get_game(
    Path(game_id): Path<GameID>,
    _: AdminAuth,
) -> Result<Json<GameSnapshot>, GamesError> {
    let services = GlobalState::services();
    let games = services
        .game_manager
        .snapshot(game_id)
        .await
        .ok_or(GamesError::NotFound)?;
    Ok(Json(games))
}

impl IntoResponse for GamesError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            GamesError::NotFound => StatusCode::NOT_FOUND,
            GamesError::Server => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}
