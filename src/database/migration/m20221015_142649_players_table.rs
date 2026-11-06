//! Migration logic for generating the players table which stores
//! the basic player details

use sea_orm_migration::{
    prelude::*,
    schema::{integer, pk_auto, string_len, string_len_uniq, string_null},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Players::Table)
                    .if_not_exists()
                    .col(pk_auto(Players::Id))
                    .col(string_len_uniq(Players::Email, 254))
                    .col(string_len(Players::DisplayName, 99))
                    .col(string_null(Players::Password))
                    .col(integer(Players::Role).default(0))
                    .to_owned(),
            )
            .await?;

        // Create index for email
        manager
            .create_index(
                Index::create()
                    .name("idx-pr-email")
                    .table(Players::Table)
                    .col(Players::Email)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the table
        manager
            .drop_table(Table::drop().table(Players::Table).to_owned())
            .await?;

        // Drop the index
        manager
            .drop_index(
                Index::drop()
                    .name("idx-pr-email")
                    .table(Players::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum Players {
    Table,
    Id,
    Email,
    DisplayName,
    Password,
    Role,
}
