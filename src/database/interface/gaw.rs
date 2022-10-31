use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, DatabaseConnection, IntoActiveModel, ModelTrait, Set,
};
use std::cmp;

use super::DbResult;
use crate::database::entities::{galaxy_at_war, player_classes, players};

const DEFAULT_GAW_VALUE: u16 = 5000;
pub const GAW_MIN_VALUE: u16 = 5000;
pub const GAW_MAX_VALUE: u16 = 10099;

pub async fn find_promotions(
    db: &DatabaseConnection,
    player: &players::Model,
    enabled: bool,
) -> DbResult<u32> {
    if !enabled {
        return Ok(0);
    }
    match player.find_related(player_classes::Entity).all(db).await {
        Ok(classes) => Ok(classes.iter().map(|value| value.promotions).sum()),
        Err(_) => Ok(0),
    }
}

pub async fn find_or_create(
    db: &DatabaseConnection,
    player: &players::Model,
    decay: f32,
) -> DbResult<galaxy_at_war::Model> {
    let existing = player.find_related(galaxy_at_war::Entity).one(db).await?;

    if let Some(existing) = existing {
        apply_gaw_decay(db, existing, decay).await
    } else {
        let current_time = Local::now().naive_local();
        let model = galaxy_at_war::ActiveModel {
            id: NotSet,
            player_id: Set(player.id),
            last_modified: Set(current_time),
            group_a: Set(DEFAULT_GAW_VALUE),
            group_b: Set(DEFAULT_GAW_VALUE),
            group_c: Set(DEFAULT_GAW_VALUE),
            group_d: Set(DEFAULT_GAW_VALUE),
            group_e: Set(DEFAULT_GAW_VALUE),
        };
        model.insert(db).await
    }
}

pub async fn increase_gaw(
    db: &DatabaseConnection,
    value: galaxy_at_war::Model,
    a: u16,
    b: u16,
    c: u16,
    d: u16,
    e: u16,
) -> DbResult<galaxy_at_war::Model> {
    let mut gaw_data = value.into_active_model();
    gaw_data.group_a = Set(a);
    gaw_data.group_b = Set(b);
    gaw_data.group_c = Set(c);
    gaw_data.group_d = Set(d);
    gaw_data.group_e = Set(e);
    gaw_data.update(db).await
}

async fn apply_gaw_decay(
    db: &DatabaseConnection,
    value: galaxy_at_war::Model,
    decay: f32,
) -> DbResult<galaxy_at_war::Model> {
    // Ignore if decay is negative or zero
    if decay <= 0.0 {
        return Ok(value);
    }

    // Calculate decay value from days passed
    let current_time = Local::now().naive_local();
    let days_passed = (current_time - value.last_modified).num_days() as f32;
    let decay_value = (decay * days_passed * 100.0) as u16;

    // Apply decay while keeping minimum
    let a = cmp::max(value.group_a - decay_value, GAW_MIN_VALUE);
    let b = cmp::max(value.group_b - decay_value, GAW_MIN_VALUE);
    let c = cmp::max(value.group_c - decay_value, GAW_MIN_VALUE);
    let d = cmp::max(value.group_d - decay_value, GAW_MIN_VALUE);
    let e = cmp::max(value.group_e - decay_value, GAW_MIN_VALUE);

    // Update stored copy
    let mut value = value.into_active_model();
    value.group_a = Set(a);
    value.group_b = Set(b);
    value.group_c = Set(c);
    value.group_d = Set(d);
    value.group_e = Set(e);

    value.update(db).await
}
