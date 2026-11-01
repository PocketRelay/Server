use pocket_relay::database::{DatabaseConnection, entities::PlayerData};

use crate::common::player::quick_test_player;

mod common;

async fn test_player_data_all(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    let all = PlayerData::all(db, player.id).await.unwrap();
    assert!(all.is_empty());

    PlayerData::set(db, player.id, "test".to_string(), "test_value".to_string())
        .await
        .unwrap();

    let all = PlayerData::all(db, player.id).await.unwrap();
    assert_eq!(all.len(), 1);
    let value = all.first().unwrap();

    assert_eq!(value.key, "test");
    assert_eq!(value.value, "test_value");
}

async fn test_player_data_set(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    PlayerData::set(db, player.id, "test".to_string(), "test_value".to_string())
        .await
        .unwrap();

    let value = PlayerData::get(db, player.id, "test")
        .await
        .unwrap()
        .expect("key should exist");

    assert_eq!(value.key, "test");
    assert_eq!(value.value, "test_value");
}

async fn test_player_data_set_bulk(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    let data = [
        ("test", "test_value"),
        ("test_2", "test_value_2"),
        ("test_3", "test_value_3"),
    ];

    PlayerData::set_bulk(
        db,
        player.id,
        data.iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .await
    .unwrap();

    for (key, value) in data {
        let data = PlayerData::get(db, player.id, key)
            .await
            .unwrap()
            .expect("key should exist");

        assert_eq!(data.key, key);
        assert_eq!(data.value, value);
    }

    let all = PlayerData::all(db, player.id).await.unwrap();
    assert_eq!(all.len(), data.len());

    for (key, value) in data {
        let item = all
            .iter()
            .find(|value| value.key == key)
            .expect("item should exist");

        assert_eq!(item.value, value);
    }
}

async fn test_player_data_set_replaces_duplicates(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    PlayerData::set(db, player.id, "test".to_string(), "test_value".to_string())
        .await
        .unwrap();

    PlayerData::set(
        db,
        player.id,
        "test".to_string(),
        "test_value_2".to_string(),
    )
    .await
    .unwrap();

    // Updated value should be present
    let value = PlayerData::get(db, player.id, "test")
        .await
        .unwrap()
        .expect("key should exist");

    assert_eq!(value.key, "test");
    assert_eq!(value.value, "test_value_2");

    // Length should remain unchanged
    let all = PlayerData::all(db, player.id).await.unwrap();
    assert_eq!(all.len(), 1);
}

async fn test_player_data_set_bulk_replaces_duplicates(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    let data = [
        ("test", "test_value"),
        ("test_2", "test_value_2"),
        ("test_3", "test_value_3"),
    ];

    let data_2 = [
        ("test", "test_value"),
        ("test_2", "test_value_4"),
        ("test_3", "test_value_5"),
    ];

    PlayerData::set_bulk(
        db,
        player.id,
        data.iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .await
    .unwrap();

    PlayerData::set_bulk(
        db,
        player.id,
        data_2
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .await
    .unwrap();

    for (key, value) in data_2 {
        let data = PlayerData::get(db, player.id, key)
            .await
            .unwrap()
            .expect("key should exist");

        assert_eq!(data.key, key);
        assert_eq!(data.value, value);
    }

    let all = PlayerData::all(db, player.id).await.unwrap();
    assert_eq!(all.len(), data_2.len());

    for (key, value) in data_2 {
        let item = all
            .iter()
            .find(|value| value.key == key)
            .expect("item should exist");

        assert_eq!(item.value, value);
    }
}

async fn test_player_data_delete(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let data = [
        ("test", "test_value"),
        ("test_2", "test_value_2"),
        ("test_3", "test_value_3"),
    ];
    PlayerData::set_bulk(
        db,
        player.id,
        data.iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .await
    .unwrap();

    PlayerData::delete(db, player.id, "test").await.unwrap();

    let player_data = PlayerData::get(db, player.id, "test").await.unwrap();
    assert!(player_data.is_none());

    let all = PlayerData::all(db, player.id).await.unwrap();
    assert_eq!(all.len(), 2);

    let player_data = all.iter().find(|value| value.key == "test");
    assert!(player_data.is_none());
}

async fn test_player_data_delete_non_existent(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let data = [
        ("test", "test_value"),
        ("test_2", "test_value_2"),
        ("test_3", "test_value_3"),
    ];
    PlayerData::set_bulk(
        db,
        player.id,
        data.iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .await
    .unwrap();

    PlayerData::delete(db, player.id, "test_unknown")
        .await
        .unwrap();

    let all = PlayerData::all(db, player.id).await.unwrap();
    assert_eq!(all.len(), data.len());
}

async fn test_player_data_get(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    PlayerData::set(db, player.id, "test".to_string(), "test_value".to_string())
        .await
        .unwrap();

    let value = PlayerData::get(db, player.id, "test")
        .await
        .unwrap()
        .expect("key should exist");

    assert_eq!(value.key, "test");
    assert_eq!(value.value, "test_value");
}

async fn test_player_data_classes(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let data = [
        ("not_a_class", "test_value"),
        ("class1", "test_value_2"),
        ("class2", "test_value_3"),
        ("some_item", "test_value_3"),
    ];
    PlayerData::set_bulk(
        db,
        player.id,
        data.iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
    .await
    .unwrap();

    let all = PlayerData::get_classes(db, player.id).await.unwrap();
    assert_eq!(all.len(), 2);

    for player_data in all {
        assert!(matches!(player_data.key.as_str(), "class1" | "class2"))
    }
}

database_tests!(
    test_player_data_all,
    test_player_data_set,
    test_player_data_set_bulk,
    test_player_data_set_replaces_duplicates,
    test_player_data_set_bulk_replaces_duplicates,
    test_player_data_delete,
    test_player_data_delete_non_existent,
    test_player_data_get,
    test_player_data_classes
);
