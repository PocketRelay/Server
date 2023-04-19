//! Migration logic for generating galaxy at war table which
//! stores the galaxy at war progress in each region for a
//! specific player

use super::m20221015_142649_players_table::Players;
use sea_orm_migration::prelude::*;

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
                    .col(
                        ColumnDef::new(GalaxyAtWar::Id)
                            .unsigned()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GalaxyAtWar::PlayerId).unsigned().not_null())
                    .col(
                        ColumnDef::new(GalaxyAtWar::LastModified)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GalaxyAtWar::GroupA)
                            .small_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GalaxyAtWar::GroupB)
                            .small_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GalaxyAtWar::GroupC)
                            .small_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GalaxyAtWar::GroupD)
                            .small_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GalaxyAtWar::GroupE)
                            .small_unsigned()
                            .not_null(),
                    )
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
