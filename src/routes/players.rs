use crate::{
    database::{
        entities::players,
        entities::players::PlayerRole,
        entities::{GalaxyAtWar, Player, PlayerData},
        DatabaseConnection, DbErr,
    },
    middleware::auth::{AdminAuth, Auth},
    utils::{
        hashing::{hash_password, verify_password},
        types::PlayerID,
    },
};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use email_address::EmailAddress;
use log::error;
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};
use serde::{ser::SerializeMap, Deserialize, Serialize};
use thiserror::Error;

/// Enum for errors that could occur when accessing any of
/// the players routes
#[derive(Debug, Error)]
pub enum PlayersError {
    /// The player with the requested ID was not found
    #[error("Unable to find requested player")]
    PlayerNotFound,

    /// The provided email address was already in use
    #[error("Email address already in use")]
    EmailTaken,

    /// The provided email was not a valid email
    #[error("Invalid email address")]
    InvalidEmail,

    /// Database error occurred
    #[error("Internal server error")]
    Database(#[from] DbErr),

    /// Failed to create a password hash
    #[error("Internal server error")]
    PasswordHash(#[from] argon2::password_hash::Error),

    /// Requested class could not be found
    #[error("Unable to find data")]
    DataNotFound,

    /// The account doesn't have permission to complete the action
    #[error("Invalid permission")]
    InvalidPermission,

    /// Invalid current password was provided when attempting
    /// to update the account password
    #[error("Invalid password")]
    InvalidPassword,
}

/// Type alias for players result responses which wraps the provided type in
/// a result where the success is wrapped in Json and the error type is
/// PlayersError
type PlayersRes<T> = PlayersResult<Json<T>>;
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
pub struct PlayersQuery {
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
pub struct PlayersResponse {
    /// The list of players retrieved
    players: Vec<Player>,
    /// Whether there is more players left in the database
    more: bool,
    /// Total number of pages available
    total_pages: u64,
    /// Total number of items available
    total_items: u64,
}

/// GET /api/players
///
/// Route for retrieving a list of players from the database. The
/// offset value if used to know how many rows to skip and count
/// is the number of rows to collect. Offset = offset * count
///
/// `query` The query containing the offset and count values
pub async fn get_players(
    _: AdminAuth,
    Query(query): Query<PlayersQuery>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<PlayersResponse>, PlayersError> {
    const DEFAULT_COUNT: u8 = 20;

    let count = query.count.unwrap_or(DEFAULT_COUNT);

    let paginator = players::Entity::find()
        .order_by_asc(players::Column::Id)
        .paginate(&db, count as u64);
    let page = query.offset as u64;
    let totals = paginator.num_items_and_pages().await?;
    let more = page < totals.number_of_pages;
    let players = paginator.fetch_page(page).await?;

    Ok(Json(PlayersResponse {
        players,
        more,
        total_pages: totals.number_of_pages,
        total_items: totals.number_of_items,
    }))
}

/// GET /api/players/self
///
/// Route for obtaining the player details for the current
/// authentication token
///
/// `auth` The currently authenticated player
pub async fn get_self(Auth(auth): Auth) -> Json<Player> {
    Json(auth)
}

/// GET /api/players/:id
///
/// Route for retrieving a player from the database with an ID that
/// matches the provided {id}
///
/// `player_id` The ID of the player to get
pub async fn get_player(
    _: AdminAuth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Player>, PlayersError> {
    let player = find_player(&db, player_id).await?;
    Ok(Json(player))
}

/// Request to update the basic details of the currently
/// authenticated account
///
/// Will ignore the fields that already match the current
/// account details
#[derive(Deserialize)]
pub struct UpdateDetailsRequest {
    /// The new or current username
    username: String,
    /// The new or current email
    email: String,
}

/// PUT /api/players/:id/details
///
/// Admin route for updating the basic details of another
/// account.
///
/// `player_id` The ID of the player to set the details for
/// `auth`      The currently authenticated player
/// `req`       The update details request
pub async fn set_details(
    AdminAuth(auth): AdminAuth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
    Json(req): Json<UpdateDetailsRequest>,
) -> Result<(), PlayersError> {
    // Get the target player
    let player = find_player(&db, player_id).await?;

    // Check modification permission
    if !auth.has_permission_over(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    attempt_set_details(&db, player, req).await?;

    // Ok status code indicating updated
    Ok(())
}

/// PUT /api/players/self/details
///
/// Route for updating the basic account details for the
/// currently authenticated account. WIll ignore any fields
/// that are already up to date
///
/// `auth` The currently authenticated player
/// `req`  The details update request
pub async fn update_details(
    Auth(auth): Auth,
    Extension(db): Extension<DatabaseConnection>,
    Json(req): Json<UpdateDetailsRequest>,
) -> Result<(), PlayersError> {
    if !EmailAddress::is_valid(&req.email) {
        return Err(PlayersError::InvalidEmail);
    }

    attempt_set_details(&db, auth, req).await?;

    // Ok status code indicating updated
    Ok(())
}

/// Attempts to set the details for the provided account using the
/// provided details request
///
/// `db`     The database connection
/// `player` The player to set the details for
/// `req`    The update request
async fn attempt_set_details(
    db: &DatabaseConnection,
    player: Player,
    req: UpdateDetailsRequest,
) -> Result<(), PlayersError> {
    // Decide whether to update the account email based on whether
    // it has been changed
    let email = if player.email == req.email {
        None
    } else {
        // Email taken checking
        if Player::by_email(db, &req.email).await?.is_some() {
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
    player.set_details(db, username, email).await?;

    Ok(())
}

/// Request to set the password of another account
#[derive(Deserialize)]
pub struct SetPasswordRequest {
    /// The new password for the account
    password: String,
}

/// PUT /api/players/:id/password
///
/// Admin route for setting the password of another account
/// to the desired password. Requires that the authenticated
/// account has a higher role than the target account
///
/// `player_id` The ID of the player to set the password for
/// `auth`      The currently authenticated (Admin) player
/// `req`       The password set request
pub async fn set_password(
    AdminAuth(auth): AdminAuth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
    Json(SetPasswordRequest { password }): Json<SetPasswordRequest>,
) -> PlayersResult<()> {
    // Get the target player
    let player = find_player(&db, player_id).await?;

    // Check modification permission
    if !auth.has_permission_over(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    let password = hash_password(&password)?;
    player.set_password(&db, password).await?;

    // Ok status code indicating updated
    Ok(())
}

/// Request to set the role of a player only allowed
/// to be used by SuperAdmin's and can only set
/// between Default and Admin roles
#[derive(Deserialize)]
pub struct SetPlayerRoleRequest {
    /// The role to give the player
    role: PlayerRole,
}

pub async fn set_role(
    AdminAuth(auth): AdminAuth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
    Json(SetPlayerRoleRequest { role }): Json<SetPlayerRoleRequest>,
) -> PlayersResult<()> {
    // Super admin role cannot be granted by anyone but the server
    if let PlayerRole::SuperAdmin = role {
        return Err(PlayersError::InvalidPermission);
    }

    // Changing an account role requires Super Admin permission
    if auth.role != PlayerRole::SuperAdmin {
        return Err(PlayersError::InvalidPermission);
    }

    // Get the target player
    let player = find_player(&db, player_id).await?;

    player.set_role(&db, role).await?;

    Ok(())
}

/// Request to update the password of the current user account
#[derive(Deserialize)]
pub struct UpdatePasswordRequest {
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
pub async fn update_password(
    Auth(player): Auth,
    Extension(db): Extension<DatabaseConnection>,
    Json(UpdatePasswordRequest {
        current_password,
        new_password,
    }): Json<UpdatePasswordRequest>,
) -> PlayersResult<()> {
    let player_password: &str = player
        .password
        .as_ref()
        .ok_or(PlayersError::InvalidPassword)?;

    // Compare the existing passwords
    if !verify_password(&current_password, player_password) {
        return Err(PlayersError::InvalidPassword);
    }

    let password = hash_password(&new_password)?;
    player.set_password(&db, password).await?;

    Ok(())
}

/// DELETE /api/players/:id
///
/// Route for deleting a player using its Player ID
///
/// `player_id` The ID of the player to delete
/// `auth`      The currently authenticated (Admin) player
pub async fn delete_player(
    AdminAuth(auth): AdminAuth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
) -> PlayersResult<()> {
    let player: Player = find_player(&db, player_id).await?;

    if !auth.can_delete(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    player.delete(&db).await?;
    Ok(())
}
/// Request to update the password of the current user account
#[derive(Deserialize)]
pub struct DeleteSelfRequest {
    /// Account password for deletion
    password: String,
}

/// DELETE /api/players/self
///
/// Route for deleting the authenticated player
pub async fn delete_self(
    Auth(auth): Auth,
    Extension(db): Extension<DatabaseConnection>,
    Json(req): Json<DeleteSelfRequest>,
) -> PlayersResult<()> {
    let player_password: &str = auth
        .password
        .as_ref()
        .ok_or(PlayersError::InvalidPassword)?;

    // Compare the existing passwords
    if !verify_password(&req.password, player_password) {
        return Err(PlayersError::InvalidPassword);
    }

    auth.delete(&db).await?;
    Ok(())
}

/// Structure wrapping a vec of player data in order to make
/// it serializable without requiring a hashmap
pub struct PlayerDataMap(Vec<PlayerData>);

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

/// GET /api/players/:id/data
///
/// Route for retrieving the list of classes for a provided player
/// matches the provided {id}
///
/// `player_id` The ID of the player
pub async fn all_data(
    Auth(auth): Auth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
) -> PlayersRes<PlayerDataMap> {
    let player: Player = find_player(&db, player_id).await?;

    if !auth.has_permission_over(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    let data = PlayerData::all(&db, player_id).await?;
    Ok(Json(PlayerDataMap(data)))
}

/// GET /api/players/:id/data/:key
///
/// Route for getting a specific piece of player data for
/// a specific player using the ID of the player and the
/// key of the data
///  
/// `player_id` The ID of the player
/// `key`       The player data key
/// `auth`      The currently authenticated player
pub async fn get_data(
    Auth(auth): Auth,
    Path((player_id, key)): Path<(PlayerID, String)>,
    Extension(db): Extension<DatabaseConnection>,
) -> PlayersRes<PlayerData> {
    let player: Player = find_player(&db, player_id).await?;

    if !auth.has_permission_over(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    let value = PlayerData::get(&db, player.id, &key)
        .await?
        .ok_or(PlayersError::DataNotFound)?;
    Ok(Json(value))
}

/// Request structure for a request to update the level and or promotions
/// of a class
#[derive(Deserialize)]
pub struct SetDataRequest {
    value: String,
}

/// PUT /api/players/:id/data/:key
///
/// Route for setting a piece of player data for a specific
/// player using the key of the data
///
/// `player_id` The ID of the player
/// `key`       The player data key
/// `auth`      The currently authenticated (Admin) player
/// `req`       The request containing the data value
pub async fn set_data(
    AdminAuth(auth): AdminAuth,
    Path((player_id, key)): Path<(PlayerID, String)>,
    Extension(db): Extension<DatabaseConnection>,
    Json(SetDataRequest { value }): Json<SetDataRequest>,
) -> PlayersResult<()> {
    let player: Player = find_player(&db, player_id).await?;

    if !auth.has_permission_over(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    PlayerData::set(&db, player.id, key.clone(), value).await?;

    Ok(())
}

/// DELETE /api/players/:id/data/:key
///
/// Route for deleting the player data for a specific player
/// using the key of the data
///
/// `player_id` The ID of the player
/// `key`       The player data key
/// `auth`      The currently authenticated (Admin) player
pub async fn delete_data(
    AdminAuth(auth): AdminAuth,
    Path((player_id, key)): Path<(PlayerID, String)>,
    Extension(db): Extension<DatabaseConnection>,
) -> PlayersResult<()> {
    let player: Player = find_player(&db, player_id).await?;

    if !auth.has_permission_over(&player) {
        return Err(PlayersError::InvalidPermission);
    }

    PlayerData::delete(&db, player.id, &key).await?;

    Ok(())
}

/// GET /api/players/:id/galaxy_at_war
///
/// Route for retrieving the galaxy at war data for a provided player
/// matches the provided `id`
///
/// `player_id` The ID of the player to get the GAW data for
pub async fn get_player_gaw(
    _: AdminAuth,
    Path(player_id): Path<PlayerID>,
    Extension(db): Extension<DatabaseConnection>,
) -> PlayersRes<GalaxyAtWar> {
    let player = find_player(&db, player_id).await?;
    let galax_at_war = GalaxyAtWar::get(&db, player.id).await?;
    Ok(Json(galax_at_war))
}

/// IntoResponse implementation for PlayersError to allow it to be
/// used within the result type as a error response
impl IntoResponse for PlayersError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::DataNotFound => StatusCode::NOT_FOUND,
            Self::PlayerNotFound => StatusCode::NOT_FOUND,
            Self::EmailTaken | Self::InvalidEmail => StatusCode::BAD_REQUEST,
            Self::InvalidPassword | Self::InvalidPermission => StatusCode::UNAUTHORIZED,
            Self::Database(_) | Self::PasswordHash(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}
