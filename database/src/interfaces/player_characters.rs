use crate::{
    entities::{player_characters, players},
    DbResult, PlayerCharacter,
};
use log::warn;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter,
};
use utils::{parse::MEStringParser, types::PlayerID};

impl PlayerCharacter {
    /// Finds all the player characters for the provided player model
    ///
    /// `db`     The database instance
    /// `player` The player to find the characters for
    pub async fn find_all(db: &DatabaseConnection, player: &players::Model) -> DbResult<Vec<Self>> {
        player.find_related(player_characters::Entity).all(db).await
    }

    /// Finds all the player classes for the player with the provided ID
    ///
    /// `db`        The databse connection
    /// `player_id` The player ID to find classes for
    pub async fn find_by_pid(db: &DatabaseConnection, player_id: PlayerID) -> DbResult<Vec<Self>> {
        player_characters::Entity::find()
            .filter(player_characters::Column::PlayerId.eq(player_id))
            .all(db)
            .await
    }

    /// Attempts to find a player character relating to the provided player in the database
    /// using its index and relation to the player. If None could be found a new value
    /// will be created and returned instead.
    ///
    /// `db`     The database instance
    /// `player` The player to find the class for
    /// `index`  The index of the player class to find
    async fn find(
        db: &DatabaseConnection,
        player: &players::Model,
        index: u16,
    ) -> DbResult<player_characters::ActiveModel> {
        let player_character = player
            .find_related(player_characters::Entity)
            .filter(player_characters::Column::Index.eq(index))
            .one(db)
            .await?;

        if let Some(player_character) = player_character {
            return Ok(player_character.into_active_model());
        }

        Ok(player_characters::ActiveModel {
            id: NotSet,
            player_id: Set(player.id),
            index: Set(index),
            kit_name: NotSet,
            name: NotSet,
            tint1: NotSet,
            tint2: NotSet,
            pattern: NotSet,
            pattern_color: NotSet,
            phong: NotSet,
            emissive: NotSet,
            skin_tone: NotSet,
            seconds_played: NotSet,
            timestamp_year: NotSet,
            timestamp_month: NotSet,
            timestamp_day: NotSet,
            timestamp_seconds: NotSet,
            powers: NotSet,
            hotkeys: NotSet,
            weapons: NotSet,
            weapon_mods: NotSet,
            deployed: NotSet,
            leveled_up: NotSet,
        })
    }

    /// Attempts to parse the provided player character data string and update the fields
    /// on the provided active player character model. Will return a None option if parsing
    /// failed.
    fn parse(model: &mut player_characters::ActiveModel, value: &str) -> Option<()> {
        let mut parser = MEStringParser::new(value)?;
        model.kit_name = Set(parser.next_str()?);
        model.name = Set(parser.next()?);
        model.tint1 = Set(parser.next()?);
        model.tint2 = Set(parser.next()?);
        model.pattern = Set(parser.next()?);
        model.pattern_color = Set(parser.next()?);
        model.phong = Set(parser.next()?);
        model.emissive = Set(parser.next()?);
        model.skin_tone = Set(parser.next()?);
        model.seconds_played = Set(parser.next()?);
        model.timestamp_year = Set(parser.next()?);
        model.timestamp_month = Set(parser.next()?);
        model.timestamp_day = Set(parser.next()?);
        model.timestamp_seconds = Set(parser.next()?);
        model.powers = Set(parser.next_str()?);
        model.hotkeys = Set(parser.next_str()?);
        model.weapons = Set(parser.next_str()?);
        model.weapon_mods = Set(parser.next_str()?);
        model.deployed = Set(parser.next_bool()?);
        model.leveled_up = Set(parser.next_bool()?);
        Some(())
    }

    /// Attempts to parse the character index from the provided
    /// character key. If the key is too short or doesn't contain
    /// an index then an error is returned
    fn parse_index(key: &str) -> Result<u16, PlayerCharactersError> {
        if key.len() <= 4 {
            return Err(PlayerCharactersError::InvalidKey);
        }
        match key[4..].parse() {
            Ok(value) => Ok(value),
            Err(_) => Err(PlayerCharactersError::InvalidIndex),
        }
    }

    /// Updates the provided character for the provided player
    /// by parsing the key and values
    ///
    /// `db`     The database instance
    /// `player` The player to update the character for
    /// `key`    The key to determine which character to update
    /// `value`  The value to use for updating the character
    pub async fn update(
        db: &DatabaseConnection,
        player: &players::Model,
        key: &str,
        value: &str,
    ) -> Result<(), PlayerCharactersError> {
        let index = Self::parse_index(key)?;
        let mut model = Self::find(db, player, index).await?;
        if let None = Self::parse(&mut model, value) {
            warn!("Failed to fully parse player character: {key} = {value}");
        }
        model.save(db).await?;
        Ok(())
    }

    /// Encodes the provided player character model into the ME string
    /// encoded value to send as apart of the settings map
    pub fn encode(&self) -> String {
        format!(
            "20;4;{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
            &self.kit_name,
            &self.name,
            self.tint1,
            self.tint2,
            self.pattern,
            self.pattern_color,
            self.phong,
            self.emissive,
            self.skin_tone,
            self.seconds_played,
            self.timestamp_year,
            self.timestamp_month,
            self.timestamp_day,
            self.timestamp_seconds,
            self.powers,
            self.hotkeys,
            self.weapons,
            self.weapon_mods,
            if self.deployed { "True" } else { "False" },
            if self.leveled_up { "True" } else { "False" },
        )
    }
}

#[derive(Debug)]
pub enum PlayerCharactersError {
    InvalidKey,
    InvalidIndex,
    Database(DbErr),
}

impl From<DbErr> for PlayerCharactersError {
    fn from(err: DbErr) -> Self {
        Self::Database(err)
    }
}
