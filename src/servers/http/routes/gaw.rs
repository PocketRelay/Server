//! Routes for the Galaxy At War API used by the Mass Effect 3 client in order
//! to retrieve and increase the Galxay At War values for a player

use crate::{env, state::GlobalState};
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web::{Path, Query, ServiceConfig};
use actix_web::{get, HttpResponse, Responder, ResponseError};
use database::{DatabaseConnection, DbErr, GalaxyAtWar, Player};
use serde::Deserialize;
use std::fmt::Display;
use tokio::try_join;

/// Function for configuring the services in this route
///
/// `cfg` Service config to configure
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(shared_token_login)
        .service(get_ratings)
        .service(increase_ratings);
}

#[derive(Debug)]
enum GAWError {
    InvalidID,
    UnknownID,
    DatabaseError(DbErr),
}

type GAWResult<T> = Result<T, GAWError>;

/// Attempts to find a player in the database with a matching player ID
/// to the provided ID that is hex encoded.
///
/// `db` The database connection
/// `id` The hex encoded player ID
async fn get_player(db: &DatabaseConnection, id: &str) -> GAWResult<Player> {
    let id = match u32::from_str_radix(id, 16) {
        Ok(value) => value,
        Err(_) => return Err(GAWError::InvalidID),
    };
    let player = match Player::by_id(db, id).await? {
        Some(value) => value,
        None => return Err(GAWError::UnknownID),
    };
    Ok(player)
}

/// Query for authenticating with a shared login token. In this case
/// the shared login token is simply the hex encoded ID of the player
#[derive(Deserialize)]
struct AuthQuery {
    /// The authentication token
    auth: String,
}

#[get("gaw/authentication/sharedTokenLogin")]
async fn shared_token_login(query: Query<AuthQuery>) -> GAWResult<impl Responder> {
    let db = GlobalState::database();
    let player = get_player(db, &query.auth).await?;
    let (player, token) = player.with_token(db).await?;

    let id = player.id;
    let sess = format!("{:x}", id);
    let display_name = player.display_name;

    let response = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<fulllogin>
    <canageup>0</canageup>
    <legaldochost/>
    <needslegaldoc>0</needslegaldoc>
    <pclogintoken>{token}</pclogintoken>
    <privacypolicyuri/>
    <sessioninfo>
        <blazeuserid>{id}</blazeuserid>
        <isfirstlogin>0</isfirstlogin>
        <sessionkey>{sess}</sessionkey>
        <lastlogindatetime>1422639771</lastlogindatetime>
        <email>test@test.com</email>
        <personadetails>
            <displayname>{display_name}</displayname>
            <lastauthenticated>1422639540</lastauthenticated>
            <personaid>{id}</personaid>
            <status>UNKNOWN</status>
            <extid>0</extid>
            <exttype>BLAZE_EXTERNAL_REF_TYPE_UNKNOWN</exttype>
        </personadetails>
        <userid>{id}</userid>
    </sessioninfo>
    <isoflegalcontactage>0</isoflegalcontactage>
    <toshost/>
    <termsofserviceuri/>
    <tosuri/>
</fulllogin>"#
    );

    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::xml())
        .body(response))
}

/// Retrieves the galaxy at war data and promotions count for
/// the player with the provided ID
///
/// `db` The dataabse connection
/// `id` The hex ID of the player
async fn get_player_gaw_data(db: &DatabaseConnection, id: &str) -> GAWResult<(GalaxyAtWar, u32)> {
    let player = get_player(db, id).await?;
    let gaw_task = GalaxyAtWar::find_or_create(db, &player, env::from_env(env::GAW_DAILY_DECAY));
    let promotions_task = async {
        Ok(if env::from_env(env::GAW_PROMOTIONS) {
            GalaxyAtWar::find_promotions(db, &player).await
        } else {
            0
        })
    };

    let (gaw_data, promotions) = try_join!(gaw_task, promotions_task)?;
    Ok((gaw_data, promotions))
}

/// Route for retrieving the galaxy at war ratings for the player
/// with the provied ID
///
/// `id` The hex encoded ID of the player
#[get("gaw/galaxyatwar/getRatings/{id}")]
async fn get_ratings(id: Path<String>) -> GAWResult<impl Responder> {
    let db = GlobalState::database();
    let id = id.into_inner();
    let (gaw_data, promotions) = get_player_gaw_data(db, &id).await?;
    ratings_response(gaw_data, promotions)
}

/// The query structure for increasing the
#[derive(Deserialize)]
struct IncreaseQuery {
    /// The increase amount for the first region
    #[serde(rename = "rinc|0", default)]
    a: u16,
    /// The increase amount for the second region
    #[serde(rename = "rinc|1", default)]
    b: u16,
    /// The increase amount for the third region
    #[serde(rename = "rinc|2", default)]
    c: u16,
    /// The increase amount for the fourth region
    #[serde(rename = "rinc|3", default)]
    d: u16,
    /// The increase amount for the fifth region
    #[serde(rename = "rinc|4", default)]
    e: u16,
}

/// Route for increasing the galaxy at war ratings for the player with
/// the provided ID will respond with the new ratings after increasing
/// them.
///
/// `id`    The hex encoded ID of the player
/// `query` The query data containing the increase values
#[get("gaw/galaxyatwar/increaseRatings/{id}")]
async fn increase_ratings(
    id: Path<String>,
    query: Query<IncreaseQuery>,
) -> GAWResult<impl Responder> {
    let db = GlobalState::database();
    let id = id.into_inner();
    let (gaw_data, promotions) = get_player_gaw_data(db, &id).await?;
    let gaw_data = gaw_data
        .increase(db, (query.a, query.b, query.c, query.d, query.e))
        .await?;
    ratings_response(gaw_data, promotions)
}

/// Generates a ratings XML response from the provided ratings struct and
/// promotions value.
///
/// `ratings`    The galaxy at war ratings value
/// `promotions` The promotions value
fn ratings_response(ratings: GalaxyAtWar, promotions: u32) -> GAWResult<impl Responder> {
    let a = ratings.group_a;
    let b = ratings.group_b;
    let c = ratings.group_c;
    let d = ratings.group_d;
    let e = ratings.group_e;
    let level = (a + b + c + d + e) / 5;
    let response = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<galaxyatwargetratings>
    <ratings>
        <ratings>{a}</ratings>
        <ratings>{b}</ratings>
        <ratings>{c}</ratings>
        <ratings>{d}</ratings>
        <ratings>{e}</ratings>
    </ratings>
    <level>{level}</level>
    <assets>
        <assets>{promotions}</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
        <assets>0</assets>
    </assets>
</galaxyatwargetratings>
"#
    );
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::xml())
        .body(response))
}

impl Display for GAWError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidID => f.write_str("Invalid ID"),
            Self::UnknownID => f.write_str("Unknown ID"),
            Self::DatabaseError(_) => f.write_str("Database Error"),
        }
    }
}

impl From<DbErr> for GAWError {
    fn from(err: DbErr) -> Self {
        GAWError::DatabaseError(err)
    }
}

impl ResponseError for GAWError {
    fn status_code(&self) -> StatusCode {
        match self {
            GAWError::InvalidID | GAWError::UnknownID => StatusCode::BAD_REQUEST,
            GAWError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
