use core::{game::manager::GamesSnapshot, GlobalState};

use actix_web::{
    get,
    web::{Data, Json, ServiceConfig},
};

/// Function for configuring the services in this route
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(games_list);
}

#[get("/api/games")]
async fn games_list(global: Data<GlobalState>) -> Json<GamesSnapshot> {
    let games = global.games.snapshot().await;
    Json(games)
}
