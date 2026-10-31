use pocket_relay::{config::DatabaseConfig, database::connect_database};

use crate::common::postgres::{test_database_container, test_database_url};

mod common;

#[tokio::test]
async fn test_sqlite_database_initialize() {
    let config = DatabaseConfig {
        connection_url: Some("sqlite::memory:".to_string()),
        ..Default::default()
    };
    connect_database(&config).await;
}

#[tokio::test]
async fn test_postgres_database_initialize() {
    let postgres = test_database_container().await;
    let url = test_database_url(&postgres).await;

    let config = DatabaseConfig {
        connection_url: Some(url),
        ..Default::default()
    };
    connect_database(&config).await;
}
