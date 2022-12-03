//! SeaORM Entity. Generated by sea-orm-codegen 0.9.3

use sea_orm::entity::prelude::*;
use serde::Serialize;
use utils::types::PlayerID;

/// Structure for a player class model stored in the database
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "player_classes")]
pub struct Model {
    /// The unique ID for this player class
    #[sea_orm(primary_key)]
    #[serde(skip)]
    pub id: u32,
    /// The ID of the player this class belongs to
    #[serde(skip)]
    pub player_id: PlayerID,
    /// The index of this class
    pub index: u16,
    /// The class name
    pub name: String,
    /// The class level
    pub level: u32,
    /// The amount of exp the class has
    pub exp: f32,
    /// The number of promotions the class has
    pub promotions: u32,
}

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
