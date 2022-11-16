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
use sea_orm::{Database as SeaDatabase, DatabaseConnection};
use tokio::{
    fs::{create_dir_all, File},
    io,
};

pub use entities::*;

pub use sea_orm::DbErr;
pub type DbResult<T> = Result<T, DbErr>;

/// Structure wrapping the database connection and providing functionality
/// for accessing the database without exposing any normal database access
pub struct Database {
    connection: DatabaseConnection,
}

impl Database {
    /// Wrapper function for connecting to the database through
    /// a SQLite file connection.
    ///
    /// `file` The path to the SQLite database file
    pub async fn connect_sqlite(file: String) -> Self {
        let path = Path::new(&file);
        Self::ensure_exists(path)
            .await
            .expect("Unable to create database file / directory");

        let url = format!("sqlite:{file}");
        Self::connect_url(url).await
    }

    /// Connects to the database returning a Database interface
    /// which allows accessing the database without accessing sea_orm
    ///
    /// `url` The database connection url
    pub async fn connect_url(url: String) -> Self {
        let connection = SeaDatabase::connect(&url)
            .await
            .expect("Unable to create database connection");

        info!("Connected to database: {url}");
        debug!("Running migrations...");

        Migrator::up(&connection, None)
            .await
            .expect("Unable to run database migrations");
        debug!("Migrations complete");

        Self { connection }
    }

    /// Ensures the provided path exists and will attempt to
    /// create it and the parent directories if they are missing.
    ///
    /// `path` The path to ensure exists
    async fn ensure_exists(path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                create_dir_all(parent).await?;
            }
        }

        if !path.exists() {
            File::create(path).await?;
        }

        Ok(())
    }
}
