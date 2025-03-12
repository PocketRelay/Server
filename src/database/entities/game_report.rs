use crate::{
    database::DbResult,
    utils::{
        parsing::player_character::{PlayerCharacterPower, PlayerCharacterWeaponMod, WeaponId},
        types::PlayerID,
    },
};
use futures_util::future::BoxFuture;
use sea_orm::{entity::prelude::*, ActiveValue::Set, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type GameReportModel = Model;

/// Structure for player data
#[derive(Serialize, Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "game_report")]
pub struct Model {
    /// Unique Identifier for the player data
    #[sea_orm(primary_key, column_type = "Integer")]
    pub id: i64,
    pub data: GameReportData,
    pub created_at: DateTimeUtc,
    pub finished_at: DateTimeUtc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct GameReportData {
    pub attributes: HashMap<String, String>,
    pub players: Vec<GameReportPlayer>,
    /// Randomness seed used by players
    pub seed: u32,
    /// Whether the extraction
    pub extracted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameReportPlayer {
    /// ID of the player
    pub player_id: PlayerID,

    /// Player username
    pub player_name: String,

    /// Name of the player character kit the player was using
    pub kit_name: Option<String>,

    /// Player weapon list
    pub weapons: Option<Vec<WeaponId>>,

    /// Player weapon mods
    pub weapon_mods: Option<Vec<PlayerCharacterWeaponMod>>,

    /// Player power choices and levels
    pub powers: Option<Vec<PlayerCharacterPower>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn create(
        db: &DatabaseConnection,
        data: GameReportData,
        created_at: DateTimeUtc,
        finished_at: DateTimeUtc,
    ) -> BoxFuture<'_, DbResult<Self>> {
        ActiveModel {
            data: Set(data),
            created_at: Set(created_at),
            finished_at: Set(finished_at),
            ..Default::default()
        }
        .insert(db)
    }
}
