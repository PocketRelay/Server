mod blaze;
mod env;
mod http;
mod utils;

use tokio::fs::File;
use std::io;
use std::path::Path;
use dotenvy::dotenv;
use env_logger::WriteStyle;
use log::info;
use tokio::try_join;
use blaze::components::{Authentication, Components};
use migration::{Migrator, MigratorTrait};

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();
    let log_level = env::logging_level();
    env_logger::builder()
        .filter_module("pocket_relay", log_level)
        .write_style(WriteStyle::Always)
        .init();

    let db_path = Path::new("app.db");
    if !db_path.exists() {
        File::create(db_path).await
            .unwrap();
    }

    let connection = sea_orm::Database::connect("sqlite:app.db").await
        .expect("Unable to connect to database app.db");
    Migrator::up(&connection, None).await
        .expect("Unable to migrate database app.db");

    info!("Starting Pocket Relay v{}", env::VERSION);

    try_join!(
        http::start_server(),
        blaze::start_server()
    )?;

    Ok(())
}

