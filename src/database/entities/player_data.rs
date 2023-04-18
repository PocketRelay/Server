use sea_orm::{
    entity::prelude::*, ActiveValue::NotSet, DeleteResult, InsertResult, IntoActiveModel, Set,
};
use serde::Serialize;
use std::future::Future;

use crate::{database::DbResult, utils::types::PlayerID};

/// Structure for player data stro
#[derive(Serialize, Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "player_data")]
pub struct Model {
    /// Unique Identifier for the player data
    #[sea_orm(primary_key)]
    #[serde(skip)]
    pub id: u32,
    /// Unique Identifier of the player this data belongs to
    #[serde(skip)]
    pub player_id: u32,
    /// The key for this player data
    pub key: String,
    /// The value for this player data
    pub value: String,
}

/// The relationships for the player data
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::Id",
        to = "Column::PlayerId",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    SelfRef,
    #[sea_orm(
        belongs_to = "super::players::Entity",
        from = "Column::PlayerId",
        to = "super::players::Column::Id"
    )]
    Player,
}

impl Related<super::players::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Retrieves all the player data for the desired player
    ///
    /// `player_id` The ID of the player
    /// `db`        The database connection
    pub fn all(
        db: &DatabaseConnection,
        player_id: PlayerID,
    ) -> impl Future<Output = DbResult<Vec<Model>>> + Send + '_ {
        Entity::find()
            .filter(Column::PlayerId.eq(player_id))
            .all(db)
    }

    /// Sets the key value data for the provided player. If the data exists then
    /// the value is updated otherwise the data will be created. The new data is
    /// returned.
    ///
    /// `player_id` The ID of the player
    /// `db`        The database connection
    /// `key`       The data key
    /// `value`     The data value
    pub async fn set(
        db: &DatabaseConnection,
        player_id: PlayerID,
        key: String,
        value: String,
    ) -> DbResult<Self> {
        let existing = Entity::find()
            .filter(
                Column::PlayerId
                    .eq(player_id)
                    .and(Column::Key.eq(&key as &str)),
            )
            .one(db)
            .await?;

        if let Some(player_data) = existing {
            let mut model = player_data.into_active_model();
            model.key = Set(key);
            model.value = Set(value);
            model.update(db).await
        } else {
            ActiveModel {
                player_id: Set(player_id),
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
    /// `db`        The database connection
    /// `player_id` The ID of the player to set the data for
    /// `data`      Iterator of the data keys and values
    pub fn set_bulk(
        db: &DatabaseConnection,
        player_id: PlayerID,
        data: impl Iterator<Item = (String, String)>,
    ) -> impl Future<Output = DbResult<InsertResult<ActiveModel>>> + Send + '_ {
        // Transform the provided key values into active models
        let models_iter = data.map(|(key, value)| ActiveModel {
            id: NotSet,
            player_id: Set(player_id),
            key: Set(key),
            value: Set(value),
        });
        // Insert all the models
        Entity::insert_many(models_iter).exec(db)
    }

    /// Deletes the player data with the provided key for the
    /// current player
    ///
    /// `db`        The database connection
    /// `player_id` The ID of the player to delete the data from
    /// `key`       The data key
    pub fn delete<'a>(
        db: &'a DatabaseConnection,
        player_id: PlayerID,
        key: &str,
    ) -> impl Future<Output = DbResult<DeleteResult>> + Send + 'a {
        Entity::delete_many()
            .filter(Column::PlayerId.eq(player_id).and(Column::Key.eq(key)))
            .exec(db)
    }

    /// Gets the player data with the provided key for the
    /// current player
    ///
    /// `db`        The database connection
    /// `player_id` The ID of the player to get the data for
    /// `key`       The data key
    pub fn get<'a>(
        db: &'a DatabaseConnection,
        player_id: PlayerID,
        key: &str,
    ) -> impl Future<Output = DbResult<Option<Self>>> + Send + 'a {
        Entity::find()
            .filter(Column::PlayerId.eq(player_id).and(Column::Key.eq(key)))
            .one(db)
    }

    /// Gets all the player class data for the current player
    ///
    /// `db`        The database connection
    /// `player_id` The ID of the player to get the classes for
    pub fn get_classes(
        db: &DatabaseConnection,
        player_id: PlayerID,
    ) -> impl Future<Output = DbResult<Vec<Self>>> + Send + '_ {
        Entity::find()
            .filter(
                Column::PlayerId
                    .eq(player_id)
                    .and(Column::Key.starts_with("class")),
            )
            .all(db)
    }

    /// Parses the challenge points value which is the second
    /// item in the completion list.
    ///
    /// `db`        The database connection
    /// `player_id` The ID of the player to get the cp for
    pub async fn get_challenge_points(db: &DatabaseConnection, player_id: PlayerID) -> Option<u32> {
        let list = Self::get(db, player_id, "Completion").await.ok()??.value;
        let part = list.split(',').nth(1)?;
        let value: u32 = part.parse().ok()?;
        Some(value)
    }
}
