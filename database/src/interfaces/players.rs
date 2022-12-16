use crate::{
    dto::{
        players::{PlayerDataUpdate, PlayerUpdate},
        ParsedUpdate,
    },
    entities::{player_characters, player_classes, players},
    DbResult, GalaxyAtWar, Player, PlayerCharacter, PlayerClass,
};
use rand_core::{OsRng, RngCore};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, CursorTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait,
    QueryFilter,
};
use std::iter::Iterator;
use tokio::try_join;

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
        update: PlayerUpdate,
    ) -> DbResult<Self> {
        let mut active = self.into_active_model();
        if let Some(email) = update.email {
            active.email = Set(email);
        }

        if let Some(display_name) = update.display_name {
            active.display_name = Set(display_name);
        }

        if let Some(origin) = update.origin {
            active.origin = Set(origin);
        }

        if let Some(password) = update.password {
            active.password = Set(password);
        }

        if let Some(credits) = update.credits {
            active.credits = Set(credits);
        }

        if let Some(inventory) = update.inventory {
            active.inventory = Set(inventory);
        }

        if let Some(csreward) = update.csreward {
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
    pub async fn by_id(db: &DatabaseConnection, id: u32) -> DbResult<Option<Self>> {
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
                let token = Self::generate_random_string(Self::TOKEN_LENGTH);
                let out = self.set_token(db, token).await?;
                return Ok(out);
            }
            Some(value) => value.clone(),
        };
        Ok((self, token))
    }

    fn generate_random_string(len: usize) -> String {
        const RANGE: u32 = 26 + 26 + 10;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                    abcdefghijklmnopqrstuvwxyz\
                    0123456789";

        let mut rand = OsRng;
        let mut output = String::with_capacity(len);

        // Loop until the string length is finished
        for _ in 0..len {
            // Loop until a valid random is found
            loop {
                let var = rand.next_u32() >> (32 - 6);
                if var < RANGE {
                    output.push(char::from(CHARSET[var as usize]));
                    break;
                }
            }
        }

        output
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

    fn apply_update(model: &mut players::ActiveModel, update: PlayerDataUpdate) {
        match update {
            PlayerDataUpdate::Base(base) => {
                model.credits = Set(base.credits);
                model.credits_spent = Set(base.credits_spent);
                model.games_played = Set(base.games_played);
                model.seconds_played = Set(base.seconds_played);
                model.inventory = Set(base.inventory);
            }
            PlayerDataUpdate::FaceCodes(value) => model.face_codes = Set(Some(value)),
            PlayerDataUpdate::NewItem(value) => model.new_item = Set(Some(value)),
            PlayerDataUpdate::ChallengeReward(value) => model.csreward = Set(value),
            PlayerDataUpdate::Completion(value) => model.completion = Set(Some(value)),
            PlayerDataUpdate::Progress(value) => model.progress = Set(Some(value)),
            PlayerDataUpdate::Cscompletion(value) => model.cs_completion = Set(Some(value)),
            PlayerDataUpdate::Cstimestamps(value) => model.cs_timestamps1 = Set(Some(value)),
            PlayerDataUpdate::Cstimestamps2(value) => model.cs_timestamps2 = Set(Some(value)),
            PlayerDataUpdate::Cstimestamps3(value) => model.cs_timestamps3 = Set(Some(value)),
        }
    }

    pub async fn update(self, db: &DatabaseConnection, update: PlayerDataUpdate) -> DbResult<Self> {
        let mut model = self.into_active_model();
        Self::apply_update(&mut model, update);
        model.update(db).await
    }

    pub async fn update_all(
        self,
        db: &DatabaseConnection,
        updates: Vec<ParsedUpdate>,
    ) -> DbResult<players::Model> {
        let mut data_updates = Vec::new();
        for update in updates {
            match update {
                ParsedUpdate::Character(index, value) => {
                    PlayerCharacter::update(db, &self, index, value).await?;
                }
                ParsedUpdate::Class(index, value) => {
                    PlayerClass::update(db, &self, index, value).await?;
                }
                ParsedUpdate::Data(value) => data_updates.push(value),
            }
        }

        if !data_updates.is_empty() {
            let mut model = self.into_active_model();
            for update in data_updates {
                Self::apply_update(&mut model, update);
            }
            let model = model.update(db).await?;
            return Ok(model);
        }

        Ok(self)
    }
}
