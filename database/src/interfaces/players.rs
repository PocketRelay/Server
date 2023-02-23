use crate::{
    entities::{player_data, players, PlayerData},
    DbResult, Player,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, CursorTrait, DatabaseConnection, DeleteResult, EntityTrait, IntoActiveModel,
    ModelTrait, QueryFilter,
};
use std::{future::Future, iter::Iterator};

impl Player {
    /// Takes all the player models using a cursor starting at the offset row
    /// and finding the count number of values will check the count + 1 rows
    /// in order to determine if there are more entires to come.
    ///
    /// `db`     The database connection
    /// `offset` The number of rows to skip
    /// `count`  The number of rows to collect
    pub async fn all(
        db: &DatabaseConnection,
        offset: u64,
        count: u64,
    ) -> DbResult<(Vec<Self>, bool)> {
        let mut values = players::Entity::find()
            .cursor_by(players::Column::Id)
            .after(offset)
            .first(count + 1)
            .all(db)
            .await?;
        let is_more = values.len() == (count + 1) as usize;
        if is_more {
            // Pop the value being used to determine the leftover size
            values.pop();
        }
        Ok((values, is_more))
    }

    /// Creates a new player with the proivded details and inserts
    /// it into the database
    ///
    /// `db`           The database instance
    /// `email`        The player account email
    /// `display_name` The player display name
    /// `password`     The hashed player password
    /// `origin`       Whether the account is an origin account
    pub async fn create(
        db: &DatabaseConnection,
        email: String,
        display_name: String,
        password: String,
        origin: bool,
    ) -> DbResult<Self> {
        let active_model = players::ActiveModel {
            email: Set(email),
            display_name: Set(display_name),
            origin: Set(origin),
            password: Set(password),
            ..Default::default()
        };
        active_model.insert(db).await
    }

    /// Deletes the provided player
    ///
    /// `db` The database connection
    pub async fn delete(self, db: &DatabaseConnection) -> DbResult<DeleteResult> {
        // Delete player itself
        let model = self.into_active_model();
        model.delete(db).await
    }

    /// Retrieves all the player data for this player
    pub async fn all_data(id: u32, db: &DatabaseConnection) -> DbResult<Vec<PlayerData>> {
        player_data::Entity::find()
            .filter(player_data::Column::PlayerId.eq(id))
            .all(db)
            .await
    }

    /// Sets the key value data for the provided player. If the data exists then
    /// the value is updated otherwise the data will be created. The new data is
    /// returned.
    ///
    /// `db`    The database connection
    /// `key`   The data key
    /// `value` The data value
    pub async fn set_data(
        id: u32,
        db: &DatabaseConnection,
        key: String,
        value: String,
    ) -> DbResult<PlayerData> {
        match player_data::Entity::find()
            .filter(
                player_data::Column::PlayerId
                    .eq(id)
                    .and(player_data::Column::Key.eq(&key as &str)),
            )
            .one(db)
            .await?
        {
            Some(player_data) => {
                let mut model = player_data.into_active_model();
                model.key = Set(key);
                model.value = Set(value);
                model.update(db).await
            }
            None => {
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
    }

    /// Bulk inserts a collection of player data for the provided player. Will not handle
    /// conflicts so this should only be done on a freshly create player where data doesnt
    /// already exist
    ///
    /// `db`   The database connection
    /// `data` Iterator of the data keys and values
    pub async fn bulk_insert_data(
        &self,
        db: &DatabaseConnection,
        data: impl Iterator<Item = (String, String)>,
    ) -> DbResult<()> {
        // Transform the provided key values into active models
        let models_iter = data.map(|(key, value)| player_data::ActiveModel {
            id: NotSet,
            player_id: Set(self.id),
            key: Set(key),
            value: Set(value),
        });
        // Insert all the models
        player_data::Entity::insert_many(models_iter)
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn delete_data(&self, db: &DatabaseConnection, key: &str) -> DbResult<()> {
        let data = self
            .find_related(player_data::Entity)
            .filter(player_data::Column::Key.eq(key))
            .one(db)
            .await?;
        if let Some(data) = data {
            data.delete(db).await?;
        }
        Ok(())
    }

    pub async fn get_data(
        &self,
        db: &DatabaseConnection,
        key: &str,
    ) -> DbResult<Option<PlayerData>> {
        self.find_related(player_data::Entity)
            .filter(player_data::Column::Key.eq(key))
            .one(db)
            .await
    }

    pub async fn get_classes(&self, db: &DatabaseConnection) -> DbResult<Vec<PlayerData>> {
        self.find_related(player_data::Entity)
            .filter(player_data::Column::Key.starts_with("class"))
            .all(db)
            .await
    }

    pub async fn get_characters(&self, db: &DatabaseConnection) -> DbResult<Vec<PlayerData>> {
        self.find_related(player_data::Entity)
            .filter(player_data::Column::Key.starts_with("char"))
            .all(db)
            .await
    }

    /// Updates the player using the optional values provided from the HTTP
    /// API
    ///
    /// `db`           The database connection
    /// `email`        The optional email to use
    /// `display_name` The optional display name to use
    /// `origin`       The optional origin value to use
    /// `password`     The optional password to use
    pub async fn update_http(
        self,
        db: &DatabaseConnection,
        email: Option<String>,
        display_name: Option<String>,
        origin: Option<bool>,
        password: Option<String>,
    ) -> DbResult<Self> {
        let mut active = self.into_active_model();
        if let Some(email) = email {
            active.email = Set(email);
        }

        if let Some(display_name) = display_name {
            active.display_name = Set(display_name);
        }

        if let Some(origin) = origin {
            active.origin = Set(origin);
        }

        if let Some(password) = password {
            active.password = Set(password);
        }

        active.update(db).await
    }

    /// Parses the challenge points value which is the second
    /// item in the completion list.
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
    #[inline]
    pub async fn by_id(db: &DatabaseConnection, id: u32) -> DbResult<Option<Self>> {
        players::Entity::find_by_id(id).one(db).await
    }

    /// Attempts to find a player with the provided email. Conditional
    /// check for whether to allow origin accounts in the search.
    ///
    /// `email`  The email address to search for
    /// `origin` Whether to check for origin accounts or normal accounts
    pub async fn by_email(
        db: &DatabaseConnection,
        email: &str,
        origin: bool,
    ) -> DbResult<Option<Self>> {
        players::Entity::find()
            .filter(
                players::Column::Email
                    .eq(email)
                    .and(players::Column::Origin.eq(origin)),
            )
            .one(db)
            .await
    }

    /// Checks whether the provided email address is taken by any
    /// accounts in the database including origin accounts.
    ///
    /// `db`    The datbase instance
    /// `email` The email to check for
    pub async fn is_email_taken(db: &DatabaseConnection, email: &str) -> DbResult<bool> {
        players::Entity::find()
            .filter(players::Column::Email.eq(email))
            .one(db)
            .await
            .map(|value| value.is_some())
    }

    /// Updates the password for the provided player returning
    /// a future resolving to the new player with its updated
    /// password value
    ///
    /// `db`       The database connection to use
    /// `password` The new hashed password
    pub fn set_password<'a>(
        self,
        db: &'a DatabaseConnection,
        password: String,
    ) -> impl Future<Output = DbResult<Player>> + 'a {
        let mut model = self.into_active_model();
        model.password = Set(password);
        model.update(db)
    }
}
