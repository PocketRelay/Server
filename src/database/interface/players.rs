use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, NotSet, QueryFilter};
use sea_orm::ActiveValue::Set;
use crate::database::entities::{PlayerActiveModel, PlayerEntity, PlayerModel, players};
use crate::database::interface::DbResult;

type PlayerResult = DbResult<Option<PlayerModel>>;

pub async fn find_by_id(db: &DatabaseConnection, id: u32) -> PlayerResult {
    PlayerEntity::find_by_id(id)
        .one(db)
        .await
}

pub async fn create_normal(db: &DatabaseConnection, email: String, password: String) -> DbResult<PlayerModel> {
    let display_name = if email.len() > 99 { email[0..99].to_string() } else { email.clone() };
    let active_model = PlayerActiveModel {
        id: NotSet,
        email: Set(email.to_string()),
        display_name: Set(display_name),
        session_token: NotSet,
        origin: Set(false),
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

pub async fn find_by_email(db: &DatabaseConnection, email: &str) -> PlayerResult {
    PlayerEntity::find()
        .filter(players::Column::Email.eq(email))
        .one(db)
        .await
}

pub async fn find_by_session(db: &DatabaseConnection, session_token: &str) -> PlayerResult {
    PlayerEntity::find()
        .filter(players::Column::SessionToken.eq(session_token))
        .one(db)
        .await
}

pub async fn set_session_token(db: &DatabaseConnection, player: PlayerModel, session_token: Option<String>) -> DbResult<PlayerModel> {
    let mut active = player.into_active_model();
    active.session_token = Set(session_token);
    active.update(db).await
}

