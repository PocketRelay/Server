use crate::{
    entities::{player_data, players, PlayerData},
    DbResult, Player,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, CursorTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, QueryFilter,
};
use std::iter::Iterator;

impl Player {
    /// The length of player session tokens
    const TOKEN_LENGTH: usize = 128;

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

    /// Deletes the current player
    ///
    /// `db` The database connection
    pub async fn delete(self, db: &DatabaseConnection) -> DbResult<()> {
        let model = self.into_active_model();
        model.delete(db).await?;
        Ok(())
    }

    /// Retrieves all the player data for this player
    pub async fn all_data(&self, db: &DatabaseConnection) -> DbResult<Vec<PlayerData>> {
        self.find_related(player_data::Entity).all(db).await
    }

    pub async fn set_data(
        &self,
        db: &DatabaseConnection,
        key: String,
        value: String,
    ) -> DbResult<PlayerData> {
        Self::set_data_impl(self.id, db, key, value).await
    }

    pub async fn set_data_impl(
        player_id: u32,
        db: &DatabaseConnection,
        key: String,
        value: String,
    ) -> DbResult<PlayerData> {
        match player_data::Entity::find()
            .filter(
                player_data::Column::PlayerId
                    .eq(player_id)
                    .and(player_data::Column::Key.eq(key.clone())),
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
                    player_id: Set(player_id),
                    key: Set(key),
                    value: Set(value),
                    ..Default::default()
                }
                .insert(db)
                .await
            }
        }
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
    pub async fn by_id(db: &DatabaseConnection, id: u32) -> DbResult<Option<Self>> {
        players::Entity::find_by_id(id).one(db).await
    }

    /// Attempts to find a player with the provided ID and matching session
    /// token will return none if there was no players with that ID
    ///
    /// `db` The database instance
    /// `id` The ID of the player to find
    pub async fn by_id_with_token(
        db: &DatabaseConnection,
        id: u32,
        token: &str,
    ) -> DbResult<Option<Self>> {
        players::Entity::find_by_id(id)
            .filter(players::Column::SessionToken.eq(token))
            .one(db)
            .await
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

    /// Attempts to find a player by the provided session token
    ///
    /// `db`    The database instance
    /// `token` The session token to search for
    pub async fn by_token(db: &DatabaseConnection, token: &str) -> DbResult<Option<Self>> {
        players::Entity::find()
            .filter(players::Column::SessionToken.eq(token))
            .one(db)
            .await
    }

    /// Sets the token for the provided player returning both
    /// the player model and token that was set.
    ///
    /// `db`     The database instance
    /// `player` The player to set the token for
    /// `token`  The token to set
    async fn set_token(self, db: &DatabaseConnection, token: String) -> DbResult<(Self, String)> {
        let mut player = self.into_active_model();
        player.session_token = Set(Some(token.clone()));
        let player = player.update(db).await?;
        Ok((player, token))
    }

    /// Attempts to get the existing session token for the provided
    /// player or creates a new session token if there is not already
    /// one will return both the player model and session token
    ///
    /// `db`     The database instance
    /// `player` The player to get the token for
    /// `gen_fn` Function for generating a new token if there is not one
    pub async fn with_token(
        self,
        db: &DatabaseConnection,
        gen_fn: fn(usize) -> String,
    ) -> DbResult<(Self, String)> {
        let token = match &self.session_token {
            None => {
                let token = gen_fn(Self::TOKEN_LENGTH);
                let out = self.set_token(db, token).await?;
                return Ok(out);
            }
            Some(value) => value.clone(),
        };
        Ok((self, token))
    }
}
