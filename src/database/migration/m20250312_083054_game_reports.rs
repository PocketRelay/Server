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
                    .col(big_pk_auto(GameReport::Id))
                    .col(json_binary(GameReport::Data))
                    .col(timestamp_with_time_zone(GameReport::CreatedAt))
                    .col(timestamp_with_time_zone(GameReport::FinishedAt))
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
