use crate::m20221015_142649_players_table::Players;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerCharacters::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerCharacters::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::PlayerId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::Index)
                            .integer_len(3)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlayerCharacters::KitName).text().not_null())
                    .col(ColumnDef::new(PlayerCharacters::Name).text().not_null())
                    .col(
                        ColumnDef::new(PlayerCharacters::Tint1)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::Tint2)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::Pattern)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::PatternColor)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::Phong)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::Emissive)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::SkinTone)
                            .integer_len(4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::SecondsPlayed)
                            .integer_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::TimestampYear)
                            .integer_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::TimestampMonth)
                            .integer_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::TimestampDay)
                            .integer_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::TimestampSeconds)
                            .integer_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlayerCharacters::Powers).text().not_null())
                    .col(ColumnDef::new(PlayerCharacters::Hotkeys).text().not_null())
                    .col(ColumnDef::new(PlayerCharacters::Weapons).text().not_null())
                    .col(
                        ColumnDef::new(PlayerCharacters::WeaponMods)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::Deployed)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(PlayerCharacters::LeveledUp)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerCharacters::Table, PlayerCharacters::PlayerId)
                            .to(Players::Table, Players::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlayerCharacters::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PlayerCharacters {
    Table,
    Id,
    PlayerId,
    Index,
    KitName,
    Name,
    Tint1,
    Tint2,
    Pattern,
    PatternColor,
    Phong,
    Emissive,
    SkinTone,
    SecondsPlayed,
    TimestampYear,
    TimestampMonth,
    TimestampDay,
    TimestampSeconds,
    Powers,
    Hotkeys,
    Weapons,
    WeaponMods,
    Deployed,
    LeveledUp,
}