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
use serde::Serialize;
use utils::types::GameID;

/// Function for configuring the services in this route
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(games_list).service(game);
}

#[derive(Debug, Serialize)]
pub enum GamesError {
    GameNotFound,
}

impl Display for GamesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameNotFound => f.write_str("Couldn't find any games with that ID"),
        }
    }
}

impl ResponseError for GamesError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::GameNotFound => StatusCode::NOT_FOUND,
        }
    }
}

#[get("/api/games")]
async fn games_list() -> Json<GamesSnapshot> {
    let games = GlobalState::games().snapshot().await;
    Json(games)
}

#[get("/api/games/{id}")]
async fn game(path: Path<GameID>) -> Result<Json<GameSnapshot>, GamesError> {
    let game_id = path.into_inner();
    let games = GlobalState::games()
        .snapshot_id(game_id)
        .await
        .ok_or(GamesError::GameNotFound)?;
    Ok(Json(games))
}
