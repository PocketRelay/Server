use log::warn;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
};
use utils::parse::MEStringParser;

use crate::{entities::players, DbResult};

use super::{player_characters::PlayerCharactersInterface, player_classes::PlayerClassesInterface};
use std::iter::Iterator;

pub struct PlayersInterface;

impl PlayersInterface {
    /// The length of player session tokens
    const TOKEN_LENGTH: usize = 128;

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
    ) -> DbResult<players::Model> {
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

    /// Attempts to find a player with the provided ID will return none
    /// if there was no players with that ID
    ///
    /// `db` The database instance
    /// `id` The ID of the player to find
    pub async fn by_id(db: &DatabaseConnection, id: u32) -> DbResult<Option<players::Model>> {
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
        token: String,
    ) -> DbResult<Option<players::Model>> {
        players::Entity::find_by_id(id)
            .filter(players::Column::SessionToken.eq(token))
            .one(db)
            .await
    }

    /// Attempts to find a player with the provided email. Conditional
    /// check for whether to allow origin accounts in the search.
    pub async fn by_email(
        db: &DatabaseConnection,
        email: &str,
        origin: bool,
    ) -> DbResult<Option<players::Model>> {
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
    ///
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
    pub async fn by_token(
        db: &DatabaseConnection,
        token: &str,
    ) -> DbResult<Option<players::Model>> {
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
    pub async fn set_token(
        db: &DatabaseConnection,
        player: players::Model,
        token: String,
    ) -> DbResult<(players::Model, String)> {
        let mut player = player.into_active_model();
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
    pub async fn get_token(
        db: &DatabaseConnection,
        player: players::Model,
    ) -> DbResult<(players::Model, String)> {
        let token = match &player.session_token {
            None => {
                let token = utils::random::generate_random_string(Self::TOKEN_LENGTH);
                let out = Self::set_token(db, player, token).await?;
                return Ok(out);
            }
            Some(value) => value.clone(),
        };
        Ok((player, token))
    }

    pub fn encode_base(model: &players::Model) -> String {
        format!(
            "20;4;{};-1;0;{};0;{};{};0;{}",
            model.credits,
            model.credits_spent,
            model.games_played,
            model.seconds_played,
            model.inventory
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
        model.credits = Set(parser.next()?);
        parser.skip(2); // Skip -1;0
        model.credits_spent = Set(parser.next()?);
        parser.skip(1)?;
        model.games_played = Set(parser.next()?);
        model.seconds_played = Set(parser.next()?);
        parser.skip(1);
        model.inventory = Set(parser.next_str()?);
        Some(())
    }

    fn modify(model: &mut players::ActiveModel, key: &str, value: String) {
        match key {
            "Base" => {
                if let None = Self::parse_base(model, &value) {
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

    pub async fn update(
        db: &DatabaseConnection,
        player: players::Model,
        key: &str,
        value: String,
    ) -> DbResult<players::Model> {
        let mut model = player.into_active_model();
        Self::modify(&mut model, key, value);
        let player = model.update(db).await?;
        Ok(player)
    }

    pub async fn update_all(
        db: &DatabaseConnection,
        player: players::Model,
        values: impl Iterator<Item = (String, String)>,
    ) -> DbResult<players::Model> {
        let mut others = Vec::new();
        for (key, value) in values {
            if key.starts_with("class") {
                PlayerClassesInterface::update(db, &player, &key, &value)
                    .await
                    .ok();
            } else if key.starts_with("char") {
                PlayerCharactersInterface::update(db, &player, &key, &value)
                    .await
                    .ok();
            } else {
                others.push((key, value));
            }
        }
        if others.len() > 0 {
            let mut model = player.into_active_model();
            for (key, value) in others {
                Self::modify(&mut model, &key, value);
            }
            let model = model.update(db).await?;
            Ok(model)
        } else {
            Ok(player)
        }
    }
}
