use crate::{
    config::Config,
    database::entities::{
        game_report::{self, GameReportModel},
        players::PlayerRole,
    },
    middleware::auth::MaybeAuth,
    services::game::{snapshot::GameSnapshot, store::Games},
    utils::types::GameID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Errors that could occur while working with game endpoints
#[derive(Debug, Error)]
pub enum GamesError {
    /// The requested game could not be found (For specific game lookup)
    #[error("Game not found")]
    NotFound,

    /// Permission not allowed
    #[error("Missing required access")]
    NoPermission,

    /// Database error occurred
    #[error("Internal server error")]
    Database(#[from] DbErr),
}

/// The query structure for a players query
#[derive(Deserialize)]
pub struct GamesRequest {
    /// The page offset (offset = offset * count)
    #[serde(default)]
    offset: usize,
    /// The number of games to query for count has a maximum limit
    /// of 255 entries to prevent server strain from querying the
    /// entire list of leaderboard entries
    count: Option<u8>,
}

/// Response from the games endpoint which contains a list of
/// games and whether there is more games after
#[derive(Serialize)]
pub struct GamesResponse {
    /// The list of games retrieved
    games: Vec<GameSnapshot>,
    /// Whether there is more players left in the database
    more: bool,
    /// Total number of items available
    total_items: usize,
}

#[derive(Serialize)]
pub struct GameReportsResponse {
    /// The list of game reports retrieved
    games: Vec<GameReportModel>,
    /// Whether there is more reports left in the database
    more: bool,
    /// Total number of pages available
    total_pages: u64,
    /// Total number of items available
    total_items: u64,
}

/// GET /api/games
///
/// Handles requests for a paginated list of games that
/// are actively running. Query provides the start offset
/// and the number of games to respond with.
///
/// Player networking information is included for requesting
/// players with admin level or greater access.
pub async fn get_games(
    MaybeAuth(auth): MaybeAuth,
    Query(GamesRequest { offset, count }): Query<GamesRequest>,
    Extension(games): Extension<Arc<Games>>,
    Extension(config): Extension<Arc<Config>>,
) -> Result<Json<GamesResponse>, GamesError> {
    if let (None, false) = (&auth, config.api.public_games) {
        return Err(GamesError::NoPermission);
    }

    let count: usize = count.unwrap_or(20) as usize;
    let offset: usize = offset * count;
    let include_net = auth
        .as_ref()
        .is_some_and(|player| player.role >= PlayerRole::Admin);
    let include_players = auth.is_some() || !config.api.public_games_hide_players;

    // Retrieve the game snapshots
    let (snapshots, more) = games.create_snapshot(offset, count, include_net, include_players);

    // Get the total number of games
    let total_games = games.total();

    Ok(Json(GamesResponse {
        games: snapshots,
        more,
        total_items: total_games,
    }))
}

/// GET /api/games/:id
///
/// Handles requests for details about a specific game
/// using the ID of the game.
///
/// Player networking information is included for requesting
/// players with admin level or greater access.
pub async fn get_game(
    MaybeAuth(auth): MaybeAuth,
    Path(game_id): Path<GameID>,
    Extension(games): Extension<Arc<Games>>,
    Extension(config): Extension<Arc<Config>>,
) -> Result<Json<GameSnapshot>, GamesError> {
    if let (None, false) = (&auth, config.api.public_games) {
        return Err(GamesError::NoPermission);
    }

    let include_net = auth
        .as_ref()
        .is_some_and(|player| player.role >= PlayerRole::Admin);
    let include_players = auth.is_some() || !config.api.public_games_hide_players;

    let game = games.get_by_id(game_id).ok_or(GamesError::NotFound)?;
    let snapshot = GameSnapshot::new(&game.read(), include_net, include_players);

    Ok(Json(snapshot))
}

/// GET /api/game-reports
///
/// Handles requests for a paginated list of games reports of games
/// that have already ended. Query provides the start offset
/// and the number of games to respond with.
pub async fn get_game_reports(
    MaybeAuth(auth): MaybeAuth,
    Query(GamesRequest { offset, count }): Query<GamesRequest>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<Config>>,
) -> Result<Json<GameReportsResponse>, GamesError> {
    if let (None, false) = (&auth, config.api.public_games) {
        return Err(GamesError::NoPermission);
    }

    const DEFAULT_COUNT: u8 = 20;

    let count = count.unwrap_or(DEFAULT_COUNT);

    let paginator = game_report::Entity::find()
        .order_by_asc(game_report::Column::CreatedAt)
        .paginate(&db, count as u64);
    let page = offset as u64;
    let totals = paginator.num_items_and_pages().await?;
    let more = page < totals.number_of_pages;
    let games = paginator.fetch_page(page).await?;

    Ok(Json(GameReportsResponse {
        games,
        more,
        total_pages: totals.number_of_pages,
        total_items: totals.number_of_items,
    }))
}

/// GET /api/game-reports/{id}
///
/// Get a specific game report
pub async fn get_game_report(
    MaybeAuth(auth): MaybeAuth,
    Path(game_id): Path<GameID>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<Config>>,
) -> Result<Json<GameReportModel>, GamesError> {
    if let (None, false) = (&auth, config.api.public_games) {
        return Err(GamesError::NoPermission);
    }

    let game = game_report::Entity::find_by_id(game_id)
        .one(&db)
        .await?
        .ok_or(GamesError::NotFound)?;

    Ok(Json(game))
}

/// Response implementation for games errors
impl IntoResponse for GamesError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::NoPermission => StatusCode::FORBIDDEN,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}
