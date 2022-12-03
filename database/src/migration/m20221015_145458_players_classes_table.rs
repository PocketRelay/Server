//! Migration logic for generating player classes table
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
                    .table(PlayerClasses::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerClasses::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerClasses::PlayerId).integer().not_null())
                    .col(
                        ColumnDef::new(PlayerClasses::Index)
                            .integer_len(2)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlayerClasses::Name).text().not_null())
                    .col(
                        ColumnDef::new(PlayerClasses::Level)
                            .integer_len(3)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlayerClasses::Exp).float_len(4).not_null())
                    .col(
                        ColumnDef::new(PlayerClasses::Promotions)
                            .integer_len(6)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerClasses::Table, PlayerClasses::PlayerId)
                            .to(Players::Table, Players::Id),
                    )
                    .to_owned(),
            )
            .await
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
    Promotions,
}
