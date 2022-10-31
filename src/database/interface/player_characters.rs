use crate::blaze::errors::{BlazeError, BlazeResult};
use crate::blaze::SessionArc;
use crate::database::entities::{player_characters, players};
use crate::utils::conv::MEStringParser;
use log::warn;
use sea_orm::ActiveModelTrait;
use sea_orm::{
    ActiveValue::NotSet, ActiveValue::Set, ColumnTrait, DatabaseConnection, IntoActiveModel,
    ModelTrait, QueryFilter,
};

/// Attempts to find a player character relating to the provided player in the database
/// using its index and relation to the player. If None could be found a new value
/// will be created and returned instead.
async fn find(
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

/// Attempts to parse the provided player character data string and update the fields
/// on the provided active player character model. Will return a None option if parsing
/// failed.
fn parse(model: &mut player_characters::ActiveModel, value: &str) -> Option<()> {
    let mut parser = MEStringParser::new(value)?;
    model.kit_name = Set(parser.next_str()?);
    model.name = Set(parser.next()?);
    model.tint1 = Set(parser.next()?);
    model.tint2 = Set(parser.next()?);
    model.pattern = Set(parser.next()?);
    model.pattern_color = Set(parser.next()?);
    model.phong = Set(parser.next()?);
    model.emissive = Set(parser.next()?);
    model.skin_tone = Set(parser.next()?);
    model.seconds_played = Set(parser.next()?);
    model.timestamp_year = Set(parser.next()?);
    model.timestamp_month = Set(parser.next()?);
    model.timestamp_day = Set(parser.next()?);
    model.timestamp_seconds = Set(parser.next()?);
    model.powers = Set(parser.next_str()?);
    model.hotkeys = Set(parser.next_str()?);
    model.weapons = Set(parser.next_str()?);
    model.weapon_mods = Set(parser.next_str()?);
    model.deployed = Set(parser.next_bool()?);
    model.leveled_up = Set(parser.next_bool()?);
    Some(())
}

/// Attempts to parse the character index from the provided
/// character key. If the key is too short or doesn't contain
/// an index then an error is returned
fn parse_index(key: &str) -> BlazeResult<u16> {
    if key.len() <= 4 {
        return Err(BlazeError::Other("Player character key missing index"));
    }
    key[4..]
        .parse()
        .map_err(|_| BlazeError::Other("Player character key was not an integer"))
}

/// Attempts to update the player character stored at the provided index by
/// parsing the provided value and updating the database with any parsed changes.
pub async fn update(session: &SessionArc, key: &str, value: &str) -> BlazeResult<()> {
    let index = parse_index(key)?;
    let db = session.db();
    let session_data = session.data.read().await;

    let player = session_data.expect_player()?;
    let mut model = find(db, player, index).await?;
    if let None = parse(&mut model, value) {
        warn!("Failed to fully parse player character: {key} = {value}");
    }

    drop(session_data);

    model.save(db).await?;
    Ok(())
}

/// Encodes the provided player character model into the ME string
/// encoded value to send as apart of the settings map
pub fn encode(model: &player_characters::Model) -> String {
    format!(
        "20;4;{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}",
        model.kit_name,
        model.name,
        model.tint1,
        model.tint2,
        model.pattern,
        model.pattern_color,
        model.phong,
        model.emissive,
        model.skin_tone,
        model.seconds_played,
        model.timestamp_year,
        model.timestamp_month,
        model.timestamp_day,
        model.timestamp_seconds,
        model.powers,
        model.hotkeys,
        model.weapons,
        model.weapon_mods,
        if model.deployed { "True" } else { "False" },
        if model.leveled_up { "True" } else { "False" },
    )
}
