use crate::{leaderboard::models::*, state::GlobalState, utils::types::PlayerID};
use actix_web::{
    get,
    http::StatusCode,
    web::{Query, ServiceConfig},
    HttpResponse, Responder, ResponseError,
};
use database::DbErr;
use serde::Deserialize;
use std::fmt::Display;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_n7);
    cfg.service(get_cp);
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
#[get("/api/leaderboard/n7")]
async fn get_n7(query: Query<LeaderboardQuery>) -> Result<impl Responder, LeaderboardError> {
    get_leaderboard(query, LeaderboardType::N7Rating).await
}

/// Route for retreiving the Challenge points leaderboard
///
/// `query` The leaderboard query
#[get("/api/leaderboard/cp")]
async fn get_cp(query: Query<LeaderboardQuery>) -> Result<impl Responder, LeaderboardError> {
    get_leaderboard(query, LeaderboardType::ChallengePoints).await
}

/// Retrieves the leaderboard query for the provided leaderboard
/// type returning the response or any errors
///
/// `query` The leaderboard query
/// `ty`    The leaderboard type
async fn get_leaderboard(
    query: Query<LeaderboardQuery>,
    ty: LeaderboardType,
) -> Result<impl Responder, LeaderboardError> {
    let leaderboard = GlobalState::leaderboard();
    let (_, group) = leaderboard.get(ty).await?;
    let group: &LeaderboardEntityGroup = &*group.read().await;

    if let Some(player) = query.player {
        let entry = group.values.iter().find(|entry| entry.player_id == player);
        if let Some(entry) = entry {
            let response = HttpResponse::Ok().json(entry);
            Ok(response)
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
            let response = HttpResponse::Ok().json(values);
            Ok(response)
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

impl ResponseError for LeaderboardError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
