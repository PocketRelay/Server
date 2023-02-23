use crate::{
    servers::http::{
        ext::ErrorStatusCode,
        middleware::auth::{AdminAuth, Auth},
    },
    state::GlobalState,
    utils::{
        hashing::{hash_password, verify_password},
        types::PlayerID,
    },
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};
use database::{DatabaseConnection, DbErr, GalaxyAtWar, Player, PlayerData};
use log::error;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use std::fmt::Display;
use validator::validate_email;
/// Router function creates a new router with all the underlying
/// routes for this file.
///
/// Prefix: /api/players
pub fn router() -> Router {
    Router::new()
        .route("/", get(get_players))
        .nest(
            "/self",
            Router::new()
                .route("", get(get_self))
                .route("password", put(update_password))
                .route("details", put(update_details)),
        )
        .route(
            "/:id",
            get(get_player).put(modify_player).delete(delete_player),
        )
        .route("/:id/data", get(all_data))
        .route(
            "/:id/data/:key",
            get(get_data).put(set_data).delete(delete_data),
        )
        .route("/:id/galaxy_at_war", get(get_player_gaw))
}

/// Enum for errors that could occur when accessing any of
/// the players routes
#[derive(Debug)]
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
    /// The account doesn't have permission to complete the action
    InvalidPermission,
    /// Invalid current password was provided when attempting
    /// to update the account password
    InvalidPassword,
}

/// Type alias for players result responses which wraps the provided type in
/// a result where the success is wrapped in Json and the error type is
/// PlayersError
type PlayersJsonResult<T> = PlayersResult<Json<T>>;
type PlayersResult<T> = Result<T, PlayersError>;

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
async fn get_players(
    Query(query): Query<PlayersQuery>,
    _: AdminAuth,
) -> PlayersJsonResult<PlayersResponse> {
    const DEFAULT_COUNT: u8 = 20;
    const DEFAULT_OFFSET: u16 = 0;

    let db = GlobalState::database();
    let count = query.count.unwrap_or(DEFAULT_COUNT);
    let offset = query.offset as u64 * count as u64;
    let (players, more) = Player::all(&db, offset, count as u64).await?;

    Ok(Json(PlayersResponse { players, more }))
}

/// Route for obtaining the player details for the current
/// authentication token
async fn get_self(auth: Auth) -> Json<Player> {
    Json(auth.into_inner())
}

/// Route for retrieving a player from the database with an ID that
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find
async fn get_player(Path(player_id): Path<PlayerID>, _: AdminAuth) -> PlayersJsonResult<Player> {
    let db = GlobalState::database();
    let player = find_player(&db, player_id).await?;
    Ok(Json(player))
}

/// Request to update the basic details of the currently
/// authenticated account
///
/// Will ignore the fields that already match the current
/// account details
#[derive(Deserialize)]
struct UpdateDetailsRequest {
    /// The new or current username
    username: String,
    /// The new or current email
    email: String,
}

/// PUT /api/players/self/details
///
/// Route for updating the basic account details for the
/// currenlty authenticated account. WIll ignore any fields
/// that are already up to date
async fn update_details(
    auth: Auth,
    Json(req): Json<UpdateDetailsRequest>,
) -> PlayersResult<StatusCode> {
    // Obtain the player from auth
    let player = auth.into_inner();

    if !validate_email(&req.email) {
        return Err(PlayersError::InvalidEmail);
    }

    let db = GlobalState::database();

    // Decide whether to update the account email based on whether
    // it has been changed
    let email = if player.email == req.email {
        None
    } else {
        // Check if the email is already taken
        let is_taken = match Player::is_email_taken(&db, &req.email).await {
            Ok(value) => value,
            Err(err) => {
                error!("Failed to check if email address is taken: {:?}", err);
                return Err(PlayersError::ServerError);
            }
        };

        if is_taken {
            return Err(PlayersError::EmailTaken);
        }

        Some(req.email)
    };
    // Decide whether to update the account username based on
    // whether it has been changed
    let username = if player.display_name == req.username {
        None
    } else {
        Some(req.username)
    };

    // Update the details
    let db = GlobalState::database();
    if let Err(err) = player.set_details(&db, username, email).await {
        error!("Failed to update player password: {:?}", err);
        return Err(PlayersError::ServerError);
    }

    // Ok status code indicating updated
    Ok(StatusCode::OK)
}

/// Request to update the password of the current user account
#[derive(Deserialize)]
struct UpdatePasswordRequest {
    /// The current password for the account
    current_password: String,
    /// The new account password
    new_password: String,
}

/// PUT /api/players/self/password
///
/// Route for updating the password of the authenticated account
/// takes the current account password and the new account password
/// as the request data
async fn update_password(
    auth: Auth,
    Json(req): Json<UpdatePasswordRequest>,
) -> PlayersResult<StatusCode> {
    // Obtain the player from auth
    let player = auth.into_inner();

    // Compare the existing passwords
    if !verify_password(&req.current_password, &player.password) {
        return Err(PlayersError::InvalidPassword);
    }

    let password = match hash_password(&req.new_password) {
        Ok(value) => value,
        Err(_) => {
            // This block shouldn't ever be encounted with the current alg
            // but is handled anyway
            return Err(PlayersError::ServerError);
        }
    };

    // Update the password
    let db = GlobalState::database();
    if let Err(err) = player.set_password(&db, password).await {
        error!("Failed to update player password: {:?}", err);
        return Err(PlayersError::ServerError);
    }

    // Ok status code indicating updated
    Ok(StatusCode::OK)
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
    _: AdminAuth,
    Json(req): Json<ModifyPlayerRequest>,
) -> PlayersJsonResult<Player> {
    let db = GlobalState::database();
    let player: Player = find_player(&db, player_id).await?;

    let email = if let Some(email) = req.email {
        // Ensure the email is valid email format
        if !validate_email(&email) {
            return Err(PlayersError::InvalidEmail);
        }

        // Ignore unchanged email field
        if email == player.email {
            None
        } else {
            // Ensure the email is not already taken
            if Player::by_email(&db, &email, player.origin)
                .await?
                .is_some()
            {
                return Err(PlayersError::EmailTaken);
            }
            Some(email)
        }
    } else {
        None
    };

    // Ignore the display name field if it has not changed
    let display_name = req
        .display_name
        .filter(|value| value.ne(&player.display_name));

    // Hash the password value if it is present
    let password = if let Some(password) = req.password.as_ref() {
        let password = hash_password(password).map_err(|_| PlayersError::ServerError)?;
        Some(password)
    } else {
        None
    };

    let player = player
        .update_http(&db, email, display_name, req.origin, password)
        .await?;

    Ok(Json(player))
}

/// Route for deleting a player using its Player ID
///
/// `path` The route path with the ID for the player to find
async fn delete_player(
    auth: AdminAuth,
    Path(player_id): Path<PlayerID>,
) -> Result<Response, PlayersError> {
    // Obtain the authenticated player
    let auth = auth.into_inner();

    let db = GlobalState::database();
    let player: Player = find_player(&db, player_id).await?;

    if auth.id != player.id && auth.role <= player.role {
        return Err(PlayersError::InvalidPermission);
    }

    player.delete(&db).await?;
    Ok(StatusCode::OK.into_response())
}

/// Structure wrapping a vec of player data in order to make
/// it serializable without requiring a hashmap
struct PlayerDataMap(Vec<PlayerData>);

impl Serialize for PlayerDataMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for value in &self.0 {
            map.serialize_entry(&value.key, &value.value)?;
        }
        map.end()
    }
}

/// Route for retrieving the list of classes for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the classes for
async fn all_data(
    Path(player_id): Path<PlayerID>,
    _: AdminAuth,
) -> PlayersJsonResult<PlayerDataMap> {
    let db = GlobalState::database();
    let player: Player = find_player(&db, player_id).await?;
    let data = Player::all_data(player.id, &db).await?;
    Ok(Json(PlayerDataMap(data)))
}

async fn get_data(
    Path((player_id, key)): Path<(PlayerID, String)>,
    auth: Auth,
) -> PlayersJsonResult<PlayerData> {
    let auth = auth.into_inner();
    let db = GlobalState::database();
    let player: Player = find_player(&db, player_id).await?;

    if auth.id != player.id && auth.role <= player.role {
        return Err(PlayersError::InvalidPermission);
    }

    let value = player
        .get_data(&db, &key)
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
    auth: AdminAuth,
    Json(req): Json<SetDataRequest>,
) -> PlayersJsonResult<PlayerData> {
    // Obtain the authenticated player
    let auth = auth.into_inner();

    let db = GlobalState::database();
    let player: Player = find_player(&db, player_id).await?;

    if auth.id != player.id && auth.role <= player.role {
        return Err(PlayersError::InvalidPermission);
    }

    let data = Player::set_data(player.id, &db, key, req.value).await?;
    Ok(Json(data))
}
/// Route for updating the class for a player with the provided {id}
/// at the class {index}
///
/// `path` The route path with the ID for the player to find the classes for and class index
/// `req`  The update class request
async fn delete_data(
    Path((player_id, key)): Path<(PlayerID, String)>,
    auth: AdminAuth,
) -> PlayersJsonResult<()> {
    // Obtain the authenticated player
    let auth = auth.into_inner();

    let db = GlobalState::database();
    let player: Player = find_player(&db, player_id).await?;

    if auth.id != player.id && auth.role <= player.role {
        return Err(PlayersError::InvalidPermission);
    }

    player.delete_data(&db, &key).await?;
    Ok(Json(()))
}

/// Route for retrieving the galaxy at war data for a provided player
/// matches the provided {id}
///
/// `path` The route path with the ID for the player to find the characters for
async fn get_player_gaw(
    Path(player_id): Path<PlayerID>,
    _: AdminAuth,
) -> PlayersJsonResult<GalaxyAtWar> {
    let db = GlobalState::database();
    let player = find_player(&db, player_id).await?;
    let galax_at_war = GalaxyAtWar::find_or_create(&db, &player, 0.0).await?;
    Ok(Json(galax_at_war))
}

/// Display implementation for the PlayersError type. Only the PlayerNotFound
/// error has a custom message. All other errors use "Internal Server Error"
impl Display for PlayersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
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
