use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GameReport::Table)
                    .if_not_exists()
                    .col(pk_auto(GameReport::Id))
                    .col(json_binary(GameReport::Data))
                    .col(date_time(GameReport::CreatedAt))
                    .col(date_time(GameReport::FinishedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GameReport::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GameReport {
    Table,
    /// Unique ID of the game report
    Id,
    /// The actual game report data
    Data,
    /// Timestamp of the game creation
    CreatedAt,
    /// Timestamp of the game finish
    FinishedAt,
}
