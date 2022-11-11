use crate::database::entities::{player_characters, player_classes, players};
use crate::database::interface::DbResult;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait,
    NotSet, QueryFilter,
};
use utils::random::generate_token;

type PlayerResult = DbResult<Option<players::Model>>;

pub async fn find_by_id(db: &DatabaseConnection, id: u32) -> PlayerResult {
    players::Entity::find_by_id(id).one(db).await
}

pub async fn create(
    db: &DatabaseConnection,
    email: String,
    display_name: String,
    password: String,
    origin: bool,
) -> DbResult<players::Model> {
    let active_model = players::ActiveModel {
        id: NotSet,
        email: Set(email.to_string()),
        display_name: Set(display_name),
        session_token: NotSet,
        origin: Set(origin),
        password: Set(password),
        credits: NotSet,
        credits_spent: NotSet,
        games_played: NotSet,
        seconds_played: NotSet,
        inventory: Set(String::new()),
        csreward: NotSet,
        face_codes: NotSet,
        new_item: NotSet,
        completion: NotSet,
        progress: NotSet,
        cs_completion: NotSet,
        cs_timestamps1: NotSet,
        cs_timestamps2: NotSet,
        cs_timestamps3: NotSet,
    };
    active_model.insert(db).await
}

pub async fn find_by_email(db: &DatabaseConnection, email: &str, origin: bool) -> PlayerResult {
    players::Entity::find()
        .filter(
            players::Column::Email
                .eq(email)
                .and(players::Column::Origin.eq(origin)),
        )
        .one(db)
        .await
}

pub async fn find_by_email_any(db: &DatabaseConnection, email: &str) -> PlayerResult {
    players::Entity::find()
        .filter(players::Column::Email.eq(email))
        .one(db)
        .await
}

pub async fn find_by_session(db: &DatabaseConnection, session_token: &str) -> PlayerResult {
    players::Entity::find()
        .filter(players::Column::SessionToken.eq(session_token))
        .one(db)
        .await
}

pub async fn set_session_token(
    db: &DatabaseConnection,
    player: players::Model,
    session_token: String,
) -> DbResult<(players::Model, String)> {
    let mut active = player.into_active_model();
    active.session_token = Set(Some(session_token.clone()));
    let player = active.update(db).await?;
    Ok((player, session_token))
}

pub async fn get_session_token(
    db: &DatabaseConnection,
    player: players::Model,
) -> DbResult<(players::Model, String)> {
    let token = match &player.session_token {
        None => {
            let token = generate_token(128);
            let out = set_session_token(db, player, token).await?;
            return Ok(out);
        }
        Some(value) => value.clone(),
    };
    Ok((player, token))
}

/// Finds all the player class entities related to the provided player
/// and returns them in a Vec
pub async fn find_classes(
    db: &DatabaseConnection,
    player: &players::Model,
) -> DbResult<Vec<player_classes::Model>> {
    player.find_related(player_classes::Entity).all(db).await
}

/// Finds all the player character entities related to the provided player
/// and returns them in a Vec
pub async fn find_characters(
    db: &DatabaseConnection,
    player: &players::Model,
) -> DbResult<Vec<player_characters::Model>> {
    player.find_related(player_characters::Entity).all(db).await
}
