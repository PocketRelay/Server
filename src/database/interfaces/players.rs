use crate::database::{
    entities::{player_data, players, PlayerData},
    DbResult, Player, PlayerRole,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, DeleteResult, EntityTrait, InsertResult, IntoActiveModel,
    ModelTrait, QueryFilter,
};
use std::{future::Future, iter::Iterator, pin::Pin};

type DbFuture<'a, T> = Pin<Box<dyn Future<Output = DbResult<T>> + Send + 'a>>;

impl Player {
    /// Creates a new player with the proivded details and inserts
    /// it into the database
    ///
    /// `db`           The database instance
    /// `email`        The player account email
    /// `display_name` The player display name
    /// `password`     The hashed player password
    /// `origin`       Whether the account is an origin account
    pub fn create(
        db: &DatabaseConnection,
        email: String,
        display_name: String,
        password: Option<String>,
    ) -> DbFuture<Self> {
        let active_model = players::ActiveModel {
            email: Set(email),
            display_name: Set(display_name),
            password: Set(password),
            ..Default::default()
        };
        active_model.insert(db)
    }

    /// Deletes the provided player
    ///
    /// `db` The database connection
    pub fn delete(self, db: &DatabaseConnection) -> DbFuture<DeleteResult> {
        // Delete player itself
        let model = self.into_active_model();
        model.delete(db)
    }

    /// Retrieves all the player data for the desired player
    ///
    /// `id`    The ID of the player
    /// `db`    The database connection
    pub fn all_data(
        id: u32,
        db: &DatabaseConnection,
    ) -> impl Future<Output = DbResult<Vec<PlayerData>>> + Send + '_ {
        player_data::Entity::find()
            .filter(player_data::Column::PlayerId.eq(id))
            .all(db)
    }

    /// Sets the key value data for the provided player. If the data exists then
    /// the value is updated otherwise the data will be created. The new data is
    /// returned.
    ///
    /// `id`    The ID of the player
    /// `db`    The database connection
    /// `key`   The data key
    /// `value` The data value
    pub async fn set_data(
        id: u32,
        db: &DatabaseConnection,
        key: String,
        value: String,
    ) -> DbResult<PlayerData> {
        let existing = player_data::Entity::find()
            .filter(
                player_data::Column::PlayerId
                    .eq(id)
                    .and(player_data::Column::Key.eq(&key as &str)),
            )
            .one(db)
            .await?;

        if let Some(player_data) = existing {
            let mut model = player_data.into_active_model();
            model.key = Set(key);
            model.value = Set(value);
            model.update(db).await
        } else {
            player_data::ActiveModel {
                player_id: Set(id),
                key: Set(key),
                value: Set(value),
                ..Default::default()
            }
            .insert(db)
            .await
        }
    }

    /// Bulk inserts a collection of player data for the provided player. Will not handle
    /// conflicts so this should only be done on a freshly create player where data doesnt
    /// already exist
    ///
    /// `db`   The database connection
    /// `data` Iterator of the data keys and values
    pub fn bulk_insert_data<'a>(
        &self,
        db: &'a DatabaseConnection,
        data: impl Iterator<Item = (String, String)>,
    ) -> impl Future<Output = DbResult<InsertResult<player_data::ActiveModel>>> + Send + 'a {
        // Transform the provided key values into active models
        let models_iter = data.map(|(key, value)| player_data::ActiveModel {
            id: NotSet,
            player_id: Set(self.id),
            key: Set(key),
            value: Set(value),
        });
        // Insert all the models
        player_data::Entity::insert_many(models_iter).exec(db)
    }

    /// Deletes the player data with the provided key for the
    /// current player
    ///
    /// `db`    The database connection
    /// `key`   The data key
    pub fn delete_data<'a>(
        &self,
        db: &'a DatabaseConnection,
        key: &str,
    ) -> impl Future<Output = DbResult<DeleteResult>> + Send + 'a {
        player_data::Entity::delete_many()
            .belongs_to(self)
            .filter(player_data::Column::Key.eq(key))
            .exec(db)
    }

    /// Gets the player data with the provided key for the
    /// current player
    ///
    /// `db`  The database connection
    /// `key` The data key
    pub fn get_data<'a>(
        &self,
        db: &'a DatabaseConnection,
        key: &str,
    ) -> impl Future<Output = DbResult<Option<PlayerData>>> + Send + 'a {
        self.find_related(player_data::Entity)
            .filter(player_data::Column::Key.eq(key))
            .one(db)
    }

    /// Gets all the player class data for the current player
    ///
    /// `db` The database connection
    pub fn get_classes<'a>(
        &self,
        db: &'a DatabaseConnection,
    ) -> impl Future<Output = DbResult<Vec<PlayerData>>> + Send + 'a {
        self.find_related(player_data::Entity)
            .filter(player_data::Column::Key.starts_with("class"))
            .all(db)
    }

    /// Parses the challenge points value which is the second
    /// item in the completion list.
    ///
    /// `db` The database connection
    pub async fn get_challenge_points(&self, db: &DatabaseConnection) -> Option<u32> {
        let list = self.get_data(db, "Completion").await.ok()??.value;
        let part = list.split(',').nth(1)?;
        let value: u32 = part.parse().ok()?;
        Some(value)
    }

    /// Attempts to find a player with the provided ID will return none
    /// if there was no players with that ID
    ///
    /// `db` The database instance
    /// `id` The ID of the player to find
    pub fn by_id(
        db: &DatabaseConnection,
        id: u32,
    ) -> impl Future<Output = DbResult<Option<Player>>> + Send + '_ {
        players::Entity::find_by_id(id).one(db)
    }

    /// Attempts to find a player with the provided email.
    ///
    /// `db`    The database connection
    /// `email` The email address to search for
    pub fn by_email<'a>(
        db: &'a DatabaseConnection,
        email: &str,
    ) -> impl Future<Output = DbResult<Option<Player>>> + Send + 'a {
        players::Entity::find()
            .filter(players::Column::Email.eq(email))
            .one(db)
    }

    /// Determines whether the current player has permission to
    /// make actions on behalf of the other player. This can
    /// occur when they are both the same player or the role of
    /// self is greater than the other role
    ///
    /// `other` The player to check for permission over
    pub fn has_permission_over(&self, other: &Self) -> bool {
        self.id == other.id || self.role > other.role
    }

    /// Updates the password for the provided player returning
    /// a future resolving to the new player with its updated
    /// password value
    ///
    /// `db`       The database connection
    /// `password` The new hashed password
    pub fn set_password(self, db: &DatabaseConnection, password: String) -> DbFuture<'_, Player> {
        let mut model = self.into_active_model();
        model.password = Set(Some(password));
        model.update(db)
    }

    /// Sets the role of the provided player
    ///
    /// `db`   The database connection
    /// `role` The new role for the player
    pub fn set_role(self, db: &DatabaseConnection, role: PlayerRole) -> DbFuture<'_, Player> {
        let mut model = self.into_active_model();
        model.role = Set(role);
        model.update(db)
    }

    /// Updates the basic details of the provided player if
    /// they are provided
    ///
    /// `db`       The database connection
    /// `username` Optional new username for the player
    /// `email`    Optional new email for the player
    pub fn set_details(
        self,
        db: &DatabaseConnection,
        username: Option<String>,
        email: Option<String>,
    ) -> DbFuture<'_, Player> {
        let mut model = self.into_active_model();

        if let Some(username) = username {
            model.display_name = Set(username);
        }

        if let Some(email) = email {
            model.email = Set(email)
        }

        model.update(db)
    }
}
