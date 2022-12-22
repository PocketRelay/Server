use crate::{
    servers::http::ext::ErrorStatusCode,
    state::GlobalState,
    utils::{hashing::hash_password, types::PlayerID, validate::is_email},
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use database::{
    dto::players::PlayerUpdate, DatabaseConnection, DbErr, GalaxyAtWar, Player, PlayerData,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};

/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/players
pub(super) fn router() -> Router {
    Router::new()
        .route("/", get(get_players).post(create_player))
        .route(
            "/:id",
            get(get_player).put(modify_player).delete(delete_player),
        )
        .route("/:id/data", get(all_data))
        .route("/:id/data/:key", get(get_data).put(set_data))
        .route("/:id/galaxy_at_war", get(get_player_gaw))
}

/// Enum for errors that could occur when accessing any of
/// the players routes
enum PlayersError {
    /// The player with the requested ID was not found
    PlayerNotFound,
    /// The provided email address was already in use
    EmailTaken,
    /// The provided email was not a valid email
    InvalidEmail,
    /// Server error occurred such as failing to hash a password
    /// or a database error
    ServerError,
    /// Requested class could not be found
    DataNotFound,
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
    offset: u32,
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
async fn get_players(Query(query): Query<PlayersQuery>) -> PlayersResult<PlayersResponse> {
    const DEFAULT_COUNT: u8 = 20;
    const DEFAULT_OFFSET: u16 = 0;

    let db = GlobalState::database();
    let count = query.count.unwrap_or(DEFAULT_COUNT);
    let offset = query.offset as u64 * count as u64;
    let (players, more) = Player::all(db, offset, count as u64).await?;

    Ok(Json(PlayersResponse { players, more }))
}

/// Route for retrieving a player from the database with an ID that
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find
async fn get_player(Path(player_id): Path<PlayerID>) -> PlayersResult<Player> {
    let db = GlobalState::database();
    let player = find_player(db, player_id).await?;
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
}

/// Route for modifying a player with the provided ID can take multiple
/// fields to update.
///
/// `path` The route path with the ID for the player to find
/// `req`  The request body
async fn modify_player(
    Path(player_id): Path<PlayerID>,
    Json(req): Json<ModifyPlayerRequest>,
) -> PlayersResult<Player> {
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;

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
async fn create_player(Json(req): Json<CreatePlayerRequest>) -> PlayersResult<Player> {
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
async fn delete_player(Path(player_id): Path<PlayerID>) -> Result<Response, PlayersError> {
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;
    player.delete(db).await?;
    Ok(StatusCode::OK.into_response())
}

/// Route for retrieving the list of classes for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the classes for
async fn all_data(Path(player_id): Path<PlayerID>) -> PlayersResult<HashMap<String, String>> {
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;
    let data = player.all_data(db).await?;
    let mut output = HashMap::with_capacity(data.len());
    for value in data {
        output.insert(value.key, value.value);
    }

    Ok(Json(output))
}

async fn get_data(Path((player_id, key)): Path<(PlayerID, String)>) -> PlayersResult<PlayerData> {
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;
    let value = player
        .get_data(db, &key)
        .await?
        .ok_or(PlayersError::DataNotFound)?;
    Ok(Json(value))
}

/// Request structure for a request to update the level and or promotions
/// of a class
#[derive(Deserialize)]
struct SetDataRequest {
    value: String,
}

/// Route for updating the class for a player with the provided {id}
/// at the class {index}
///
/// `path` The route path with the ID for the player to find the classes for and class index
/// `req`  The update class request
async fn set_data(
    Path((player_id, key)): Path<(PlayerID, String)>,
    Json(req): Json<SetDataRequest>,
) -> PlayersResult<PlayerData> {
    let db = GlobalState::database();
    let player: Player = find_player(db, player_id).await?;
    let data = player.set_data(db, key, req.value).await?;
    Ok(Json(data))
}

/// Route for retrieving the galaxy at war data for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the characters for
async fn get_player_gaw(Path(player_id): Path<PlayerID>) -> PlayersResult<GalaxyAtWar> {
    let db = GlobalState::database();
    let player = find_player(db, player_id).await?;
    let galax_at_war = GalaxyAtWar::find_or_create(db, &player, 0.0).await?;
    Ok(Json(galax_at_war))
}

/// Display implementation for the PlayersError type. Only the PlayerNotFound
/// error has a custom message. All other errors use "Internal Server Error"
impl Display for PlayersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataNotFound => f.write_str("Class with that index not found"),
            Self::EmailTaken => f.write_str("Email address is already taken"),
            Self::InvalidEmail => f.write_str("Email address is not valid"),
            Self::PlayerNotFound => f.write_str("Couldn't find any players with that ID"),
            _ => f.write_str("Internal Server Error"),
        }
    }
}

/// Error status code implementation for the different error
/// status codes of each error
impl ErrorStatusCode for PlayersError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::DataNotFound => StatusCode::NOT_FOUND,
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

/// IntoResponse implementation for PlayersError to allow it to be
/// used within the result type as a error response
impl IntoResponse for PlayersError {
    #[inline]
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
