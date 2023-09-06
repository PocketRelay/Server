use tdf::DecodeError;

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

#[test]
fn decode_error() {
    let value: i32 = -2146566144;
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

impl From<ServerError> for BlazeError {
    fn from(value: ServerError) -> Self {
        BlazeError(value as u16)
    }
}
impl From<GlobalError> for BlazeError {
    fn from(value: GlobalError) -> Self {
        BlazeError(value as u16)
    }
}

impl From<DatabaseError> for BlazeError {
    fn from(value: DatabaseError) -> Self {
        BlazeError(value as u16)
    }
}

impl IntoResponse for BlazeError {
    fn into_response(self, req: &Packet) -> Packet {
        req.respond_error_empty(self.0)
    }
}

impl IntoResponse for ServerError {
    fn into_response(self, req: &Packet) -> Packet {
        req.respond_error_empty(self as u16)
    }
}

impl IntoResponse for DecodeError {
    fn into_response(self, req: &Packet) -> Packet {
        req.respond_error_empty(ServerError::ServerUnavailable as u16)
    }
}
