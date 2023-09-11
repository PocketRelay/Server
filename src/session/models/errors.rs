use interlink::prelude::LinkError;
use log::error;
use sea_orm::DbErr;

use crate::session::{packet::Packet, router::IntoPacketResponse};

use super::{
    auth::AuthenticationError, game_manager::GameManagerError, user_sessions::UserSessionsError,
    util::UtilError,
};

pub type ServerResult<T> = Result<T, BlazeError>;

#[test]
fn decode_error() {
    let value: i32 = 96258;
    let bytes = value.to_le_bytes();
    let mut out = [0u8; 2];
    out.copy_from_slice(&bytes[2..]);
    let out = u16::from_le_bytes(out);
    println!("{:#00x}", out);
}

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum GlobalError {
    Cancelled = 0x4009,
    Disconnected = 0x4006,
    DuplicateLogin = 0x4007,
    AuthorizationRequired = 0x4008,
    Timeout = 0x4005,
    ComponentNotFound = 0x4002,
    CommandNotFound = 0x4003,
    AuthenticationRequired = 0x4004,
    System = 0x4001,
}

#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum DatabaseError {
    Timeout = 0x406c,
    InitFailure = 0x406d,
    TranscationNotComplete = 0x406e,
    Disconnected = 0x406b,
    NoConnectionAvailable = 0x4068,
    DuplicateEntry = 0x4069,
    System = 0x4065,
}

/// Response type for some blaze error code
pub struct BlazeError(u16);

impl From<LinkError> for BlazeError {
    fn from(_: LinkError) -> Self {
        GlobalError::System.into()
    }
}

impl From<DbErr> for BlazeError {
    fn from(value: DbErr) -> Self {
        error!("Database error: {}", value);
        match value {
            DbErr::ConnectionAcquire(_) => DatabaseError::NoConnectionAvailable,
            DbErr::Conn(_) => DatabaseError::InitFailure,
            _ => DatabaseError::System,
        }
        .into()
    }
}

impl From<GameManagerError> for BlazeError {
    fn from(value: GameManagerError) -> Self {
        BlazeError(value as u16)
    }
}

impl From<UserSessionsError> for BlazeError {
    fn from(value: UserSessionsError) -> Self {
        BlazeError(value as u16)
    }
}

impl From<GlobalError> for BlazeError {
    fn from(value: GlobalError) -> Self {
        BlazeError(value as u16)
    }
}
impl From<AuthenticationError> for BlazeError {
    fn from(value: AuthenticationError) -> Self {
        BlazeError(value as u16)
    }
}

impl From<DatabaseError> for BlazeError {
    fn from(value: DatabaseError) -> Self {
        BlazeError(value as u16)
    }
}
impl From<UtilError> for BlazeError {
    fn from(value: UtilError) -> Self {
        BlazeError(value as u16)
    }
}

impl IntoPacketResponse for BlazeError {
    fn into_response(self, req: &Packet) -> Packet {
        Packet::error_empty(req, self.0)
    }
}
