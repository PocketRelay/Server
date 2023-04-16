use sea_orm::entity::prelude::*;
use serde::Serialize;

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
