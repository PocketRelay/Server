//! Migration for adding the role value to the player table and setting a default
//! value for all the players

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .add_column(
                        ColumnDef::new(Players::Role)
                            .tiny_integer()
                            .default(0)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .drop_column(Players::Role)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Players {
    Table,
    Role,
}
