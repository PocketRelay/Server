use core::{
    game::{game::GameSnapshot, manager::GamesSnapshot},
    state::GlobalState,
};
use std::fmt::Display;

use actix_web::{
    get,
    http::StatusCode,
    web::{Json, Path, ServiceConfig},
    ResponseError,
};

use utils::types::GameID;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_games).service(get_game);
}

/// Error type for a game that couldn't be located
#[derive(Debug)]
struct GameNotFound;

/// Route for retrieving a list of all the games that are currently running.
/// Will take a snapshot of all the games.
#[get("/api/games")]
async fn get_games() -> Json<GamesSnapshot> {
    let games = GlobalState::games().snapshot().await;
    Json(games)
}

/// Route for retrieving the details of a game with a specific game ID
///
/// `game_id` The ID of the game
#[get("/api/games/{id}")]
async fn get_game(game_id: Path<GameID>) -> Result<Json<GameSnapshot>, GameNotFound> {
    let games = GlobalState::games()
        .snapshot_id(game_id.into_inner())
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

/// Game not found responses are always 404 errors
impl ResponseError for GameNotFound {
    fn status_code(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }
}
