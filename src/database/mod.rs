use log::{error, info, warn};
use migration::{Migrator, MigratorTrait};
use sea_orm::Database as SeaDatabase;
use std::{
    fs::{create_dir_all, File},
    path::Path,
};

pub mod entities;
mod migration;

/// Testing seeding logic
#[cfg(test)]
mod seed;

// Re-exports of database types
pub use sea_orm::DatabaseConnection;
pub use sea_orm::DbErr;

use self::entities::{Player, PlayerRole};
use crate::{
    config::RuntimeConfig,
    utils::hashing::{hash_password, verify_password},
};

/// Database error result type
pub type DbResult<T> = Result<T, DbErr>;

const DATABASE_PATH: &str = "data/app.db";
const DATABASE_PATH_URL: &str = "sqlite:data/app.db";

/// Connects to the database and applies the admin changes if
/// required, returning the database connection
pub async fn init(config: &RuntimeConfig) -> DatabaseConnection {
    info!("Connected to database..");

    let connection = connect_database().await;

    // Setup the super admin account
    init_database_admin(&connection, config).await;

    connection
}

/// Connects to the database
async fn connect_database() -> DatabaseConnection {
    let path = Path::new(&DATABASE_PATH);

    // Create path to database file if missing
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            create_dir_all(parent).expect("Unable to create parent directory for sqlite database");
        }
    }

    // Create the database if file is missing
    if !path.exists() {
        File::create(path).expect("Unable to create sqlite database file");
    }

    // Connect to database
    let connection = SeaDatabase::connect(DATABASE_PATH_URL)
        .await
        .expect("Unable to create database connection");

    // Run migrations
    if let Err(err) = Migrator::up(&connection, None).await {
        if let DbErr::Custom(custom_err) = err {
            if custom_err
                .contains("is missing, this migration has been applied but its file is missing")
            {
                // Forward migrations are not always a failure, so its just a warning
                warn!(
                    "It looks like your app.db has been used with a newer version \
                of Pocket Relay, you may encounter unexpected issues or bugs its \
                recommended that you backup your database before trying a new version: {}",
                    custom_err
                )
            }
        } else {
            // Other errors should be considered fatal
            panic!("Failed to run database migrations: {}", err);
        }
    }

    connection
}

/// Initializes the database super admin account using the
/// admin email stored within the environment variables if
/// one is present
///
/// `db`     The database connection
/// `config` The config to use for the admin details
async fn init_database_admin(db: &DatabaseConnection, config: &RuntimeConfig) {
    let admin_email = match &config.dashboard.super_email {
        Some(value) => value,
        None => return,
    };

    let player = match Player::by_email(db, admin_email).await {
        // Player exists
        Ok(Some(value)) => value,
        // Player doesn't exist yet
        Ok(None) => return,
        // Encountered an error
        Err(err) => {
            error!("Failed to find player to provide super admin: {:?}", err);
            return;
        }
    };

    let player = match player.set_role(db, PlayerRole::SuperAdmin).await {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to assign super admin role: {:?}", err);
            return;
        }
    };

    if let Some(password) = &config.dashboard.super_password {
        let password_hash = hash_password(password).expect("Failed to hash super user password");

        let matches = match &player.password {
            Some(value) => verify_password(password, value),
            None => false,
        };

        if !matches {
            if let Err(err) = player.set_password(db, password_hash).await {
                error!("Failed to set super admin password: {:?}", err)
            } else {
                info!("Updated super admin password")
            }
        }
    }
}
