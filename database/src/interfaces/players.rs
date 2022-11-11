use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{entities::players, Database, DbResult};

pub struct PlayersInterface;

impl PlayersInterface {
    /// Creates a new player with the proivded details and inserts
    /// it into the database
    ///
    /// `db`           The database instance
    /// `email`        The player account email
    /// `display_name` The player display name
    /// `password`     The hashed player password
    /// `origin`       Whether the account is an origin account
    pub async fn create(
        db: &Database,
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

    /// Attempts to find a player with the provided ID will return none
    /// if there was no players with that ID
    ///
    /// `db` The database instance
    /// `id` The ID of the player to find
    pub async fn by_id(db: &Database, id: u32) -> DbResult<Option<players::Model>> {
        players::Entity::find_by_id(id).one(&db.connection).await
    }

    /// Attempts to find a player with the provided email. Conditional
    /// check for whether to allow origin accounts in the search.
    pub async fn by_email(
        db: &Database,
        email: &str,
        origin: bool,
    ) -> DbResult<Option<players::Model>> {
        players::Entity::find()
            .filter(
                players::Column::Email
                    .eq(email)
                    .and(players::Column::Origin.eq(origin)),
            )
            .one(&db.connection)
            .await
    }

    /// Checks whether the provided email address is taken by any
    /// accounts in the database including origin accounts.
    ///
    /// `db`    The datbase instance
    /// `email` The email to check for
    ///
    pub async fn is_email_taken(db: &Database, email: &str) -> DbResult<bool> {
        players::Entity::find()
            .filter(players::Column::Email.eq(email))
            .one(&db.connection)
            .await
            .map(|value| value.is_some())
    }

    /// Attempts to find a player by the provided session token
    ///
    /// `db`    The database instance
    /// `token` The session token to search for
    pub async fn by_token(db: &Database, token: &str) -> DbResult<Option<players::Model>> {
        players::Entity::find()
            .filter(players::Column::SessionToken.eq(token))
            .one(&db.connection)
            .await
    }
}
