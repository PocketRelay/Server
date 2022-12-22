//! Migration logic for generating the players table
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
                    .col(ColumnDef::new(Players::Email).string_len(254).not_null())
                    .col(
                        ColumnDef::new(Players::DisplayName)
                            .string_len(99)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Players::SessionToken).string_len(254).null())
                    .col(ColumnDef::new(Players::Origin).boolean().not_null())
                    .col(ColumnDef::new(Players::Password).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Players::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Players {
    Table,
    Id,
    Email,
    DisplayName,
    SessionToken,
    Origin,
    Password,
}
