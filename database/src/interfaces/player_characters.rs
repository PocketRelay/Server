use crate::{
    dto::player_characters::PlayerCharacterUpdate,
    entities::{player_characters, players},
    DbResult, PlayerCharacter,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, QueryFilter,
};

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
    pub async fn find_by_pid(db: &DatabaseConnection, player_id: u32) -> DbResult<Vec<Self>> {
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
            player_id: Set(player.id),
            index: Set(index),
            ..Default::default()
        })
    }

    /// Attempts to parse the provided player character data string and update the fields
    /// on the provided active player character model. Will return a None option if parsing
    /// failed.
    fn apply_update(model: &mut player_characters::ActiveModel, update: PlayerCharacterUpdate) {
        model.kit_name = Set(update.kit_name);
        model.name = Set(update.name);
        model.tint1 = Set(update.tint1);
        model.tint2 = Set(update.tint2);
        model.pattern = Set(update.pattern);
        model.pattern_color = Set(update.pattern_color);
        model.phong = Set(update.phong);
        model.emissive = Set(update.emissive);
        model.skin_tone = Set(update.skin_tone);
        model.seconds_played = Set(update.seconds_played);
        model.timestamp_year = Set(update.timestamp_year);
        model.timestamp_month = Set(update.timestamp_month);
        model.timestamp_day = Set(update.timestamp_day);
        model.timestamp_seconds = Set(update.timestamp_seconds);
        model.powers = Set(update.powers);
        model.hotkeys = Set(update.hotkeys);
        model.weapons = Set(update.weapons);
        model.weapon_mods = Set(update.weapon_mods);
        model.deployed = Set(update.deployed);
        model.leveled_up = Set(update.leveled_up);
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
        index: u16,
        update: PlayerCharacterUpdate,
    ) -> DbResult<()> {
        let mut model = Self::find(db, player, index).await?;
        Self::apply_update(&mut model, update);
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
