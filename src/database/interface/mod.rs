use sea_orm::DbErr;

pub mod player_characters;
pub mod player_classes;
pub mod players;

pub type DbResult<T> = Result<T, DbErr>;
