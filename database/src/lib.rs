use migration::{Migrator, MigratorTrait};
use sea_orm::Database as SeaDatabase;
use std::{
    fs::{create_dir_all, File},
    path::Path,
};

mod data;
mod entities;
pub mod interfaces;
mod migration;

// Re-exports of named entities
pub use data::user::PlayerRole;
pub use entities::{GalaxyAtWar, Player, PlayerData};

// Re-exports of database types
pub use sea_orm::DatabaseConnection;
pub use sea_orm::DbErr;

/// Database error result type
pub type DbResult<T> = Result<T, DbErr>;

const DATABASE_PATH: &str = "data/app.db";
const DATABASE_PATH_URL: &str = "sqlite:data/app.db";

/// Connects to the database returning a Database connection
/// which allows accessing the database without accessing sea_orm
pub async fn init() -> DatabaseConnection {
    let path = Path::new(&DATABASE_PATH);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            create_dir_all(parent).expect("Unable to create parent directory for sqlite database");
        }
    }

    if !path.exists() {
        File::create(path).expect("Unable to create sqlite database file");
    }

    let connection = SeaDatabase::connect(DATABASE_PATH_URL)
        .await
        .expect("Unable to create database connection");

    Migrator::up(&connection, None)
        .await
        .expect("Unable to run database migrations");

    connection
}
