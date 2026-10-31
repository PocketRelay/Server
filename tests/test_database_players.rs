use pocket_relay::database::entities::{Player, PlayerRole};
use sea_orm::{DatabaseConnection, SqlErr, prelude::DateTimeUtc};

use crate::common::database::{postgres_database, sqlite_database};

mod common;

async fn quick_test_player(db: &DatabaseConnection, email: impl Into<String>) -> Player {
    quick_test_player_with_name(db, email, "test").await
}

async fn quick_test_player_with_name(
    db: &DatabaseConnection,
    email: impl Into<String>,
    name: impl Into<String>,
) -> Player {
    Player::create(db, email.into(), name.into(), None, PlayerRole::Default)
        .await
        .unwrap()
}

/// Tests that a player can be created and then retrieved and matches
async fn test_create_player(db: &DatabaseConnection) {
    let player = Player::create(
        db,
        "test_create@test.com".to_string(),
        "test".to_string(),
        None,
        PlayerRole::Default,
    )
    .await
    .unwrap();

    assert_eq!(player.email, "test_create@test.com");
    assert_eq!(player.display_name, "test");
    assert_eq!(player.password, None);
    assert_eq!(player.role, PlayerRole::Default);

    let player_get = Player::by_id(db, player.id)
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player, player_get)
}

/// Tests that an email cannot be duplicated on creation
async fn test_create_player_prevents_duplicate_email(db: &DatabaseConnection) {
    let _player_1 = Player::create(
        db,
        "test_create_duplicate_email@test.com".to_string(),
        "test".to_string(),
        None,
        PlayerRole::Default,
    )
    .await
    .unwrap();

    let player_2_err = Player::create(
        db,
        "test_create_duplicate_email@test.com".to_string(),
        "test 1".to_string(),
        None,
        PlayerRole::Default,
    )
    .await
    .unwrap_err();

    assert!(matches!(
        player_2_err.sql_err(),
        Some(SqlErr::UniqueConstraintViolation(_))
    ))
}

async fn test_delete_player(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_delete@test.com").await;
    let player_id = player.id;
    player.delete(db).await.unwrap();

    let player_get = Player::by_id(db, player_id).await.unwrap();
    assert_eq!(player_get, None);
}

async fn test_get_player_by_id(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_by_id@test.com").await;
    let player_get = Player::by_id(db, player.id)
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player, player_get);

    let player_get_unknown = Player::by_id(db, 999).await.unwrap();
    assert_eq!(player_get_unknown, None);
}

async fn test_get_player_by_email(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_by_email@test.com").await;
    let player_get = Player::by_email(db, "test_by_email@test.com")
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player, player_get);

    let player_get_unknown = Player::by_email(db, "test_unknown@test.com").await.unwrap();
    assert_eq!(player_get_unknown, None);
}

async fn test_get_player_by_username(db: &DatabaseConnection) {
    let player = quick_test_player_with_name(db, "test_by_username@test.com", "test_name").await;
    let player_get = Player::by_username(db, "test_name")
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player, player_get);

    let player_get_unknown = Player::by_username(db, "test_unknown").await.unwrap();
    assert_eq!(player_get_unknown, None);
}

async fn test_set_player_password(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_set_password@test.com").await;
    let updated = player
        .set_password(db, "test_password".to_string())
        .await
        .unwrap();
    assert_eq!(updated.password, Some("test_password".to_string()));

    let player_get = Player::by_id(db, updated.id)
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player_get, updated);
}

async fn test_set_player_role(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_set_role@test.com").await;
    let updated = player.set_role(db, PlayerRole::Admin).await.unwrap();
    assert_eq!(updated.role, PlayerRole::Admin);

    let player_get = Player::by_id(db, updated.id)
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player_get, updated);
}

async fn test_set_player_details(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_set_details@test.com").await;
    let updated = player
        .set_details(
            db,
            Some("test_username".to_string()),
            Some("test_2@test.com".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(updated.display_name, "test_username");
    assert_eq!(updated.email, "test_2@test.com");

    let player_get = Player::by_id(db, updated.id)
        .await
        .unwrap()
        .expect("player should exist");

    assert_eq!(player_get, updated);
}

async fn test_set_player_last_login(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test_last_login@test.com").await;

    let constant_date = DateTimeUtc::default();

    Player::set_last_login(db, player.id, constant_date)
        .await
        .unwrap();

    let player_get = Player::by_id(db, player.id)
        .await
        .unwrap()
        .expect("player should exist");
    assert_eq!(player_get.last_login_at, Some(constant_date));
}

#[tokio::test]
async fn test_create_player_sqlite() {
    let db = sqlite_database().await;
    test_create_player(&db).await;
}

#[tokio::test]
async fn test_create_player_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_create_player(&db).await;
}

#[tokio::test]
async fn test_create_player_prevents_duplicate_email_sqlite() {
    let db = sqlite_database().await;
    test_create_player_prevents_duplicate_email(&db).await;
}

#[tokio::test]
async fn test_create_player_prevents_duplicate_email_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_create_player_prevents_duplicate_email(&db).await;
}

#[tokio::test]
async fn test_delete_player_sqlite() {
    let db = sqlite_database().await;
    test_delete_player(&db).await;
}

#[tokio::test]
async fn test_delete_player_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_delete_player(&db).await;
}

#[tokio::test]
async fn test_get_player_by_id_sqlite() {
    let db = sqlite_database().await;
    test_get_player_by_id(&db).await;
}

#[tokio::test]
async fn test_get_player_by_id_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_get_player_by_id(&db).await;
}

#[tokio::test]
async fn test_get_player_by_email_sqlite() {
    let db = sqlite_database().await;
    test_get_player_by_email(&db).await;
}

#[tokio::test]
async fn test_get_player_by_email_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_get_player_by_email(&db).await;
}

#[tokio::test]
async fn test_get_player_by_username_sqlite() {
    let db = sqlite_database().await;
    test_get_player_by_username(&db).await;
}

#[tokio::test]
async fn test_get_player_by_username_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_get_player_by_username(&db).await;
}

#[tokio::test]
async fn test_set_player_password_sqlite() {
    let db = sqlite_database().await;
    test_set_player_password(&db).await;
}

#[tokio::test]
async fn test_set_player_password_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_set_player_password(&db).await;
}

#[tokio::test]
async fn test_set_player_role_sqlite() {
    let db = sqlite_database().await;
    test_set_player_role(&db).await;
}

#[tokio::test]
async fn test_set_player_role_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_set_player_role(&db).await;
}

#[tokio::test]
async fn test_set_player_details_sqlite() {
    let db = sqlite_database().await;
    test_set_player_details(&db).await;
}

#[tokio::test]
async fn test_set_player_details_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_set_player_details(&db).await;
}

#[tokio::test]
async fn test_set_player_last_login_sqlite() {
    let db = sqlite_database().await;
    test_set_player_last_login(&db).await;
}

#[tokio::test]
async fn test_set_player_last_login_postgres() {
    let (db, _postgres) = postgres_database().await;
    test_set_player_last_login(&db).await;
}
