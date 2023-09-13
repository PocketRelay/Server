use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("idx-pid-key")
                    .table(PlayerData::Table)
                    .col(PlayerData::PlayerId)
                    .col(PlayerData::Key)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .table(PlayerData::Table)
                    .name("idx-pid-key")
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum PlayerData {
    Table,
    PlayerId,
    Key,
}
