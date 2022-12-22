use crate::{game::GameSnapshot, state::GlobalState, utils::types::GameID};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/games
pub(super) fn router() -> Router {
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
async fn get_games(Query(query): Query<GamesQuery>) -> Json<GamesResponse> {
    /// The default number of games to return in a leaderboard response
    const DEFAULT_COUNT: u8 = 20;

    let count: usize = query.count.unwrap_or(DEFAULT_COUNT) as usize;

    // Calculate the start and ending indexes
    let start_index: usize = query.offset * count;

    // Retrieve the game snapshots
    let (games, more) = GlobalState::games().snapshot(start_index, count).await;

    Json(GamesResponse { games, more })
}

/// Error type used when a game with a specific ID was requested
/// but was not found when attempting to take a snapshot
struct GameNotFound;

/// Route for retrieving the details of a game with a specific game ID
///
/// `game_id` The ID of the game
async fn get_game(Path(game_id): Path<GameID>) -> Result<Json<GameSnapshot>, GameNotFound> {
    let games = GlobalState::games()
        .snapshot_id(game_id)
        .await
        .ok_or(GameNotFound)?;
    Ok(Json(games))
}

/// IntoResponse implementation for GameNotFound to allow it to be
/// used within the result type as a error response
impl IntoResponse for GameNotFound {
    #[inline]
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, "Game with that ID not found").into_response()
    }
}
