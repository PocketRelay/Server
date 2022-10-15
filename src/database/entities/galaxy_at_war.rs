//! SeaORM Entity. Generated by sea-orm-codegen 0.9.3

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "galaxy_at_war")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u32,
    pub player_id: u32,
    pub last_modified: u64,
    pub group_a: u32,
    pub group_b: u32,
    pub group_c: u32,
    pub group_d: u32,
    pub group_e: u32,
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
}

impl ActiveModelBehavior for ActiveModel {}
