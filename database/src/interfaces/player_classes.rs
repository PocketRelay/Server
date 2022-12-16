use crate::{
    dto::player_classes::PlayerClassUpdate,
    entities::{player_classes, players},
    DbResult, PlayerClass,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter,
};

impl PlayerClass {
    /// Finds all the player classes for the provided player model
    ///
    /// `db`     The database instance
    /// `player` The player to find the classes for
    pub async fn find_all(db: &DatabaseConnection, player: &players::Model) -> DbResult<Vec<Self>> {
        player.find_related(player_classes::Entity).all(db).await
    }

    /// Finds all the player classes for the player with the provided ID
    ///
    /// `db`        The databse connection
    /// `player_id` The player ID to find classes for
    pub async fn find_by_pid(db: &DatabaseConnection, player_id: u32) -> DbResult<Vec<Self>> {
        player_classes::Entity::find()
            .filter(player_classes::Column::PlayerId.eq(player_id))
            .all(db)
            .await
    }

    /// Updates the level and promotions value for the current class
    /// if provided saving the changes to the database
    ///
    /// `level`      Optional level value to change
    /// `promotions` Optional promotions value to change
    pub async fn update_http(
        self,
        db: &DatabaseConnection,
        level: Option<u8>,
        promotions: Option<u32>,
    ) -> DbResult<Self> {
        let mut active = self.into_active_model();
        if let Some(level) = level {
            active.level = Set(level);
        }
        if let Some(promotions) = promotions {
            active.promotions = Set(promotions);
        }
        active.update(db).await
    }

    /// Finds the player class with the specific index
    ///
    /// `db`     The database instance
    /// `player` The player to find the class for
    /// `index`  The index to find
    pub async fn find_index(
        db: &DatabaseConnection,
        player: &players::Model,
        index: u16,
    ) -> DbResult<Option<Self>> {
        player_classes::Entity::find()
            .filter(
                player_classes::Column::PlayerId
                    .eq(player.id)
                    .and(player_classes::Column::Index.eq(index)),
            )
            .one(db)
            .await
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

    fn apply_update(model: &mut player_classes::ActiveModel, update: PlayerClassUpdate) {
        model.name = Set(update.name);
        model.level = Set(update.level);
        model.exp = Set(update.exp);
        model.promotions = Set(update.promotions);
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
        index: u16,
        update: PlayerClassUpdate,
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
            "20;4;{};{};{};{}",
            &self.name, self.level, self.exp, self.promotions
        )
    }
}
