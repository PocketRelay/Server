use log::warn;
use sea_orm::ActiveModelTrait;
use sea_orm::{
    ActiveValue::NotSet, ActiveValue::Set, ColumnTrait, DatabaseConnection, IntoActiveModel,
    ModelTrait, QueryFilter,
};

use crate::blaze::errors::{BlazeError, BlazeResult};
use crate::blaze::session::SessionArc;
use crate::database::entities::{player_classes, players};
use utils::parse::MEStringParser;

/// Attempts to find a player class relating to the provided player in the database
/// using its index and relation to the player. If None could be found a new value
/// will be created and returned instead.
async fn find(
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

/// Attempts to parse the provided player character data string and update the fields
/// on the provided active player character model. Will return a None option if parsing
/// failed.
///
/// # Format
/// ```
/// 20;4;Adept;20;0;50
/// 20;4;NAME;LEVEL;EXP;PROMOTIONS
/// ```
fn parse(model: &mut player_classes::ActiveModel, value: &str) -> Option<()> {
    let mut parser = MEStringParser::new(value)?;
    model.name = Set(parser.next_str()?);
    model.level = Set(parser.next()?);
    model.exp = Set(parser.next()?);
    model.promotions = Set(parser.next()?);
    Some(())
}

/// Attempts to parse the class index from the provided
/// class key. If the key is too short or doesn't contain
/// an index then an error is returned
fn parse_index(key: &str) -> BlazeResult<u16> {
    if key.len() <= 5 {
        return Err(BlazeError::Other("Player class key missing index"));
    }
    key[5..]
        .parse()
        .map_err(|_| BlazeError::Other("Player class key was not an integer"))
}

pub async fn update_with(
    db: &DatabaseConnection,
    player: &players::Model,
    key: &str,
    value: &str,
) -> BlazeResult<()> {
    let index = parse_index(key)?;
    let mut model = find(db, player, index).await?;
    if let None = parse(&mut model, value) {
        warn!("Failed to fully parse player class: {key} = {value}");
    }
    model.save(db).await?;
    Ok(())
}

/// Attempts to update the player character stored at the provided index by
/// parsing the provided value and updating the database with any parsed changes.
pub async fn update(session: &SessionArc, key: &str, value: &str) -> BlazeResult<()> {
    let db = session.db();
    let session_data = session.data.read().await;
    let Some(player) = session_data.player.as_ref() else {
        warn!("Client attempted to update player class while not authenticated. (SID: {})", session.id);
        return Err(BlazeError::MissingPlayer);
    };
    update_with(db, player, key, value).await?;
    Ok(())
}

/// Encodes the provided player character model into the ME string
/// encoded value to send as apart of the settings map
pub fn encode(model: &player_classes::Model) -> String {
    format!(
        "20;4;{};{};{};{}",
        model.name, model.level, model.exp, model.promotions
    )
}
