//! Migration to create the `player_data` table which stores the
//! associated player data for each player as key value pairs

use sea_orm_migration::prelude::*;

use super::m20221015_142649_players_table::Players;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerData::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerData::PlayerId).integer().not_null())
                    .col(ColumnDef::new(PlayerData::Key).string().not_null())
                    .col(ColumnDef::new(PlayerData::Value).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerData::Table, PlayerData::PlayerId)
                            .to(Players::Table, Players::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlayerData::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PlayerData {
    Table,
    Id,
    PlayerId,
    Key,
    Value,
}
