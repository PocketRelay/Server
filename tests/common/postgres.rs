use sea_orm::sqlx::{
    ConnectOptions,
    postgres::{PgConnectOptions, PgSslMode},
};
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

const TEST_DB_NAME: &str = "pocket_relay_test";
const TEST_DB_USER: &str = "pocket_relay";
const TEST_DB_PASSWORD: &str = "pocket_relay";

pub async fn test_database_container() -> ContainerAsync<Postgres> {
    testcontainers_modules::postgres::Postgres::default()
        .with_db_name(TEST_DB_NAME)
        .with_user(TEST_DB_USER)
        .with_password(TEST_DB_PASSWORD)
        .with_tag("18.1-alpine")
        .start()
        .await
        .unwrap()
}

pub async fn test_database_url(container: &ContainerAsync<Postgres>) -> String {
    let host_ip = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();

    let options = PgConnectOptions::new()
        .host(&host_ip.to_string())
        .port(host_port)
        .username(TEST_DB_USER)
        .password(TEST_DB_PASSWORD)
        .database(TEST_DB_NAME)
        .ssl_mode(PgSslMode::Disable);

    options.to_url_lossy().to_string()
}
