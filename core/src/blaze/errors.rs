use blaze_pk::CodecError;
use database::DbErr;
use std::io;

pub type HandleResult = Result<(), BlazeError>;
pub type BlazeResult<T> = Result<T, BlazeError>;

#[derive(Debug)]
pub enum BlazeError {
    CodecError(CodecError),
    IO(io::Error),
    Other(&'static str),
    Database(DbErr),
    MissingPlayer,
    Context(String, Box<BlazeError>),
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

impl BlazeError {
    /// Provides additional context to the error
    pub fn context(self, context: &str) -> BlazeError {
        BlazeError::Context(context.to_string(), Box::new(self))
    }

    /// Provides additional context to the error
    pub fn context_owned(self, context: String) -> BlazeError {
        BlazeError::Context(context, Box::new(self))
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
}

impl Into<u16> for ServerError {
    fn into(self) -> u16 {
        self as u16
    }
}
