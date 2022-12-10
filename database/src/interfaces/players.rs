use crate::{
    entities::{player_characters, player_classes, players},
    DbResult, GalaxyAtWar, Player, PlayerCharacter, PlayerClass,
};
use log::warn;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, CursorTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait,
    QueryFilter,
};
use std::iter::Iterator;
use tokio::try_join;
use utils::{parse::MEStringParser, types::PlayerID};

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

        Ok(if values.len() == (count + 1) as usize {
            // Pop the value being used to determine the leftover size
            values.pop();
            (values, true)
        } else {
            (values, false)
        })
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
            id: NotSet,
            email: Set(email.to_string()),
            display_name: Set(display_name),
            session_token: NotSet,
            origin: Set(origin),
            password: Set(password),
            credits: NotSet,
            credits_spent: NotSet,
            games_played: NotSet,
            seconds_played: NotSet,
            inventory: Set(String::new()),
            csreward: NotSet,
            face_codes: NotSet,
            new_item: NotSet,
            completion: NotSet,
            progress: NotSet,
            cs_completion: NotSet,
            cs_timestamps1: NotSet,
            cs_timestamps2: NotSet,
            cs_timestamps3: NotSet,
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

    /// Updates the player using the optional values provided from the HTTP
    /// API
    ///
    /// `db`           The database connection
    /// `email`        The optional email to use
    /// `display_name` The optional display name to use
    /// `origin`       The optional origin value to use
    /// `password`     The optional password to use
    /// `credits`      The optional credits to use
    /// `inventory`    The optional inventory to use
    /// `csreward`     The optional csreward to use
    pub async fn update_http(
        self,
        db: &DatabaseConnection,
        email: Option<String>,
        display_name: Option<String>,
        origin: Option<bool>,
        password: Option<String>,
        credits: Option<u32>,
        inventory: Option<String>,
        csreward: Option<u16>,
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

        if let Some(credits) = credits {
            active.credits = Set(credits);
        }

        if let Some(inventory) = inventory {
            active.inventory = Set(inventory);
        }

        if let Some(csreward) = csreward {
            active.csreward = Set(csreward);
        }

        active.update(db).await
    }

    /// Parses the challenge points value which is the second
    /// item in the completion list.
    pub fn get_challenge_points(&self) -> Option<u32> {
        let list = self.completion.as_ref()?;
        let part = list.split(',').nth(1)?;
        let value: u32 = part.parse().ok()?;
        Some(value)
    }

    /// Attempts to find a player with the provided ID will return none
    /// if there was no players with that ID
    ///
    /// `db` The database instance
    /// `id` The ID of the player to find
    pub async fn by_id(db: &DatabaseConnection, id: PlayerID) -> DbResult<Option<Self>> {
        players::Entity::find_by_id(id).one(db).await
    }

    /// Collects all the related classes, characters and galaxy at war
    /// data all at once rturning the loaded result if no errors
    /// occurred.
    ///
    /// `db` The database connection
    pub async fn collect_relations(
        &self,
        db: &DatabaseConnection,
    ) -> DbResult<(Vec<PlayerClass>, Vec<PlayerCharacter>, GalaxyAtWar)> {
        let classes = self.find_related(player_classes::Entity).all(db);
        let characters = self.find_related(player_characters::Entity).all(db);
        let galaxy_at_war = GalaxyAtWar::find_or_create(db, self, 0.0);

        try_join!(classes, characters, galaxy_at_war)
    }

    /// Collects all the related classes, characters all at once rturning
    /// the loaded result if no errors occurred.
    ///
    /// `db` The database connection
    pub async fn collect_relations_partial(
        &self,
        db: &DatabaseConnection,
    ) -> DbResult<(Vec<PlayerClass>, Vec<PlayerCharacter>)> {
        let classes = self.find_related(player_classes::Entity).all(db);
        let characters = self.find_related(player_characters::Entity).all(db);
        try_join!(classes, characters)
    }

    /// Attempts to find a player with the provided ID and matching session
    /// token will return none if there was no players with that ID
    ///
    /// `db` The database instance
    /// `id` The ID of the player to find
    pub async fn by_id_with_token(
        db: &DatabaseConnection,
        id: u32,
        token: String,
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
    pub async fn with_token(self, db: &DatabaseConnection) -> DbResult<(Self, String)> {
        let token = match &self.session_token {
            None => {
                let token = utils::random::generate_random_string(Self::TOKEN_LENGTH);
                let out = self.set_token(db, token).await?;
                return Ok(out);
            }
            Some(value) => value.clone(),
        };
        Ok((self, token))
    }

    pub fn encode_base(&self) -> String {
        format!(
            "20;4;{};-1;0;{};0;{};{};0;{}",
            self.credits,
            self.credits_spent,
            self.games_played,
            self.seconds_played,
            &self.inventory
        )
    }

    /// Attempts to parse the provided player base data string and update the fields
    /// on the provided active player model. Will return a None option if parsing
    /// failed.
    ///
    /// # Format
    /// ```
    /// 20;4;21474;-1;0;0;0;50;180000;0;fff....(LARGE SEQUENCE OF INVENTORY CHARS)
    /// 20;4;CREDITS;UNKNOWN;UKNOWN;CREDITS_SPENT;UKNOWN;GAMES_PLAYED;SECONDS_PLAYED;UKNOWN;INVENTORY
    /// ```
    fn parse_base(model: &mut players::ActiveModel, value: &str) -> Option<()> {
        let mut parser = MEStringParser::new(value)?;
        model.credits = Set(parser.parse_next()?);
        parser.skip(2); // Skip -1;0
        model.credits_spent = Set(parser.parse_next()?);
        parser.skip(1)?;
        model.games_played = Set(parser.parse_next()?);
        model.seconds_played = Set(parser.parse_next()?);
        parser.skip(1);
        model.inventory = Set(parser.next_str()?);
        Some(())
    }

    fn modify(model: &mut players::ActiveModel, key: &str, value: String) {
        match key {
            "Base" => {
                if Self::parse_base(model, &value).is_none() {
                    warn!("Failed to completely parse player base")
                };
            }
            "FaceCodes" => model.face_codes = Set(Some(value)),
            "NewItem" => model.new_item = Set(Some(value)),
            "csreward" => {
                let value = value.parse::<u16>().unwrap_or(0);
                model.csreward = Set(value)
            }
            "Completion" => model.completion = Set(Some(value)),
            "Progress" => model.progress = Set(Some(value)),
            "cscompletion" => model.cs_completion = Set(Some(value)),
            "cstimestamps" => model.cs_timestamps1 = Set(Some(value)),
            "cstimestamps2" => model.cs_timestamps2 = Set(Some(value)),
            "cstimestamps3" => model.cs_timestamps3 = Set(Some(value)),
            _ => {}
        }
    }

    pub async fn update(self, db: &DatabaseConnection, key: &str, value: String) -> DbResult<Self> {
        let mut model = self.into_active_model();
        Self::modify(&mut model, key, value);
        let player = model.update(db).await?;
        Ok(player)
    }

    pub async fn update_all(
        self,
        db: &DatabaseConnection,
        values: impl Iterator<Item = (String, String)>,
    ) -> DbResult<players::Model> {
        let mut others = Vec::new();
        for (key, value) in values {
            if key.starts_with("class") {
                PlayerClass::update(db, &self, &key, &value).await.ok();
            } else if key.starts_with("char") {
                PlayerCharacter::update(db, &self, &key, &value).await.ok();
            } else {
                others.push((key, value));
            }
        }
        if !others.is_empty() {
            let mut model = self.into_active_model();
            for (key, value) in others {
                Self::modify(&mut model, &key, value);
            }
            let model = model.update(db).await?;
            Ok(model)
        } else {
            Ok(self)
        }
    }
}
