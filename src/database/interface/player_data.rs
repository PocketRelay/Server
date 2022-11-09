use blaze_pk::TdfMap;
use log::warn;
use sea_orm::{ActiveModelTrait, DatabaseConnection, IntoActiveModel, Set};

use crate::{
    blaze::{
        errors::{BlazeError, BlazeResult},
        SessionArc,
    },
    database::entities::players,
    utils::conv::MEStringParser,
};

pub fn encode_base(model: &players::Model) -> String {
    format!(
        "20;4;{};-1;0;{};0;{};{};0;{}",
        model.credits,
        model.credits_spent,
        model.games_played,
        model.seconds_played,
        model.inventory
    )
}

/// Attempts to parse the provided player base data string and update the fields
/// on the provided active player model. Will return a None option if parsing
/// failed.
///
/// # Format
/// ```
/// 20;4;21474;-1;0;0;0;50;180000;0;fff....(LARGE SEQUENCE OF INVENTORY CHARS)
/// 20;4;CREDITS;UNKNOWN;UKNOWN;CREDITS_SPENT;UKNOWN;GAMES_PLAYED;SECONDS_PLAYED;UKNOWN;INVENTORY
/// ```
fn parse_base(model: &mut players::ActiveModel, value: &str) -> Option<()> {
    let mut parser = MEStringParser::new(value)?;
    model.credits = Set(parser.next()?);
    parser.skip(2); // Skip -1;0
    model.credits_spent = Set(parser.next()?);
    parser.skip(1)?;
    model.games_played = Set(parser.next()?);
    model.seconds_played = Set(parser.next()?);
    parser.skip(1);
    model.inventory = Set(parser.next_str()?);
    Some(())
}

fn modify(model: &mut players::ActiveModel, key: &str, value: String) {
    match key {
        "Base" => {
            if let None = parse_base(model, &value) {
                warn!("Failed to completely parse player base")
            };
        }
        "FaceCodes" => model.face_codes = Set(Some(value)),
        "NewItem" => model.new_item = Set(Some(value)),
        "csreward" => {
            let value = value.parse::<u16>().unwrap_or(0);
            model.csreward = Set(value)
        }
        "Completion" => model.completion = Set(Some(value)),
        "Progress" => model.progress = Set(Some(value)),
        "cscompletion" => model.cs_completion = Set(Some(value)),
        "cstimestamps" => model.cs_timestamps1 = Set(Some(value)),
        "cstimestamps2" => model.cs_timestamps2 = Set(Some(value)),
        "cstimestamps3" => model.cs_timestamps3 = Set(Some(value)),
        _ => {}
    }
}

/// Updates the player model stored on this session with the provided key value data pair
/// persisting the changes to the database and updating the stored model.
pub async fn update(session: &SessionArc, key: &str, value: String) -> BlazeResult<()> {
    let mut session_data = session.data.write().await;
    let Some(player) = session_data.player.take() else{return Err(BlazeError::MissingPlayer);};
    let mut model = player.into_active_model();
    modify(&mut model, key, value);
    let player = model.update(session.db()).await?;
    session_data.player = Some(player);
    Ok(())
}

pub async fn update_all(
    db: &DatabaseConnection,
    player: players::Model,
    values: TdfMap<String, String>,
) -> BlazeResult<players::Model> {
    let mut others = Vec::new();
    for (key, value) in values {
        if key.starts_with("class") {
            super::player_classes::update_with(db, &player, &key, &value)
                .await
                .map_err(|err| err.context("While updating player class"))
                .ok();
        } else if key.starts_with("char") {
            super::player_characters::update_with(db, &player, &key, &value)
                .await
                .map_err(|err| err.context("While updating player character"))
                .ok();
        } else {
            others.push((key, value));
        }
    }
    if others.len() > 0 {
        let mut model = player.into_active_model();
        for (key, value) in others {
            modify(&mut model, &key, value);
        }
        let model = model.update(db).await?;
        Ok(model)
    } else {
        Ok(player)
    }
}
