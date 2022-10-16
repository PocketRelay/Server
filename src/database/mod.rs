use std::io;
use std::path::Path;
use log::info;
use sea_orm::DatabaseConnection;
use tokio::fs::File;
use migration::{Migrator, MigratorTrait};
use crate::env;

pub mod entities;
pub mod interface;

pub async fn connect() -> io::Result<DatabaseConnection> {
    info!("Connecting to database..");

    let db_file = env::database_file();
    let file_path = Path::new(db_file);
    if !file_path.exists() {
        File::create(file_path).await?;
    }

    let con_str = format!("sqlite:{db_file}");
    let connection = sea_orm::Database::connect(&con_str)
        .await
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Unable to create database connection: {err:?}")
            )
        })?;

    info!("Connected to database: {con_str}");

    info!("Running migrations...");

    Migrator::up(&connection, None)
        .await
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Unable to run database migrations: {err:?}")
            )
        })?;

    info!("Migrations complete.");

    Ok(connection)
}