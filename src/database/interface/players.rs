use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter};
use sea_orm::ActiveValue::Set;
use crate::database::entities::{PlayerEntity, PlayerModel, players};
use crate::database::interface::DbResult;

type PlayerResult = DbResult<Option<PlayerModel>>;

pub async fn find_by_id(db: &DatabaseConnection, id: u32) -> PlayerResult {
    PlayerEntity::find_by_id(id)
        .one(db)
        .await
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

