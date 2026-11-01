use pocket_relay::database::{
    DatabaseConnection,
    entities::{LeaderboardData, Player, leaderboard_data::LeaderboardType},
};

use crate::common::player::{quick_test_player, quick_test_player_with_name};

mod common;

async fn test_leaderboard_count(db: &DatabaseConnection) {
    let count = LeaderboardData::count(db, LeaderboardType::N7Rating)
        .await
        .unwrap();
    assert_eq!(count, 0);

    let player = quick_test_player(db, "test@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 500)
        .await
        .unwrap();

    let count = LeaderboardData::count(db, LeaderboardType::N7Rating)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

async fn test_leaderboard_get_offset(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 500)
        .await
        .unwrap();

    let player_2 = quick_test_player(db, "test_2@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player_2.id, 700)
        .await
        .unwrap();

    let leaderboard = LeaderboardData::get_offset(db, LeaderboardType::N7Rating, 0, 50)
        .await
        .unwrap();

    assert_eq!(leaderboard.len(), 2);
    let first = leaderboard.first().unwrap();
    let second = leaderboard.get(1).unwrap();

    assert_eq!(first.player_id, player_2.id);
    assert_eq!(first.rank, 1);
    assert_eq!(first.value, 700);

    assert_eq!(second.player_id, player.id);
    assert_eq!(second.rank, 2);
    assert_eq!(second.value, 500);
}

async fn test_leaderboard_get_entry(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player.id)
        .await
        .unwrap();
    assert!(entry.is_none());

    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 500)
        .await
        .unwrap();

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player.id)
        .await
        .unwrap()
        .expect("entry should exist");

    assert_eq!(entry.player_id, player.id);
    assert_eq!(entry.value, 500);

    let entry = LeaderboardData::get_entry(db, LeaderboardType::ChallengePoints, player.id)
        .await
        .unwrap();
    assert!(entry.is_none());
}

async fn test_leaderboard_get_filtered(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 500)
        .await
        .unwrap();

    let player_2 = quick_test_player(db, "test_2@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player_2.id, 700)
        .await
        .unwrap();

    let leaderboard = LeaderboardData::get_filtered(db, LeaderboardType::N7Rating, vec![player.id])
        .await
        .unwrap();

    assert_eq!(leaderboard.len(), 1);
    let first = leaderboard.first().unwrap();

    assert_eq!(first.player_id, player.id);
    assert_eq!(first.rank, 2);
    assert_eq!(first.value, 500);
}

async fn test_leaderboard_get_centered(db: &DatabaseConnection) {
    let data = [
        ("test_1@test.com", 100),
        ("test_2@test.com", 200),
        ("test_3@test.com", 300),
        ("test_4@test.com", 400),
        ("test_5@test.com", 500),
        ("test_6@test.com", 600),
    ];

    let mut players: Vec<Player> = Vec::new();
    for (email, score) in data {
        let player = quick_test_player_with_name(db, email, email).await;
        LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, score)
            .await
            .unwrap();

        players.push(player);
    }

    let test_4 = players.get(3).unwrap();

    let centered = LeaderboardData::get_centered(db, LeaderboardType::N7Rating, test_4.id, 6)
        .await
        .unwrap()
        .expect("should have data");

    let expected_order = [
        "test_6@test.com",
        "test_5@test.com", // 2 before
        "test_4@test.com", // test 4 centered
        "test_3@test.com", // 3 after
        "test_2@test.com",
        "test_1@test.com",
    ];

    assert_eq!(centered.len(), expected_order.len());

    for (index, expected_order) in expected_order.iter().enumerate() {
        let value = centered.get(index).expect("item should exist");
        assert_eq!(value.player_name, *expected_order);
    }
}

async fn test_leaderboard_get_set(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 500)
        .await
        .unwrap();

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player.id)
        .await
        .unwrap()
        .expect("entry should exist");

    assert_eq!(entry.player_id, player.id);
    assert_eq!(entry.value, 500);
}

async fn test_leaderboard_get_set_ty_bulk(db: &DatabaseConnection) {
    let player_1 = quick_test_player(db, "test_1@test.com").await;
    let player_2 = quick_test_player(db, "test_2@test.com").await;

    LeaderboardData::set_ty_bulk(
        db,
        LeaderboardType::N7Rating,
        [(player_1.id, 500), (player_2.id, 700)].into_iter(),
    )
    .await
    .unwrap();

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player_1.id)
        .await
        .unwrap()
        .expect("entry should exist");

    assert_eq!(entry.player_id, player_1.id);
    assert_eq!(entry.value, 500);

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player_2.id)
        .await
        .unwrap()
        .expect("entry should exist");

    assert_eq!(entry.player_id, player_2.id);
    assert_eq!(entry.value, 700);
}

async fn test_leaderboard_get_set_update_existing(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 500)
        .await
        .unwrap();
    LeaderboardData::set(db, LeaderboardType::N7Rating, player.id, 700)
        .await
        .unwrap();

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player.id)
        .await
        .unwrap()
        .expect("entry should exist");

    assert_eq!(entry.player_id, player.id);
    assert_eq!(entry.value, 700);

    let count = LeaderboardData::count(db, LeaderboardType::N7Rating)
        .await
        .unwrap();

    assert_eq!(count, 1);
}

async fn test_leaderboard_get_set_ty_bulk_update_existing(db: &DatabaseConnection) {
    let player_1 = quick_test_player(db, "test_1@test.com").await;
    let player_2 = quick_test_player(db, "test_2@test.com").await;

    LeaderboardData::set_ty_bulk(
        db,
        LeaderboardType::N7Rating,
        [(player_1.id, 500), (player_2.id, 700)].into_iter(),
    )
    .await
    .unwrap();

    LeaderboardData::set_ty_bulk(
        db,
        LeaderboardType::N7Rating,
        [(player_1.id, 700), (player_2.id, 500)].into_iter(),
    )
    .await
    .unwrap();

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player_1.id)
        .await
        .unwrap()
        .expect("entry should exist");

    assert_eq!(entry.player_id, player_1.id);
    assert_eq!(entry.value, 700);

    let entry = LeaderboardData::get_entry(db, LeaderboardType::N7Rating, player_2.id)
        .await
        .unwrap()
        .expect("entry should exist");
    assert_eq!(entry.player_id, player_2.id);
    assert_eq!(entry.value, 500);
}

database_tests!(
    test_leaderboard_count,
    test_leaderboard_get_offset,
    test_leaderboard_get_entry,
    test_leaderboard_get_filtered,
    test_leaderboard_get_centered,
    test_leaderboard_get_set,
    test_leaderboard_get_set_ty_bulk,
    test_leaderboard_get_set_update_existing,
    test_leaderboard_get_set_ty_bulk_update_existing
);
