use crate::database::entities::{
    GalaxyAtWarActiveModel, GalaxyAtWarEntity, GalaxyAtWarModel, PlayerClassEntity, PlayerModel,
};
use crate::database::interface::players::{find_by_id, get_session_token};
use crate::{env, GlobalState};
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::web::{scope, Data, Path, Query, ServiceConfig};
use actix_web::{get, HttpResponse, Responder, ResponseError};
use chrono::Local;
use derive_more::{Display, From};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, IntoActiveModel, ModelTrait, NotSet};
use serde::Deserialize;
use std::cmp;
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

#[derive(Debug, Display, From)]
enum GAWError {
    #[display(fmt = "Invalid ID {}", _0)]
    InvalidID(ParseIntError),
    #[display(fmt = "Unknown ID")]
    UnknownID,
    #[display(fmt = "Database Error {}", _0)]
    DatabaseError(DbErr),
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
async fn gaw_player(db: &DatabaseConnection, id: &str) -> GAWResult<PlayerModel> {
    let id = u32::from_str_radix(id, 16)?;
    match find_by_id(db, id).await? {
        Some(value) => Ok(value),
        None => Err(GAWError::UnknownID),
    }
}

#[derive(Deserialize)]
struct AuthQuery {
    auth: String,
}

#[get("authentication/sharedTokenLogin")]
async fn authenticate(
    query: Query<AuthQuery>,
    global: Data<GlobalState>,
) -> GAWResult<impl Responder> {
    let player = gaw_player(&global.db, &query.auth).await?;
    let (player, token) = get_session_token(&global.db, player).await?;

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
    global: Data<GlobalState>,
) -> GAWResult<impl Responder> {
    let id = id.into_inner();
    let player = gaw_player(&global.db, &id).await?;

    let (gaw_data, promotions) = try_join!(
        get_galaxy_at_war(&global.db, &player),
        get_promotions(&global.db, &player)
    )?;

    let a = get_inc_value(gaw_data.group_a, &query.a);
    let b = get_inc_value(gaw_data.group_b, &query.b);
    let c = get_inc_value(gaw_data.group_c, &query.c);
    let d = get_inc_value(gaw_data.group_d, &query.d);
    let e = get_inc_value(gaw_data.group_e, &query.e);

    let mut gaw_data = gaw_data.into_active_model();

    gaw_data.group_a = Set(a);
    gaw_data.group_b = Set(b);
    gaw_data.group_c = Set(c);
    gaw_data.group_d = Set(d);
    gaw_data.group_e = Set(e);

    let gaw_data = gaw_data.update(&global.db).await?;

    Ok(ratings_response(promotions, gaw_data))
}

fn get_inc_value(old: u16, value: &Option<String>) -> u16 {
    match value {
        None => old,
        Some(value) => {
            let value = value.parse().unwrap_or(0);
            cmp::min(old + value, GAW_MAX_VALUE)
        }
    }
}

#[get("galaxyatwar/getRatings/{id}")]
async fn get_ratings(id: Path<String>, global: Data<GlobalState>) -> GAWResult<impl Responder> {
    let id = id.into_inner();
    let player = gaw_player(&global.db, &id).await?;

    let (gaw_data, promotions) = try_join!(
        get_galaxy_at_war(&global.db, &player),
        get_promotions(&global.db, &player)
    )?;

    Ok(ratings_response(promotions, gaw_data))
}

const DEFAULT_GAW_VALUE: u16 = 5000;
const GAW_MIN_VALUE: u16 = 5000;
const GAW_MAX_VALUE: u16 = 10099;

/// Attempts to find a galaxy at war data linked to the provided player
/// creating a new one if there is not already one.
async fn get_galaxy_at_war(
    db: &DatabaseConnection,
    player: &PlayerModel,
) -> GAWResult<GalaxyAtWarModel> {
    let existing = player.find_related(GalaxyAtWarEntity).one(db).await?;
    let current_time = Local::now().naive_local();
    match existing {
        None => {
            let gaw = GalaxyAtWarActiveModel {
                id: NotSet,
                player_id: Set(player.id),
                last_modified: Set(current_time),
                group_a: Set(DEFAULT_GAW_VALUE),
                group_b: Set(DEFAULT_GAW_VALUE),
                group_c: Set(DEFAULT_GAW_VALUE),
                group_d: Set(DEFAULT_GAW_VALUE),
                group_e: Set(DEFAULT_GAW_VALUE),
            };
            Ok(gaw.insert(db).await?)
        }
        Some(value) => apply_gaw_decay(db, value).await,
    }
}

/// Applies the galaxy at war decay values to the provided galaxy at war model to
/// ensure that the values accurately reflect the amount removed.
async fn apply_gaw_decay(
    db: &DatabaseConnection,
    value: GalaxyAtWarModel,
) -> GAWResult<GalaxyAtWarModel> {
    let decay = env::gaw_daily_decay();
    if decay <= 0.0 {
        return Ok(value);
    }

    let current_time = Local::now().naive_local();

    let days_passed = (current_time - value.last_modified).num_days() as f32;
    let decay_value = (decay * days_passed * 100.0) as u16;

    let a = cmp::max(value.group_a - decay_value, GAW_MIN_VALUE);
    let b = cmp::max(value.group_b - decay_value, GAW_MIN_VALUE);
    let c = cmp::max(value.group_c - decay_value, GAW_MIN_VALUE);
    let d = cmp::max(value.group_d - decay_value, GAW_MIN_VALUE);
    let e = cmp::max(value.group_e - decay_value, GAW_MIN_VALUE);

    let mut value = value.into_active_model();

    value.group_a = Set(a);
    value.group_b = Set(b);
    value.group_c = Set(c);
    value.group_d = Set(d);
    value.group_e = Set(e);

    let value = value.update(db).await?;

    Ok(value)
}

/// Returns a XML response generated for the provided ratings
fn ratings_response(promotions: u32, ratings: GalaxyAtWarModel) -> impl Responder {
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

/// Finds the total number of promotions the provided player has received on
/// all their classes. If promotions is disabled in the environment or if
/// an error occurs then zero is returned instead.
async fn get_promotions(db: &DatabaseConnection, player: &PlayerModel) -> GAWResult<u32> {
    let promotions = env::gaw_promotions();
    if !promotions {
        return Ok(0);
    }
    Ok(match player.find_related(PlayerClassEntity).all(db).await {
        Ok(classes) => classes.iter().map(|value| value.promotions).sum(),
        Err(_) => 0,
    })
}
