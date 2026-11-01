use chrono::Utc;
use pocket_relay::database::{
    DatabaseConnection,
    entities::{GameReport, game_report::GameReportData},
};

mod common;

async fn test_create_game_report(db: &DatabaseConnection) {
    GameReport::create(
        db,
        GameReportData {
            attributes: Default::default(),
            players: Default::default(),
            seed: Default::default(),
            extracted: Default::default(),
        },
        Utc::now(),
        Utc::now(),
    )
    .await
    .unwrap();
}

database_tests!(test_create_game_report,);
