use std::sync::Arc;

use crate::{
    database::entities::players::PlayerRole,
    middleware::auth::Auth,
    services::game::{manager::GameManager, GameSnapshot, SnapshotMessage},
    utils::types::GameID,
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use interlink::prelude::LinkError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that could occur while working with game endpoints
#[derive(Debug, Error)]
pub enum GamesError {
    /// The requested game could not be found (For specific game lookup)
    #[error("Game not found")]
    NotFound,
    /// Something went wrong with the link to the games service
    #[error("Failed to access games service")]
    Link(#[from] LinkError),
}

/// Response type alias for JSON responses with GamesError
type GamesRes<T> = Result<Json<T>, GamesError>;

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
    Query(query): Query<GamesRequest>,
    Extension(game_manager): Extension<Arc<GameManager>>,
    Auth(auth): Auth,
) -> GamesRes<GamesResponse> {
    let GamesRequest { offset, count } = query;

    let count: usize = count.unwrap_or(20) as usize;
    let offset: usize = offset * count;
    let include_net = auth.role >= PlayerRole::Admin;

    // Retrieve the game snapshots
    let (games, more) = game_manager
        .create_snapshot(offset, count, include_net)
        .await;

    Ok(Json(GamesResponse { games, more }))
}

/// GET /api/games/:id
///
/// Handles requests for details about a specific game
/// using the ID of the game.
///
/// Player networking information is included for requesting
/// players with admin level or greater access.
pub async fn get_game(
    Path(game_id): Path<GameID>,
    Extension(game_manager): Extension<Arc<GameManager>>,
    Auth(auth): Auth,
) -> GamesRes<GameSnapshot> {
    let game = game_manager
        .get_game(game_id)
        .await
        .ok_or(GamesError::NotFound)?;

    let snapshot = game
        .send(SnapshotMessage {
            include_net: auth.role >= PlayerRole::Admin,
        })
        .await?;

    Ok(Json(snapshot))
}

/// Response implementation for games errors
impl IntoResponse for GamesError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Link(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}
