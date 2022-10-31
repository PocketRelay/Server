pub mod gaw;
pub mod player_characters;
pub mod player_classes;
pub mod player_data;
pub mod players;

pub type DbResult<T> = Result<T, sea_orm::DbErr>;
