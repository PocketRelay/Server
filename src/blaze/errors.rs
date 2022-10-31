use blaze_pk::{packet, CodecError, OpaquePacket};
use derive_more::From;
use sea_orm::DbErr;
use std::io;

pub type HandleResult = Result<(), BlazeError>;
pub type BlazeResult<T> = Result<T, BlazeError>;
pub type GameResult<T> = Result<T, GameError>;

#[derive(Debug, From)]
pub enum BlazeError {
    CodecError(CodecError),
    IO(io::Error),
    Other(&'static str),
    Database(DbErr),
    MissingPlayer,
    // Response error type. Responds with the provided response through
    // the redirect handler
    Response(OpaquePacket),
    Context(String, Box<BlazeError>),
    Game(GameError),
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

#[derive(Debug, From)]
pub enum GameError {
    IO(io::Error),
    Full,
    MissingHost,
    Other(&'static str),
    UnknownGame(u32),
    Context(&'static str, Box<GameError>),
}

impl GameError {
    /// Provides additional context to the error
    pub fn context(self, context: &'static str) -> GameError {
        GameError::Context(context, Box::new(self))
    }
}

/// Enum for errors relating to authentication
#[derive(Debug, Clone)]
#[repr(u16)]
#[allow(unused)]
pub enum LoginError {
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

impl Into<u16> for LoginError {
    fn into(self) -> u16 {
        self as u16
    }
}

packet! {
    struct LoginErrorRes {
        PNAM pnam: &'static str,
        UID uid: u8
    }
}

impl Default for LoginErrorRes {
    fn default() -> Self {
        LoginErrorRes { pnam: "", uid: 0 }
    }
}
