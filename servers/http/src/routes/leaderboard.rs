use core::{leaderboard::models::LeaderboardType, state::GlobalState};
use std::fmt::Display;

use actix_web::{get, web::ServiceConfig, HttpResponse, Responder, ResponseError};
use database::DbErr;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_n7).service(get_cp);
}

#[derive(Debug)]
pub enum LeaderboardError {
    Db(DbErr),
}

#[get("/api/leaderboard/n7")]
async fn get_n7() -> Result<impl Responder, LeaderboardError> {
    let leaderboard = GlobalState::leaderboard();
    let (_, group) = leaderboard.get(LeaderboardType::N7Rating).await?;
    let group = &*group.read().await;
    let response = HttpResponse::Ok().json(&group.values);
    Ok(response)
}

#[get("/api/leaderboard/cp")]
async fn get_cp() -> Result<impl Responder, LeaderboardError> {
    let leaderboard = GlobalState::leaderboard();
    let (_, group) = leaderboard.get(LeaderboardType::ChallengePoints).await?;
    let group = &*group.read().await;
    let response = HttpResponse::Ok().json(&group.values);
    Ok(response)
}

impl From<DbErr> for LeaderboardError {
    fn from(err: DbErr) -> Self {
        Self::Db(err)
    }
}

impl Display for LeaderboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Server Error Occurred")
    }
}

impl ResponseError for LeaderboardError {}
