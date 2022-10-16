use blaze_pk::packet;

/// Enum for errors relating to authentication
#[derive(Debug, Clone)]
#[repr(u16)]
pub enum LoginError {
    ServerUnavailable = 0x0,
    EmailNotFound = 0xB,
    WrongPassword =0xC,
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
    ConnectionLost = 0x4007
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