use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web::{scope, Path, Query, ServiceConfig};
use actix_web::{get, HttpResponse, Responder, ResponseError};
use core::{env, state::GlobalState};
use database::{
    galaxy_at_war, players, DatabaseConnection, DbErr, DbResult, GalaxyAtWarInterface,
    PlayersInterface,
};
use serde::Deserialize;
use std::fmt::Display;
use std::num::ParseIntError;
use tokio::try_join;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        scope("gaw")
            .service(authenticate)
            .service(get_ratings)
            .service(increase_ratings),
    );
}

#[derive(Debug)]
enum GAWError {
    InvalidID(ParseIntError),
    UnknownID,
    DatabaseError(DbErr),
}

impl Display for GAWError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidID(_) => f.write_str("Invalid ID"),
            Self::UnknownID => f.write_str("Unknown ID"),
            Self::DatabaseError(_) => f.write_str("Database Error"),
        }
    }
}

impl From<ParseIntError> for GAWError {
    fn from(err: ParseIntError) -> Self {
        GAWError::InvalidID(err)
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
            GAWError::InvalidID(_) | GAWError::UnknownID => StatusCode::BAD_REQUEST,
            GAWError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

type GAWResult<T> = Result<T, GAWError>;

/// Attempts to find a player from the provided GAW ID
async fn gaw_player(db: &DatabaseConnection, id: &str) -> GAWResult<players::Model> {
    let id = u32::from_str_radix(id, 16)?;
    match PlayersInterface::by_id(db, id).await? {
        Some(value) => Ok(value),
        None => Err(GAWError::UnknownID),
    }
}

#[derive(Deserialize)]
struct AuthQuery {
    auth: String,
}

#[get("authentication/sharedTokenLogin")]
async fn authenticate(query: Query<AuthQuery>) -> GAWResult<impl Responder> {
    let db = GlobalState::database();
    let player = gaw_player(db, &query.auth).await?;
    let (player, token) = PlayersInterface::get_token(db, player).await?;

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

#[derive(Deserialize)]
struct IncreaseQuery {
    #[serde(rename = "rinc|0")]
    a: Option<String>,
    #[serde(rename = "rinc|1")]
    b: Option<String>,
    #[serde(rename = "rinc|2")]
    c: Option<String>,
    #[serde(rename = "rinc|3")]
    d: Option<String>,
    #[serde(rename = "rinc|4")]
    e: Option<String>,
}

#[get("galaxyatwar/increaseRatings/{id}")]
async fn increase_ratings(
    id: Path<String>,
    query: Query<IncreaseQuery>,
) -> GAWResult<impl Responder> {
    let db = GlobalState::database();
    let id = id.into_inner();
    let player = gaw_player(db, &id).await?;

    let (gaw_data, promotions) = try_join!(
        GalaxyAtWarInterface::find_or_create(db, &player, env::from_env(env::GAW_DAILY_DECAY)),
        get_promotions(db, &player, env::from_env(env::GAW_PROMOTIONS))
    )?;

    let a = get_inc_value(gaw_data.group_a, &query.a);
    let b = get_inc_value(gaw_data.group_b, &query.b);
    let c = get_inc_value(gaw_data.group_c, &query.c);
    let d = get_inc_value(gaw_data.group_d, &query.d);
    let e = get_inc_value(gaw_data.group_e, &query.e);
    let gaw_data = GalaxyAtWarInterface::increase(db, gaw_data, (a, b, c, d, e)).await?;
    Ok(ratings_response(promotions, gaw_data))
}

fn get_inc_value(old: u16, value: &Option<String>) -> u16 {
    match value {
        None => old,
        Some(value) => {
            let value = value.parse().unwrap_or(0);
            old + value
        }
    }
}

#[get("galaxyatwar/getRatings/{id}")]
async fn get_ratings(id: Path<String>) -> GAWResult<impl Responder> {
    let db = GlobalState::database();
    let id = id.into_inner();
    let player = gaw_player(db, &id).await?;

    let (gaw_data, promotions) = try_join!(
        GalaxyAtWarInterface::find_or_create(db, &player, env::from_env(env::GAW_DAILY_DECAY)),
        get_promotions(db, &player, env::from_env(env::GAW_PROMOTIONS))
    )?;

    Ok(ratings_response(promotions, gaw_data))
}

async fn get_promotions(
    db: &DatabaseConnection,
    player: &players::Model,
    enabled: bool,
) -> DbResult<u32> {
    if !enabled {
        return Ok(0);
    }
    Ok(GalaxyAtWarInterface::find_promotions(db, &player).await)
}

/// Returns a XML response generated for the provided ratings
fn ratings_response(promotions: u32, ratings: galaxy_at_war::Model) -> impl Responder {
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
    HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::xml())
        .body(response)
}
