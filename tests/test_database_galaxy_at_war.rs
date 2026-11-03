use chrono::Days;
use pocket_relay::database::DatabaseConnection;
use pocket_relay::database::entities::GalaxyAtWar;

use crate::common::player::quick_test_player;

mod common;

async fn test_galaxy_at_war_get(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let value = GalaxyAtWar::get(db, player.id, GalaxyAtWar::current_date_time())
        .await
        .unwrap();
    assert_eq!(value.group_a, 5000);
    assert_eq!(value.group_b, 5000);
    assert_eq!(value.group_c, 5000);
    assert_eq!(value.group_d, 5000);
    assert_eq!(value.group_e, 5000);
}

async fn test_galaxy_at_war_add(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let value = GalaxyAtWar::get(db, player.id, GalaxyAtWar::current_date_time())
        .await
        .unwrap();
    let value = value
        .add(db, [1, 2, 3, 4, 5], GalaxyAtWar::current_date_time())
        .await
        .unwrap();
    assert_eq!(value.group_a, 5001);
    assert_eq!(value.group_b, 5002);
    assert_eq!(value.group_c, 5003);
    assert_eq!(value.group_d, 5004);
    assert_eq!(value.group_e, 5005);
}

async fn test_galaxy_at_war_sub(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;
    let value = GalaxyAtWar::get(db, player.id, GalaxyAtWar::current_date_time())
        .await
        .unwrap();

    // Must add first as the minimum value cannot be exceeded
    let value = value
        .add(db, [5, 5, 5, 5, 5], GalaxyAtWar::current_date_time())
        .await
        .unwrap();

    let value = value
        .sub(db, [1, 2, 3, 4, 5], GalaxyAtWar::current_date_time())
        .await
        .unwrap();
    assert_eq!(value.group_a, 5004);
    assert_eq!(value.group_b, 5003);
    assert_eq!(value.group_c, 5002);
    assert_eq!(value.group_d, 5001);
    assert_eq!(value.group_e, 5000);
}

async fn test_galaxy_at_war_apply_decay(db: &DatabaseConnection) {
    let player = quick_test_player(db, "test@test.com").await;

    let start = GalaxyAtWar::current_date_time();
    let decay_days = 5;
    let decay_date = start
        .checked_add_days(Days::new(decay_days))
        .expect("date should be valid");

    let value = GalaxyAtWar::get(db, player.id, GalaxyAtWar::current_date_time())
        .await
        .unwrap();

    // Must add first as the minimum value cannot be exceeded
    let value = value
        .add(db, [500, 500, 500, 500, 500], start)
        .await
        .unwrap();

    let value = value.apply_decay(db, 0.5, decay_date).await.unwrap();

    // initial points = 5500
    // (0.5 * 5 days * 100) = 250 decay of points
    assert_eq!(value.group_a, 5250);
    assert_eq!(value.group_b, 5250);
    assert_eq!(value.group_c, 5250);
    assert_eq!(value.group_d, 5250);
    assert_eq!(value.group_e, 5250);
}

database_tests!(
    test_galaxy_at_war_get,
    test_galaxy_at_war_add,
    test_galaxy_at_war_sub,
    test_galaxy_at_war_apply_decay
);
