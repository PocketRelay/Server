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
/// Connects to the database returning a Database connection
/// which allows accessing the database without accessing sea_orm
///
/// `ty` The type of database to connect to
pub async fn connect(file: String) -> DatabaseConnection {
    let url = init_sqlite(file);
    let connection = SeaDatabase::connect(&url)
        .await
        .expect("Unable to create database connection");

    Migrator::up(&connection, None)
        .await
        .expect("Unable to run database migrations");

    connection
}

/// Initializes the SQLite database file at the provided
/// file path ensuring that the parent directories and the
/// database file itself exist. Appends the sqlite: prefix
/// to the file to create the sqlite URL.
///
/// `file` The file to initialize
fn init_sqlite(file: String) -> String {
    let path = Path::new(&file);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            create_dir_all(parent).expect("Unable to create parent directory for sqlite database");
        }
    }
    if !path.exists() {
        File::create(path).expect("Unable to create sqlite database file");
    }
    format!("sqlite:{file}")
}
