//! Routes for the Galaxy At War API used by the Mass Effect 3 client in order
//! to retrieve and increase the Galxay At War values for a player.
//!
//! This API is not documented as it is not intended to be used by anyone
//! other than the Mass Effect 3 client itself.

use crate::{
    config::RuntimeConfig,
    database::{
        entities::{GalaxyAtWar, Player, PlayerData},
        DatabaseConnection, DbErr, DbResult,
    },
    middleware::xml::Xml,
    services::sessions::Sessions,
    utils::parsing::PlayerClass,
};
use axum::{
    extract::{Path, Query},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use indoc::formatdoc;
use serde::Deserialize;
use std::{fmt::Display, sync::Arc};
use tokio::try_join;

/// Error type used in gaw routes to handle errors such
/// as being unable to parse player IDs, find players
/// or Database errors
pub enum GAWError {
    /// The player could not be found
    InvalidToken,
    /// There was a server error
    ServerError,
}

/// Query for authenticating with a shared login token.
#[derive(Deserialize)]
pub struct AuthQuery {
    /// The authentication token (This is just a hex encoded player ID)
    auth: String,
}

/// GET /authentication/sharedTokenLogin
///
/// Route for handling shared token login. In the official implementation this
/// would login the client using the shared token provided by the Main server.
/// But this implementation just responds with the bare minimum response directly
/// passing the auth key as the session token for further requests
///
/// Note: Many fields here have their values ommitted compared to the
/// actual response. This is because these are not needed to function
/// so not nessicary to implement the fetching
///
/// `query` The query containing the auth token (In this case the hex player ID)
pub async fn shared_token_login(Query(AuthQuery { auth }): Query<AuthQuery>) -> Xml {
    Xml(formatdoc! {r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <fulllogin>
            <canageup>0</canageup>
            <legaldochost/>
            <needslegaldoc>0</needslegaldoc>
            <pclogintoken/>
            <privacypolicyuri/>
            <sessioninfo>
                <blazeuserid/>
                <isfirstlogin>0</isfirstlogin>
                <sessionkey>{}</sessionkey>
                <lastlogindatetime/>
                <email/>
                <personadetails>
                    <displayname/>
                    <lastauthenticated/>
                    <personaid/>
                    <status>UNKNOWN</status>
                    <extid>0</extid>
                    <exttype>BLAZE_EXTERNAL_REF_TYPE_UNKNOWN</exttype>
                </personadetails>
                <userid/>
            </sessioninfo>
            <isoflegalcontactage>0</isoflegalcontactage>
            <toshost/>
            <termsofserviceuri/>
            <tosuri/>
        </fulllogin>
    "# ,auth})
}

/// GET /galaxyatwar/getRatings/:id
///
/// Route for retrieving the galaxy at war ratings for the player
/// with the provied ID
///
/// `id` The hex encoded ID of the player
pub async fn get_ratings(
    Path(id): Path<String>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<RuntimeConfig>>,
    Extension(sessions): Extension<Arc<Sessions>>,
) -> Result<Xml, GAWError> {
    let (gaw_data, promotions) = get_player_gaw_data(&db, sessions, &id, &config).await?;
    Ok(ratings_response(gaw_data, promotions))
}

/// The query structure for increasing the galaxy at war values
/// for a player
#[derive(Deserialize)]
pub struct IncreaseQuery {
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

/// GET /galaxyatwar/increaseRatings/:id
///
/// Route for increasing the galaxy at war ratings for the player with
/// the provided ID will respond with the new ratings after increasing
/// them.
///
/// `id`    The hex encoded ID of the player
/// `query` The query data containing the increase values
pub async fn increase_ratings(
    Path(id): Path<String>,
    Query(IncreaseQuery { a, b, c, d, e }): Query<IncreaseQuery>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<RuntimeConfig>>,
    Extension(sessions): Extension<Arc<Sessions>>,
) -> Result<Xml, GAWError> {
    let (gaw_data, promotions) = get_player_gaw_data(&db, sessions, &id, &config).await?;
    let gaw_data = gaw_data.add(&db, [a, b, c, d, e]).await?;
    Ok(ratings_response(gaw_data, promotions))
}

/// Retrieves the galaxy at war data and promotions count for
/// the player with the provided ID
///
/// `db` The dataabse connection
/// `id` The hex ID of the player
async fn get_player_gaw_data(
    db: &DatabaseConnection,
    sessions: Arc<Sessions>,
    token: &str,
    config: &RuntimeConfig,
) -> Result<(GalaxyAtWar, u32), GAWError> {
    let player_id = sessions
        .verify_token(token)
        .map_err(|_| GAWError::InvalidToken)?;

    let player = Player::by_id(db, player_id)
        .await?
        .ok_or(GAWError::InvalidToken)?;

    let (gaw_data, promotions) = try_join!(
        GalaxyAtWar::get(db, player.id),
        get_promotions(db, &player, config)
    )?;
    let gaw_data = gaw_data.apply_decay(db, config.galaxy_at_war.decay).await?;

    Ok((gaw_data, promotions))
}

async fn get_promotions(
    db: &DatabaseConnection,
    player: &Player,
    config: &RuntimeConfig,
) -> DbResult<u32> {
    if !config.galaxy_at_war.promotions {
        return Ok(0);
    }

    Ok(PlayerData::get_classes(db, player.id)
        .await?
        .iter()
        .filter_map(|value| PlayerClass::parse(&value.value))
        .map(|value| value.promotions)
        .sum())
}

/// Generates a ratings XML response from the provided ratings struct and
/// promotions value.
///
/// `ratings`    The galaxy at war ratings value
/// `promotions` The promotions value
fn ratings_response(ratings: GalaxyAtWar, promotions: u32) -> Xml {
    let GalaxyAtWar {
        group_a,
        group_b,
        group_c,
        group_d,
        group_e,
        ..
    } = ratings;

    // Calculate the average value for the level
    let level = (group_a + group_b + group_c + group_d + group_e) / 5;

    Xml(formatdoc! {r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <galaxyatwargetratings>
            <ratings>
                <ratings>{group_a}</ratings>
                <ratings>{group_b}</ratings>
                <ratings>{group_c}</ratings>
                <ratings>{group_d}</ratings>
                <ratings>{group_e}</ratings>
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
    "#})
}

/// Display implementation for the GAWError this will be displayed
/// as the error response message.
///
/// Messages match the server error messages as closely as possible.
impl Display for GAWError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidToken => f.write_str(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <error>
                    <component>2049</component>
                    <errorCode>1074003968</errorCode>
                    <errorName>ERR_AUTHENTICATION_REQUIRED</errorName>
                </error>"#,
            ),
            Self::ServerError => f.write_str(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <error>
                    <errorcode>500</errorcode>
                    <errormessage>Internal server error</errormessage>
                </error>"#,
            ),
        }
    }
}

/// From implementation to allow the conversion between the
/// two error types
impl From<DbErr> for GAWError {
    fn from(_: DbErr) -> Self {
        Self::ServerError
    }
}

/// IntoResponse implementation for GAWError to allow it to be
/// used within the result type as a error response
impl IntoResponse for GAWError {
    fn into_response(self) -> Response {
        let status = match &self {
            GAWError::InvalidToken => StatusCode::OK,
            GAWError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let mut response = (status, self.to_string()).into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/xml"));
        response
    }
}
