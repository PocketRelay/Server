use std::path::Path;

mod migration;

mod entities;
mod interfaces;

pub mod snapshots;

pub use interfaces::{
    galaxy_at_war::GalaxyAtWarInterface, player_characters::PlayerCharactersInterface,
    player_classes::PlayerClassesInterface, players::PlayersInterface,
};
use log::{debug, info};
use migration::{Migrator, MigratorTrait};
use sea_orm::Database as SeaDatabase;
use tokio::fs::{create_dir_all, File};

pub use entities::*;

pub use sea_orm::DatabaseConnection;
pub use sea_orm::DbErr;

pub type DbResult<T> = Result<T, DbErr>;

pub enum DatabaseType {
    Sqlite(String),
    MySQL(String),
}

/// Connects to the database returning a Database connection
/// which allows accessing the database without accessing sea_orm
///
/// `ty` The type of database to connect to
pub async fn connect(ty: DatabaseType) -> DatabaseConnection {
    let url = match ty {
        DatabaseType::Sqlite(file) => init_sqlite(file).await,
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
/// to the file to create the sqlite URL
async fn init_sqlite(file: String) -> String {
    let path = Path::new(&file);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            create_dir_all(parent)
                .await
                .expect("Unable to create parent directory for sqlite database");
        }
    }
    if !path.exists() {
        File::create(path)
            .await
            .expect("Unable to create sqlite database file");
    }
    format!("sqlite:{file}")
}
