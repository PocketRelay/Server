//! Migration to create the `player_data` table which stores the
//! associated player data for each player as key value pairs

use sea_orm_migration::{
    prelude::*,
    schema::{integer, pk_auto, string, text},
};

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
                    .col(pk_auto(PlayerData::Id))
                    .col(integer(PlayerData::PlayerId))
                    .col(string(PlayerData::Key))
                    .col(text(PlayerData::Value))
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
