use blaze_pk::codec::CodecError;
use database::DbErr;
use std::{fmt::Display, io};

pub type BlazeResult<T> = Result<T, BlazeError>;
pub type ServerResult<T> = Result<T, ServerError>;

#[derive(Debug)]
pub enum BlazeError {
    CodecError(CodecError),
    IO(io::Error),
    Other(&'static str),
    Database(DbErr),
    ServerError(ServerError),
}

impl Display for BlazeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodecError(value) => write!(f, "Codec error occurred: {value:?}"),
            Self::IO(value) => write!(f, "IO error: {value:?}"),
            Self::Other(value) => write!(f, "Other: {value}"),
            Self::Database(value) => write!(f, "Database error: {value}"),
            Self::ServerError(value) => write!(f, "Server error: {value:?}"),
        }
    }
}

impl From<CodecError> for BlazeError {
    fn from(err: CodecError) -> Self {
        BlazeError::CodecError(err)
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
        BlazeError::ServerError(err)
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
    // Errors from suspend
    Suspend12D = 0x12D,
    Suspend12E = 0x12E,
}
