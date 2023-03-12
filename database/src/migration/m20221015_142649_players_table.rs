//! Migration logic for generating the players table which stores
//! the basic player details

use sea_orm_migration::prelude::*;

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
                    .col(
                        ColumnDef::new(Players::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Players::Email)
                            .string_len(254)
                            .unique_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Players::DisplayName)
                            .string_len(99)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Players::Password).string().null())
                    .col(
                        ColumnDef::new(Players::Role)
                            .tiny_integer()
                            .default(0)
                            .not_null(),
                    )
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
