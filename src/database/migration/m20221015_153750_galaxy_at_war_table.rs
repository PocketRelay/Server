//! Migration logic for generating galaxy at war table which
//! stores the galaxy at war progress in each region for a
//! specific player

use super::m20221015_142649_players_table::Players;
use sea_orm_migration::{
    prelude::*,
    schema::{date_time, integer, pk_auto},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GalaxyAtWar::Table)
                    .if_not_exists()
                    .col(pk_auto(GalaxyAtWar::Id))
                    .col(integer(GalaxyAtWar::PlayerId))
                    .col(date_time(GalaxyAtWar::LastModified))
                    .col(integer(GalaxyAtWar::GroupA))
                    .col(integer(GalaxyAtWar::GroupB))
                    .col(integer(GalaxyAtWar::GroupC))
                    .col(integer(GalaxyAtWar::GroupD))
                    .col(integer(GalaxyAtWar::GroupE))
                    .foreign_key(
                        ForeignKey::create()
                            .from(GalaxyAtWar::Table, GalaxyAtWar::PlayerId)
                            .to(Players::Table, Players::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GalaxyAtWar::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum GalaxyAtWar {
    Table,
    Id,
    PlayerId,
    LastModified,
    GroupA,
    GroupB,
    GroupC,
    GroupD,
    GroupE,
}
