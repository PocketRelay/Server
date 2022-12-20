use crate::{game::GameSnapshot, state::GlobalState, utils::types::GameID};
use axum::{
    extract::{Path, Query},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: Router) -> Router {
    router
        .route("/api/games", get(get_games))
        .route("/api/games/:id", get(get_game))
}

/// The query structure for a players query
#[derive(Deserialize)]
struct GamesQuery {
    /// The page offset (offset = offset * count)
    #[serde(default)]
    offset: usize,
    /// The number of games to return.
    count: Option<usize>,
}

/// Response from the players endpoint which contains a list of
/// players and whether there is more players after
#[derive(Serialize)]
struct GamesResponse<'a> {
    /// The list of players retrieved
    games: &'a [GameSnapshot],
    /// The current offset page
    offset: usize,
    /// The count expected
    count: usize,
    /// Whether there is more players left in the database
    more: bool,
}

/// Route for retrieving a list of all the games that are currently running.
/// Will take a snapshot of all the games.
///
/// `query` The query containing the offset and count
async fn get_games(Query(query): Query<GamesQuery>) -> Response {
    const DEFAULT_COUNT: usize = 20;
    const DEFAULT_OFFSET: usize = 0;

    let games = GlobalState::games().snapshot().await;

    let games_length: usize = games.len();

    let count = query.count.unwrap_or(DEFAULT_COUNT);
    let offset = query.offset * count;

    let start_index = offset;
    let end_index = (start_index + count).min(games_length);

    let more = games_length > end_index;

    let games: Option<&[GameSnapshot]> = games.get(start_index..end_index);
    let games = games.unwrap_or(&[]);

    let response = GamesResponse {
        games,
        offset: query.offset,
        count,
        more,
    };

    Json(response).into_response()
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
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, "Game with that ID not found").into_response()
    }
}
