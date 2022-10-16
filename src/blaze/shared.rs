use blaze_pk::{group, packet};

packet! {
    struct SilentAuthRes {
        AGUP agup: u8,
        LDHT ldht: &'static str,
        NTOS ntos: u8,
        PCTK token: String,
        PRIV pri: &'static str,
        SESS session: SessionDetails,
        SPAM spam: u8,
        THST thst: &'static str,
        TSUI tsui: &'static str,
        TURI turi: &'static str
    }
}

packet! {
    struct AuthRes {
        LDHT ldht: &'static str,
        NTOS ntos: u8,
        PCTK token: String,
        PLST personas: Vec<PersonaDetails>,
        PRIV pri: &'static str,
        SKEY skey: String,
        SPAM spam: u8,
        THST thst: &'static str,
        TSUI tsui: &'static str,
        TURI turi: &'static str,
        UID uid: u32
    }
}

packet! {
    struct SessionDetails {
        BUID buid: u32,
        FRST frst: u8,
        KEY key: String,
        LLOG llog: u8,
        MAIL mail: String,
        PDTL personal_details: PersonaDetails,
        UID uid: u32,
    }
}

group! {
    struct PersonaDetails {
        DSNM display_name: String,
        LAST last_login_time: u32,
        PID  id: u32,
        STAS stas: u8,
        XREF xref: u8,
        XTYP xtype: u8
    }
}