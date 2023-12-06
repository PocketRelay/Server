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
                    .table(LeaderboardData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LeaderboardData::Id)
                            .unsigned()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(LeaderboardData::PlayerId)
                            .unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(LeaderboardData::Ty).unsigned().not_null())
                    .col(ColumnDef::new(LeaderboardData::Value).unsigned().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(LeaderboardData::Table, LeaderboardData::PlayerId)
                            .to(Players::Table, Players::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("idx-pid-ty-key")
                    .table(LeaderboardData::Table)
                    .col(LeaderboardData::Ty)
                    .col(LeaderboardData::PlayerId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LeaderboardData::Table).to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .table(LeaderboardData::Table)
                    .name("idx-pid-ty-key")
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum LeaderboardData {
    Table,
    Id,
    Ty,
    PlayerId,
    Value,
}
