use sea_orm::DbErr;
pub mod players;

pub type DbResult<T> = Result<T, DbErr>;