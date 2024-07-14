use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add the last login date time column
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .add_column(ColumnDef::new(Players::LastLoginAt).date_time().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the last login date time column
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .drop_column(Players::LastLoginAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Players {
    Table,

    LastLoginAt,
}
