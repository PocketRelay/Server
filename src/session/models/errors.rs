use crate::session::packet::{IntoResponse, Packet};

pub type ServerResult<T> = Result<T, ServerError>;

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
    fn into_response(self, req: &Packet) -> Packet {
        req.respond_error_empty(self as u16)
    }
}
