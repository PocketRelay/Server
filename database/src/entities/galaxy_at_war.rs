//! SeaORM Entity. Generated by sea-orm-codegen 0.9.3

use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::Serialize;

/// Structure for a galaxy at war model stored in the database
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "galaxy_at_war")]
pub struct Model {
    /// The unique ID for this galaxy at war data
    #[sea_orm(primary_key)]
    #[serde(skip)]
    pub id: u32,
    /// The ID of the player this galaxy at war data belongs to
    #[serde(skip)]
    pub player_id: u32,
    /// The time at which this galaxy at war data was last modified. Used
    /// to calculate how many days of decay have passed
    pub last_modified: NaiveDateTime,
    /// The first group value
    pub group_a: u16,
    /// The second group value
    pub group_b: u16,
    /// The third group value
    pub group_c: u16,
    /// The fourth group value
    pub group_d: u16,
    /// The fifth group value
    pub group_e: u16,
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
