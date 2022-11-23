use core::state::GlobalState;
use std::fmt::Display;

use actix_web::{
    get,
    http::StatusCode,
    web::{Json, Path, Query, ServiceConfig},
    ResponseError,
};
use database::{DatabaseConnection, DbErr, GalaxyAtWar, Player, PlayerCharacter, PlayerClass};
use serde::{Deserialize, Serialize};
use utils::types::PlayerID;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_players)
        .service(get_player)
        .service(get_player_full)
        .service(get_player_classes)
        .service(get_player_characters)
        .service(get_player_gaw);
}

/// Enum for errors that could occur when accessing any of
/// the players routes
#[derive(Debug)]
enum PlayersError {
    PlayerNotFound,
    Database(DbErr),
}

/// Type alias for players result responses which wraps the provided type in
/// a result where the success is wrapped in Json and the error type is
/// PlayersError
type PlayersResult<T> = Result<Json<T>, PlayersError>;

/// Attempts to find a player with the provided player ID returning
/// the PlayerNotFound error if the player didn't exist.
///
/// `db`        The database connection
/// `player_id` The ID of the player to find
async fn find_player(db: &DatabaseConnection, player_id: PlayerID) -> Result<Player, PlayersError> {
    Player::by_id(db, player_id)
        .await?
        .ok_or(PlayersError::PlayerNotFound)
}

/// The query structure for a players query
#[derive(Deserialize)]
struct PlayersQuery {
    /// The offset in the database (offset = offset * count)
    #[serde(default)]
    offset: u16,
    /// The number of players to return. This is restricted to
    /// 255 to prevent the database having to do any larger
    /// queries
    count: Option<u8>,
}

/// Response from the players endpoint which contains a list of
/// players and whether there is more players after
#[derive(Serialize)]
struct PlayersResponse {
    /// The list of players retrieved
    players: Vec<Player>,
    /// Whether there is more players left in the database
    more: bool,
}

/// Route for retrieving a list of players from the database. The
/// offset value if used to know how many rows to skip and count
/// is the number of rows to collect. Offset = offset * count
///
/// `query` The query containing the offset and count values
#[get("/api/players")]
async fn get_players(query: Query<PlayersQuery>) -> PlayersResult<PlayersResponse> {
    const DEFAULT_COUNT: u8 = 20;
    const DEFAULT_OFFSET: u16 = 0;

    let query = query.into_inner();
    let db = GlobalState::database();
    let count = query.count.unwrap_or(DEFAULT_COUNT) as u64;
    let offset = query.offset as u64 * count;
    let (players, more) = Player::all(db, offset, count).await?;

    Ok(Json(PlayersResponse { players, more }))
}

/// Route for retrieving a player from the database with an ID that
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find
#[get("/api/players/{id}")]
async fn get_player(path: Path<PlayerID>) -> PlayersResult<Player> {
    let db = GlobalState::database();
    let player = find_player(db, path.into_inner()).await?;
    Ok(Json(player))
}

/// Response structure for a response from the full player route
/// which includes the player as well as all its relations
#[derive(Serialize)]
struct FullPlayerResponse {
    /// Player that was found
    player: Player,
    /// The classes for the player
    classes: Vec<PlayerClass>,
    /// The characters for the player
    characters: Vec<PlayerCharacter>,
    /// The galaxy at war for the player
    galaxy_at_war: GalaxyAtWar,
}

/// Route for retrieving a player from the database with an ID that
/// matches the provided {id} this route will also load all the
/// classes, characters, and galaxy at war data for the player
///
/// `path` The route path with the ID for the player to find
#[get("/api/players/{id}/full")]
async fn get_player_full(path: Path<PlayerID>) -> PlayersResult<FullPlayerResponse> {
    let db = GlobalState::database();
    let player = find_player(db, path.into_inner()).await?;
    let (classes, characters, galaxy_at_war) = player.collect_relations(db).await?;
    Ok(Json(FullPlayerResponse {
        player,
        classes,
        characters,
        galaxy_at_war,
    }))
}

/// Route for retrieving the list of classes for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the classes for
#[get("/api/players/{id}/classes")]
async fn get_player_classes(path: Path<PlayerID>) -> PlayersResult<Vec<PlayerClass>> {
    let db = GlobalState::database();
    let player = find_player(db, path.into_inner()).await?;
    let classes = PlayerClass::find_all(db, &player).await?;
    Ok(Json(classes))
}

/// Route for retrieving the list of characters for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the characters for
#[get("/api/players/{id}/characters")]
async fn get_player_characters(path: Path<PlayerID>) -> PlayersResult<Vec<PlayerCharacter>> {
    let db = GlobalState::database();
    let player = find_player(db, path.into_inner()).await?;
    let characters = PlayerCharacter::find_all(db, &player).await?;
    Ok(Json(characters))
}

/// Route for retrieving the galaxy at war data for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the characters for
#[get("/api/players/{id}/galaxy_at_war")]
async fn get_player_gaw(path: Path<PlayerID>) -> PlayersResult<GalaxyAtWar> {
    let db = GlobalState::database();
    let player = find_player(db, path.into_inner()).await?;
    let galax_at_war = GalaxyAtWar::find_or_create(db, &player, 0.0).await?;
    Ok(Json(galax_at_war))
}

/// Display implementation for the PlayersError type. Only the PlayerNotFound
/// error has a custom message. All other errors use "Internal Server Error"
impl Display for PlayersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayerNotFound => f.write_str("Couldn't find any players with that ID"),
            _ => f.write_str("Internal Server Error"),
        }
    }
}

/// Response code implementation for PlayersError. The PlayerNotFound
/// implementation uses the NOT_FOUND status code and all other errors
/// use INTERNAL_SERVER_ERROR
impl ResponseError for PlayersError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// From implementation for converting database errors into
/// players errors without needing to map the value
impl From<DbErr> for PlayersError {
    fn from(err: DbErr) -> Self {
        PlayersError::Database(err)
    }
}
