use pocket_relay::database::DatabaseConnection;
use pocket_relay::database::entities::{Player, PlayerRole};

pub async fn quick_test_player(db: &DatabaseConnection, email: impl Into<String>) -> Player {
    quick_test_player_with_name(db, email, "test").await
}

pub async fn quick_test_player_with_name(
    db: &DatabaseConnection,
    email: impl Into<String>,
    name: impl Into<String>,
) -> Player {
    Player::create(db, email.into(), name.into(), None, PlayerRole::Default)
        .await
        .unwrap()
}
