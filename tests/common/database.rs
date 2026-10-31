use pocket_relay::{config::DatabaseConfig, database::connect_database};
use sea_orm::DatabaseConnection;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

use crate::common::postgres::{test_database_container, test_database_url};

pub async fn sqlite_database() -> DatabaseConnection {
    let config = DatabaseConfig {
        connection_url: Some("sqlite::memory:".to_string()),
        ..Default::default()
    };
    connect_database(&config).await
}

pub async fn postgres_database() -> (DatabaseConnection, ContainerAsync<Postgres>) {
    let postgres = test_database_container().await;
    let url = test_database_url(&postgres).await;

    let config = DatabaseConfig {
        connection_url: Some(url),
        ..Default::default()
    };
    (connect_database(&config).await, postgres)
}
