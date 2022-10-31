use log::warn;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

use crate::{
    blaze::{errors::BlazeResult, SessionArc},
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
    let player = session_data.expect_player_owned()?;
    let mut model = player.into_active_model();
    modify(&mut model, key, value);
    let player = model.update(session.db()).await?;
    session_data.player = Some(player);
    Ok(())
}
