use sea_orm::{
    ActiveValue::NotSet, ActiveValue::Set, ColumnTrait, DatabaseConnection, IntoActiveModel,
    ModelTrait, QueryFilter,
};

use crate::blaze::errors::BlazeResult;
use crate::database::entities::{player_characters, player_classes, players};

/// Attempts to find a player class relating to the provided player in the database
/// using its index and relation to the player. If None could be found a new value
/// will be created and returned instead.
pub async fn find_player_class(
    db: &DatabaseConnection,
    player: &players::Model,
    index: u16,
) -> BlazeResult<player_classes::ActiveModel> {
    let player_class = player
        .find_related(player_classes::Entity)
        .filter(player_classes::Column::Index.eq(index))
        .one(db)
        .await?;
    if let Some(player_class) = player_class {
        return Ok(player_class.into_active_model());
    }
    Ok(player_classes::ActiveModel {
        id: NotSet,
        player_id: Set(player.id),
        index: Set(index),
        name: NotSet,
        level: NotSet,
        exp: NotSet,
        promotions: NotSet,
    })
}

/// Attempts to find a player character relating to the provided player in the database
/// using its index and relation to the player. If None could be found a new value
/// will be created and returned instead.
pub async fn find_player_character(
    db: &DatabaseConnection,
    player: &players::Model,
    index: u16,
) -> BlazeResult<player_characters::ActiveModel> {
    let player_character = player
        .find_related(player_characters::Entity)
        .filter(player_characters::Column::Index.eq(index))
        .one(db)
        .await?;

    if let Some(player_character) = player_character {
        return Ok(player_character.into_active_model());
    }

    Ok(player_characters::ActiveModel {
        id: NotSet,
        player_id: Set(player.id),
        index: Set(index),
        kit_name: NotSet,
        name: NotSet,
        tint1: NotSet,
        tint2: NotSet,
        pattern: NotSet,
        pattern_color: NotSet,
        phong: NotSet,
        emissive: NotSet,
        skin_tone: NotSet,
        seconds_played: NotSet,
        timestamp_year: NotSet,
        timestamp_month: NotSet,
        timestamp_day: NotSet,
        timestamp_seconds: NotSet,
        powers: NotSet,
        hotkeys: NotSet,
        weapons: NotSet,
        weapon_mods: NotSet,
        deployed: NotSet,
        leveled_up: NotSet,
    })
}
