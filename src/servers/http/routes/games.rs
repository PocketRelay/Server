use crate::{game::GameSnapshot, state::GlobalState, utils::types::GameID};
use axum::{
    extract::{Path, Query},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: Router) -> Router {
    router
        .route("/api/games", get(get_games))
        .route("/api/games/:id", get(get_game))
}

/// Error type for a game that couldn't be located
#[derive(Debug)]
struct GameNotFound;

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

/// Display for game not found just an error message saying it couldn't
/// find any games with that ID
impl Display for GameNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Game with that ID not found")
    }
}

impl IntoResponse for GameNotFound {
    fn into_response(self) -> Response {
        (StatusCode::NOT_FOUND, self.to_string()).into_response()
    }
}
