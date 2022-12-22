use log::{debug, info};
use migration::{Migrator, MigratorTrait};
use sea_orm::Database as SeaDatabase;
use std::{
    fs::{create_dir_all, File},
    path::Path,
};

mod entities;
pub mod interfaces;
mod migration;

// Re-exports of named entities
pub use entities::{GalaxyAtWar, Player, PlayerData};

// Re-exports of database types
pub use sea_orm::DatabaseConnection;
pub use sea_orm::DbErr;

/// Database error result type
pub type DbResult<T> = Result<T, DbErr>;

/// Type of database to connect to with the relevant
/// connection string / file
pub enum DatabaseType {
    /// SQLite database connection with the file name / path
    Sqlite(String),
    /// MySQL database connection with the MySQL Url
    MySQL(String),
}

/// Connects to the database returning a Database connection
/// which allows accessing the database without accessing sea_orm
///
/// `ty` The type of database to connect to
pub async fn connect(ty: DatabaseType) -> DatabaseConnection {
    let url = match ty {
        DatabaseType::Sqlite(file) => init_sqlite(file),
        DatabaseType::MySQL(url) => url,
    };
    let connection = SeaDatabase::connect(&url)
        .await
        .expect("Unable to create database connection");

    info!("Connected to database: {url}");
    debug!("Running migrations...");

    Migrator::up(&connection, None)
        .await
        .expect("Unable to run database migrations");
    debug!("Migrations complete");

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
