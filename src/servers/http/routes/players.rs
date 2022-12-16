use crate::state::GlobalState;
use actix_web::{
    delete, get,
    http::StatusCode,
    post, put,
    web::{Json, Path, Query, ServiceConfig},
    HttpResponse, Responder, ResponseError,
};
use database::{
    dto::players::PlayerUpdate, DatabaseConnection, DbErr, GalaxyAtWar, Player, PlayerCharacter,
    PlayerClass,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use utils::{hashing::hash_password, types::PlayerID, validate::is_email};

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(get_players)
        .service(get_player)
        .service(get_player_full)
        .service(get_player_classes)
        .service(get_player_characters)
        .service(get_player_gaw)
        .service(modify_player)
        .service(delete_player)
        .service(create_player)
        .service(get_player_class)
        .service(update_player_class);
}

/// Enum for errors that could occur when accessing any of
/// the players routes
#[derive(Debug)]
enum PlayersError {
    PlayerNotFound,
    EmailTaken,
    InvalidEmail,
    ServerError,
    ClassNotFound,
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
    /// The current offset page
    offset: u16,
    /// The count expected
    count: u8,
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
    let count = query.count.unwrap_or(DEFAULT_COUNT);
    let offset = query.offset as u64 * count as u64;
    let (players, more) = Player::all(db, offset, count as u64).await?;

    Ok(Json(PlayersResponse {
        players,
        offset: query.offset,
        count,
        more,
    }))
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

/// Request structure for a request to modify a player entity
/// edits can range from simple name changes to converting the
/// profile to a local profile
#[derive(Deserialize)]
struct ModifyPlayerRequest {
    /// Email value
    email: Option<String>,
    /// Display name value
    display_name: Option<String>,
    /// Origin value
    origin: Option<bool>,
    /// Plain text password to be hashed and used
    password: Option<String>,
    /// Credits value
    credits: Option<u32>,
    /// Inventory value
    inventory: Option<String>,
    /// Challenge reward value
    csreward: Option<u16>,
}

/// Route for modifying a player with the provided ID can take multiple
/// fields to update.
///
/// `path` The route path with the ID for the player to find
/// `req`  The request body
#[put("/api/players/{id}")]
async fn modify_player(
    path: Path<PlayerID>,
    req: Json<ModifyPlayerRequest>,
) -> PlayersResult<Player> {
    let req = req.into_inner();
    let db = GlobalState::database();
    let player: Player = find_player(db, path.into_inner()).await?;

    let email = if let Some(email) = req.email {
        // Ensure the email is valid email format
        if !is_email(&email) {
            return Err(PlayersError::InvalidEmail);
        }

        // Ignore unchanged email field
        if email == player.email {
            None
        } else {
            // Ensure the email is not already taken
            if Player::by_email(db, &email, player.origin).await?.is_some() {
                return Err(PlayersError::EmailTaken);
            }
            Some(email)
        }
    } else {
        None
    };

    // Ignore the display name field if it has not changed
    let display_name = req.display_name.and_then(|value| {
        if value == player.display_name {
            None
        } else {
            Some(value)
        }
    });

    // Hash the password value if it is present
    let password = if let Some(password) = req.password.as_ref() {
        let password = hash_password(password).map_err(|_| PlayersError::ServerError)?;
        Some(password)
    } else {
        None
    };

    let update = PlayerUpdate {
        email,
        display_name,
        origin: req.origin,
        password,
        credits: req.credits,
        inventory: req.inventory,
        csreward: req.csreward,
    };

    let player = player.update_http(db, update).await?;

    Ok(Json(player))
}

/// Request structure for a request to create a new player
#[derive(Deserialize)]
struct CreatePlayerRequest {
    /// The email address of the player to create
    email: String,
    /// The display name of the player to create
    display_name: String,
    /// The plain text password for the player
    password: String,
}

/// Route for creating a new player from the provided creation
/// request.
///
/// `req` The request containing the player details
#[post("/api/players")]
async fn create_player(req: Json<CreatePlayerRequest>) -> PlayersResult<Player> {
    let req = req.into_inner();
    let db = GlobalState::database();
    let email = req.email;
    if !is_email(&email) {
        return Err(PlayersError::InvalidEmail);
    }
    let exists = Player::is_email_taken(db, &email).await?;
    if exists {
        return Err(PlayersError::EmailTaken);
    }
    let password = hash_password(&req.password).map_err(|_| PlayersError::ServerError)?;
    let player: Player = Player::create(db, email, req.display_name, password, false).await?;
    Ok(Json(player))
}

/// Route for deleting a player using its Player ID
///
/// `path` The route path with the ID for the player to find
#[delete("/api/players/{id}")]
async fn delete_player(path: Path<PlayerID>) -> Result<impl Responder, PlayersError> {
    let db = GlobalState::database();
    let player: Player = find_player(db, path.into_inner()).await?;
    player.delete(db).await?;
    Ok(HttpResponse::Ok().finish())
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
    let player: Player = find_player(db, path.into_inner()).await?;
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
    let player: Player = find_player(db, path.into_inner()).await?;
    let classes = PlayerClass::find_all(db, &player).await?;
    Ok(Json(classes))
}

/// Request structure for a request to update the level and or promotions
/// of a class
#[derive(Deserialize)]
struct UpdateClassRequest {
    /// The level to change to
    level: Option<u32>,
    /// The promotions to change to
    promotions: Option<u32>,
}

/// Route for retrieving the list of classes for a provided player
/// matches the provided {id} with the provided {index}
///
/// `path` The route path with the ID for the player to find the classes for
#[get("/api/players/{id}/classes/{index}")]
async fn get_player_class(path: Path<(PlayerID, u16)>) -> PlayersResult<PlayerClass> {
    let (player_id, index) = path.into_inner();
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;
    let class: PlayerClass = PlayerClass::find_index(db, &player, index)
        .await?
        .ok_or(PlayersError::ClassNotFound)?;
    Ok(Json(class))
}

/// Route for updating the class for a player with the provided {id}
/// at the class {index}
///
/// `path` The route path with the ID for the player to find the classes for and class index
/// `req`  The update class request
#[put("/api/players/{id}/classes/{index}")]
async fn update_player_class(
    path: Path<(PlayerID, u16)>,
    req: Json<UpdateClassRequest>,
) -> PlayersResult<PlayerClass> {
    let (player_id, index) = path.into_inner();
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;
    let class: PlayerClass = PlayerClass::find_index(db, &player, index)
        .await?
        .ok_or(PlayersError::ClassNotFound)?;
    let class = class.update_http(db, req.level, req.promotions).await?;
    Ok(Json(class))
}

/// Route for retrieving the list of characters for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the characters for
#[get("/api/players/{id}/characters")]
async fn get_player_characters(path: Path<PlayerID>) -> PlayersResult<Vec<PlayerCharacter>> {
    let db = GlobalState::database();
    let player: Player = find_player(db, path.into_inner()).await?;
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
            Self::ClassNotFound => f.write_str("Class with that index not found"),
            Self::EmailTaken => f.write_str("Email address is already taken"),
            Self::InvalidEmail => f.write_str("Email address is not valid"),
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
            Self::ClassNotFound => StatusCode::NOT_FOUND,
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::EmailTaken | Self::InvalidEmail => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// From implementation for converting database errors into
/// players errors without needing to map the value
impl From<DbErr> for PlayersError {
    fn from(_: DbErr) -> Self {
        PlayersError::ServerError
    }
}
