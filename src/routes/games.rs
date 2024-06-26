use crate::{
    config::RuntimeConfig,
    database::entities::players::PlayerRole,
    middleware::auth::MaybeAuth,
    services::game::{manager::GameManager, GameSnapshot},
    utils::types::GameID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Errors that could occur while working with game endpoints
#[derive(Debug, Error)]
pub enum GamesError {
    /// The requested game could not be found (For specific game lookup)
    #[error("Game not found")]
    NotFound,
    #[error("Missing required access")]
    NoPermission,
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

/// Response from the players endpoint which contains a list of
/// players and whether there is more players after
#[derive(Serialize)]
pub struct GamesResponse {
    /// The list of players retrieved
    games: Vec<GameSnapshot>,
    /// Whether there is more players left in the database
    more: bool,
    /// Total number of items available
    total_items: usize,
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
    Extension(game_manager): Extension<Arc<GameManager>>,
    Extension(config): Extension<Arc<RuntimeConfig>>,
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
    let (games, more) = game_manager
        .create_snapshot(offset, count, include_net, include_players)
        .await;

    // Get the total number of games
    let total_games = game_manager.get_total_games().await;

    Ok(Json(GamesResponse {
        games,
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
    Extension(game_manager): Extension<Arc<GameManager>>,
    Extension(config): Extension<Arc<RuntimeConfig>>,
) -> Result<Json<GameSnapshot>, GamesError> {
    if let (None, false) = (&auth, config.api.public_games) {
        return Err(GamesError::NoPermission);
    }

    let include_net = auth
        .as_ref()
        .is_some_and(|player| player.role >= PlayerRole::Admin);
    let include_players = auth.is_some() || !config.api.public_games_hide_players;

    let game = game_manager
        .get_game(game_id)
        .await
        .ok_or(GamesError::NotFound)?;
    let game = &*game.read().await;
    let snapshot = game.snapshot(include_net, include_players);

    Ok(Json(snapshot))
}

/// Response implementation for games errors
impl IntoResponse for GamesError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::NoPermission => StatusCode::FORBIDDEN,
        };

        (status_code, self.to_string()).into_response()
    }
}
