use core::GlobalState;
use std::fmt::Display;

use actix_web::{
    get,
    http::StatusCode,
    web::{Data, Json, Path, ServiceConfig},
    ResponseError,
};
use database::{
    snapshots::players::{PlayerBasicSnapshot, PlayerDeepSnapshot},
    PlayersInterface,
};
use serde::Serialize;
use utils::types::PlayerID;

/// Function for configuring the services in this route
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(players_list)
        .service(player_deep)
        .service(player_basic);
}

#[derive(Debug, Serialize)]
pub enum PlayersError {
    PlayerNotFound,
    UnknownError,
}

impl Display for PlayersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayerNotFound => f.write_str("Couldn't find any players with that ID"),
            Self::UnknownError => f.write_str("Unknown error occurred"),
        }
    }
}

impl ResponseError for PlayersError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::UnknownError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[get("/api/players")]
async fn players_list(_global: Data<GlobalState>) -> Json<()> {
    Json(())
}

#[get("/api/players/{id}/deep")]
async fn player_deep(
    path: Path<PlayerID>,
    global: Data<GlobalState>,
) -> Result<Json<PlayerDeepSnapshot>, PlayersError> {
    let player_id = path.into_inner();
    let player = PlayersInterface::by_id(&global.db, player_id)
        .await
        .map_err(|_| PlayersError::UnknownError)?
        .ok_or(PlayersError::PlayerNotFound)?;
    let snapshot = PlayerDeepSnapshot::take_snapshot(&global.db, player)
        .await
        .map_err(|_| PlayersError::UnknownError)?;
    Ok(Json(snapshot))
}

#[get("/api/players/{id}")]
async fn player_basic(
    path: Path<PlayerID>,
    global: Data<GlobalState>,
) -> Result<Json<PlayerBasicSnapshot>, PlayersError> {
    let player_id = path.into_inner();
    let player = PlayersInterface::by_id(&global.db, player_id)
        .await
        .map_err(|_| PlayersError::UnknownError)?
        .ok_or(PlayersError::PlayerNotFound)?;
    let snapshot = PlayerBasicSnapshot::take_snapshot(player);
    Ok(Json(snapshot))
}
