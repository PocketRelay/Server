use sea_orm_migration::prelude::*;
use crate::m20221015_142649_players_table::Players;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(PlayerClasses::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(PlayerClasses::Id)
                        .integer()
                        .not_null()
                        .auto_increment()
                        .primary_key()
                )
                .col(
                    ColumnDef::new(PlayerClasses::Id)
                        .integer()
                        .not_null()
                )
                .col(
                    ColumnDef::new(PlayerClasses::Index)
                        .integer_len(2)
                        .not_null()
                )
                .col(
                    ColumnDef::new(PlayerClasses::Name)
                        .text()
                        .not_null()
                )
                .col(
                    ColumnDef::new(PlayerClasses::Level)
                        .integer_len(3)
                        .not_null()
                )
                .col(
                    ColumnDef::new(PlayerClasses::Exp)
                        .float_len(4)
                        .not_null()
                )
                .col(
                    ColumnDef::new(PlayerClasses::Promotions)
                        .integer_len(6)
                        .not_null()
                )
                .to_owned()
        ).await?;
        manager.create_foreign_key(
            ForeignKey::create()
                .from(Players::Table, Players::Id)
                .to(PlayerClasses::Table, PlayerClasses::PlayerId)
                .to_owned()
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlayerClasses::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PlayerClasses {
    Table,
    Id,
    PlayerId,
    Index,
    Name,
    Level,
    Exp,
    Promotions
}