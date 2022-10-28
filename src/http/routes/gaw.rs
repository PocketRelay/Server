use std::fmt::Display;
use std::num::ParseIntError;
use actix_web::{get, HttpRequest, HttpResponse, ResponseError};
use actix_web::body::BoxBody;
use actix_web::http::StatusCode;
use actix_web::web::{Data, ServiceConfig};
use derive_more::{Display, From};
use log::warn;
use sea_orm::DbErr;
use serde::Deserialize;
use crate::database::interface::players::{find_by_id, get_session_token};
use crate::GlobalState;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(authenticate);
}

#[derive(Deserialize)]
struct AuthQuery {
    auth: String,
}

#[derive(Debug, Display, From)]
enum AuthError {
    #[display(fmt = "Invalid ID {}", _0)]
    InvalidID(ParseIntError),
    #[display(fmt = "Unknown ID")]
    UnknownID,
    #[display(fmt = "Database Error {}", _0)]
    DatabaseError(DbErr),
}

impl ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        match self {
            AuthError::InvalidID(_) | AuthError::UnknownID => StatusCode::BAD_REQUEST,
            AuthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::build(self.status_code())
            .finish()
    }
}

#[get("gaw/authentication/sharedTokenLogin")]
async fn authenticate(req: HttpRequest, global: Data<GlobalState>) -> Result<String, AuthError> {
    let auth = req.match_info().query("auth");
    let auth = match u32::from_str_radix(auth, 16) {
        Ok(value) => value,
        Err(err) => {
            warn!("Failed to parse provided GAW auth ID: {auth}");
            return Err(AuthError::InvalidID(err))
        }
    };

    let player = match find_by_id(&global.db, auth).await {
        Ok(Some(value)) => value,
        Ok(None) => return Err(AuthError::UnknownID),
        Err(err) => {
            warn!("Failed to query database for provided GAW auth ID: {err}");
            return Err(AuthError::DatabaseError(err))
        }
    };

    let (player, token) = get_session_token(&global.db, player).await?;

    let id = player.id;
    let sess = format!("{:x}", id);
    let display_name = player.display_name;

    let response = format!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
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
    </fulllogin>
    "#);

    Ok(response)
}