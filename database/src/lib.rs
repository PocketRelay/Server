use std::path::Path;

mod migration;

pub mod entities;
pub mod interfaces;

use interfaces::galaxy_at_war::GalaxyAtWarInterface;
use log::{debug, info};
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database as SeaDatabase, DatabaseConnection};
use tokio::{
    fs::{create_dir_all, File},
    io,
};

pub use sea_orm::DbErr;
pub type DbResult<T> = Result<T, DbErr>;

/// Structure wrapping the database connection and providing functionality
/// for accessing the database without exposing any normal database access
pub struct Database {
    connection: DatabaseConnection,
    pub gaw: GalaxyAtWarInterface,
}

impl Database {
    /// Connects to the database returning a Database interface
    /// which allows accessing the database without accessing sea_orm
    ///
    /// `file` The path to the SQLite database file
    pub async fn connect(file: String) -> Self {
        let path = Path::new(&file);
        Self::ensure_exists(path)
            .await
            .expect("Unable to create database file / directory");

        let url = format!("sqlite:{file}");

        let connection = SeaDatabase::connect(&url)
            .await
            .expect("Unable to create database connection");

        info!("Connected to database: {url}");
        debug!("Running migrations...");

        Migrator::up(&connection, None)
            .await
            .expect("Unable to run database migrations");
        debug!("Migrations complete");

        Self {
            connection,
            gaw: GalaxyAtWarInterface,
        }
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
