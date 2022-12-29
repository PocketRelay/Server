use blaze_pk::packet::{IntoResponse, Packet};
use database::DbErr;
use std::{fmt::Display, io};

pub type BlazeResult<T> = Result<T, BlazeError>;
pub type ServerResult<T> = Result<T, ServerError>;

/// Error type used for handling a variety of possible errors
/// that can occur throughout the applications
#[derive(Debug)]
pub enum BlazeError {
    IO(io::Error),
    Database(DbErr),
    Server(ServerError),
}

impl Display for BlazeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(value) => write!(f, "IO error: {value:?}"),
            Self::Database(value) => write!(f, "Database error: {value}"),
            Self::Server(value) => write!(f, "Server error: {value:?}"),
        }
    }
}

impl From<io::Error> for BlazeError {
    fn from(err: io::Error) -> Self {
        BlazeError::IO(err)
    }
}

impl From<DbErr> for BlazeError {
    fn from(err: DbErr) -> Self {
        BlazeError::Database(err)
    }
}

impl From<ServerError> for BlazeError {
    fn from(err: ServerError) -> Self {
        BlazeError::Server(err)
    }
}

impl IntoResponse for BlazeError {
    fn into_response(self, req: Packet) -> Packet {
        let err = match self {
            Self::Server(err) => err as u16,
            _ => ServerError::ServerUnavailable as u16,
        };
        req.respond_error_empty(err)
    }
}

///  Enum for server error values
#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum ServerError {
    ServerUnavailable = 0x0,
    EmailNotFound = 0xB,
    WrongPassword = 0xC,
    InvalidSession = 0xD,
    EmailAlreadyInUse = 0x0F,
    AgeRestriction = 0x10,
    InvalidAccount = 0x11,
    BannedAccount = 0x13,
    InvalidInformation = 0x15,
    InvalidEmail = 0x16,
    LegalGuardianRequired = 0x2A,
    CodeRequired = 0x32,
    KeyCodeAlreadyInUse = 0x33,
    InvalidCerberusKey = 0x34,
    ServerUnavailableFinal = 0x4001,
    FailedNoLoginAction = 0x4004,
    ServerUnavailableNothing = 0x4005,
    ConnectionLost = 0x4007,
    UnableToUpdateSettings = 0xCB,
    // Errors from suspend
    Suspend12D = 0x12D,
    Suspend12E = 0x12E,
}

impl IntoResponse for ServerError {
    fn into_response(self, req: Packet) -> Packet {
        req.respond_error_empty(self as u16)
    }
}
