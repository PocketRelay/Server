use log::warn;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, DbErr, IntoActiveModel, ModelTrait, QueryFilter,
};
use utils::parse::MEStringParser;

use crate::{
    entities::{player_classes, players},
    DbResult,
};

pub struct PlayerClassesInterface;

impl PlayerClassesInterface {
    /// Finds all the player classes for the provided player model
    ///
    /// `db`     The database instance
    /// `player` The player to find the classes for
    pub async fn find_all(
        db: &DatabaseConnection,
        player: &players::Model,
    ) -> DbResult<Vec<player_classes::Model>> {
        player.find_related(player_classes::Entity).all(db).await
    }

    /// Attempts to find a player class relating to the provided player in the database
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
    ) -> DbResult<player_classes::ActiveModel> {
        let player_class = player
            .find_related(player_classes::Entity)
            .filter(player_classes::Column::Index.eq(index))
            .one(db)
            .await?;
        if let Some(player_class) = player_class {
            return Ok(player_class.into_active_model());
        }
        Ok(player_classes::ActiveModel {
            id: NotSet,
            player_id: Set(player.id),
            index: Set(index),
            name: NotSet,
            level: NotSet,
            exp: NotSet,
            promotions: NotSet,
        })
    }

    /// Attempts to parse the provided player character data string and update the fields
    /// on the provided active player character model. Will return a None option if parsing
    /// failed.
    ///
    /// # Format
    /// ```
    /// 20;4;Adept;20;0;50
    /// 20;4;NAME;LEVEL;EXP;PROMOTIONS
    /// ```
    fn parse(model: &mut player_classes::ActiveModel, value: &str) -> Option<()> {
        let mut parser = MEStringParser::new(value)?;
        model.name = Set(parser.next_str()?);
        model.level = Set(parser.next()?);
        model.exp = Set(parser.next()?);
        model.promotions = Set(parser.next()?);
        Some(())
    }

    /// Attempts to parse the class index from the provided
    /// class key. If the key is too short or doesn't contain
    /// an index then an error is returned
    fn parse_index(key: &str) -> Result<u16, PlayerClassesError> {
        if key.len() <= 5 {
            warn!("Player class key was missing index");
            return Err(PlayerClassesError::InvalidKey);
        }
        match key[5..].parse() {
            Ok(value) => Ok(value),
            Err(_) => {
                warn!("Player class key index was not an integer");
                Err(PlayerClassesError::InvalidIndex)
            }
        }
    }

    /// Updates the provided class for the provided player
    /// by parsing the key and values
    ///
    /// `db`     The database instance
    /// `player` The player to update the class for
    /// `key`    The key to determine which class to update
    /// `value`  The value to use for updating the class
    pub async fn update(
        db: &DatabaseConnection,
        player: &players::Model,
        key: &str,
        value: &str,
    ) -> Result<(), PlayerClassesError> {
        let index = Self::parse_index(key)?;
        let mut model = Self::find(db, player, index).await?;
        if let None = Self::parse(&mut model, value) {
            warn!("Failed to fully parse player class: {key} = {value}");
        }
        model.save(db).await?;
        Ok(())
    }

    /// Encodes the provided player character model into the ME string
    /// encoded value to send as apart of the settings map
    pub fn encode(model: &player_classes::Model) -> String {
        format!(
            "20;4;{};{};{};{}",
            model.name, model.level, model.exp, model.promotions
        )
    }
}

#[derive(Debug)]
pub enum PlayerClassesError {
    InvalidKey,
    InvalidIndex,
    Database(DbErr),
}

impl From<DbErr> for PlayerClassesError {
    fn from(err: DbErr) -> Self {
        Self::Database(err)
    }
}
