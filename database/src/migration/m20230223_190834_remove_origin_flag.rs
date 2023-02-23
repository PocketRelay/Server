//! This migration removes the origin flag from the players table and
//! adds the requirement that all emails are unique and creates an
//! email index
//!
//! Makes password field nullable for origin accounts

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Make the email field unique
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    // Drop origin column
                    .drop_column(Players::Origin)
                    // Make email unique
                    .modify_column(
                        ColumnDef::new(Players::Email)
                            .string_len(254)
                            .unique_key()
                            .not_null(),
                    )
                    // Make password nullable
                    .modify_column(ColumnDef::new(Players::Password).string().null())
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
        // Revert the unique key
        // Make the email field unique
        manager
            .alter_table(
                Table::alter()
                    .table(Players::Table)
                    .modify_column(ColumnDef::new(Players::Email).string_len(254).not_null())
                    .to_owned(),
            )
            .await?;

        // Drop the index
        manager
            .drop_index(
                Index::drop()
                    .name("idx-pr-email")
                    .table(Players::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
pub enum Players {
    Table,
    Origin,
    Email,
    Password,
}
