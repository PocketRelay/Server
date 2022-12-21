//! Routes for the Galaxy At War API used by the Mass Effect 3 client in order
//! to retrieve and increase the Galxay At War values for a player.
//!
//! This API is not documented as it is not intended to be used by anyone
//! other than the Mass Effect 3 client itself.

use crate::{
    env,
    servers::http::ext::{ErrorStatusCode, Xml},
    state::GlobalState,
};
use axum::{
    extract::{Path, Query},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use database::{DatabaseConnection, DbErr, GalaxyAtWar, Player};
use serde::Deserialize;
use std::fmt::Display;
use tokio::try_join;

/// Function for adding all the routes in this file to
/// the provided router
///
/// `router` The route to add to
pub fn route(router: Router) -> Router {
    router
        .route(
            "/gaw/authentication/sharedTokenLogin",
            get(shared_token_login),
        )
        .route("/gaw/galaxyatwar/getRatings/:id", get(get_ratings))
        .route(
            "/gaw/galaxyatwar/increaseRatings/:id",
            get(increase_ratings),
        )
}

/// Error type used in gaw routes to handle errors such
/// as being unable to parse player IDs, find players
/// or Database errors
enum GAWError {
    /// The provided player ID was not a valid hex value
    InvalidID,
    /// The player could not be found
    PlayerNotFound,
    /// There was a server error
    ServerError,
}

/// Query for authenticating with a shared login token.
#[derive(Deserialize)]
struct AuthQuery {
    /// The authentication token (This is just a hex encoded player ID)
    auth: String,
}

/// Route for handling shared token login. In the official implementation this
/// would login the client using the shared token provided by the Main server.
/// But this implementation just responds with the bare minimum response directly
/// passing the auth key as the session token for further requests
///
/// `query` The query containing the auth token (In this case the hex player ID)
async fn shared_token_login(Query(query): Query<AuthQuery>) -> Xml {
    let response = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<fulllogin>
    <canageup>0</canageup>
    <legaldochost/>
    <needslegaldoc>0</needslegaldoc>
    <pclogintoken></pclogintoken>
    <privacypolicyuri/>
    <sessioninfo>
        <blazeuserid></blazeuserid>
        <isfirstlogin>0</isfirstlogin>
        <sessionkey>{}</sessionkey>
        <lastlogindatetime></lastlogindatetime>
        <email></email>
        <personadetails>
            <displayname></displayname>
            <lastauthenticated></lastauthenticated>
            <personaid></personaid>
            <status>UNKNOWN</status>
            <extid>0</extid>
            <exttype>BLAZE_EXTERNAL_REF_TYPE_UNKNOWN</exttype>
        </personadetails>
        <userid></userid>
    </sessioninfo>
    <isoflegalcontactage>0</isoflegalcontactage>
    <toshost/>
    <termsofserviceuri/>
    <tosuri/>
</fulllogin>"#,
        query.auth
    );
    Xml(response)
}

/// Retrieves the galaxy at war data and promotions count for
/// the player with the provided ID
///
/// `db` The dataabse connection
/// `id` The hex ID of the player
async fn get_player_gaw_data(
    db: &DatabaseConnection,
    id: &str,
) -> Result<(GalaxyAtWar, u32), GAWError> {
    let id = u32::from_str_radix(id, 16).map_err(|_| GAWError::InvalidID)?;
    let player = Player::by_id(db, id)
        .await?
        .ok_or(GAWError::PlayerNotFound)?;
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
async fn get_ratings(Path(id): Path<String>) -> Result<Xml, GAWError> {
    let db = GlobalState::database();
    let (gaw_data, promotions) = get_player_gaw_data(db, &id).await?;
    Ok(ratings_response(gaw_data, promotions))
}

/// The query structure for increasing the galaxy at war values
/// for a player
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
async fn increase_ratings(
    Path(id): Path<String>,
    Query(query): Query<IncreaseQuery>,
) -> Result<Xml, GAWError> {
    let db = GlobalState::database();
    let (gaw_data, promotions) = get_player_gaw_data(db, &id).await?;
    let gaw_data = gaw_data
        .increase(db, (query.a, query.b, query.c, query.d, query.e))
        .await?;
    Ok(ratings_response(gaw_data, promotions))
}

/// Generates a ratings XML response from the provided ratings struct and
/// promotions value.
///
/// `ratings`    The galaxy at war ratings value
/// `promotions` The promotions value
fn ratings_response(ratings: GalaxyAtWar, promotions: u32) -> Xml {
    let a = ratings.group_a;
    let b = ratings.group_b;
    let c = ratings.group_c;
    let d = ratings.group_d;
    let e = ratings.group_e;

    // Calculate the average value for the level
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
</galaxyatwargetratings>"#
    );
    Xml(response)
}

/// Display implementation for the GAWError this will be displayed
/// as the error response message.
///
/// Messages match the server error messages as closely as possible.
impl Display for GAWError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidID => f.write_str(
                r#"<?xml version="1.0" encoding="UTF-8"?>
                <error>
                    <errorcode>401</errorcode>
                    <errormessage>Session key format invalid</errormessage>
                </error>"#,
            ),
            Self::PlayerNotFound => f.write_str(
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

/// Error status code implementation for the different error
/// status codes of each error.
///
/// These response codes match that of the official servers
impl ErrorStatusCode for GAWError {
    fn status_code(&self) -> StatusCode {
        match self {
            GAWError::InvalidID => StatusCode::UNAUTHORIZED,
            GAWError::PlayerNotFound => StatusCode::OK,
            GAWError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// IntoResponse implementation for GAWError to allow it to be
/// used within the result type as a error response
impl IntoResponse for GAWError {
    fn into_response(self) -> Response {
        let mut response = self.to_string().into_response();
        *response.status_mut() = self.status_code();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/xml"));
        response
    }
}
