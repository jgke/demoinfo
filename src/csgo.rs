#[allow(clippy::upper_case_acronyms)]
pub mod steammessages {
    include!(concat!(env!("OUT_DIR"), "/csgo.buf.steammessages.rs"));
}
#[allow(clippy::upper_case_acronyms)]
pub mod cstrike15_gcmessages {
    include!(concat!(
        env!("OUT_DIR"),
        "/csgo.buf.cstrike15_gcmessages.rs"
    ));
}
#[allow(clippy::upper_case_acronyms)]
pub mod cstrike15_usermessages_public {
    include!(concat!(
        env!("OUT_DIR"),
        "/csgo.buf.cstrike15_usermessages_public.rs"
    ));
}
#[allow(clippy::upper_case_acronyms)]
pub mod netmessages_public {
    include!(concat!(env!("OUT_DIR"), "/csgo.buf.netmessages_public.rs"));
}
