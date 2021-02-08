fn main() -> Result<(), std::io::Error> {
    prost_build::compile_protos(&[
        "protos/cstrike15_gcmessages.proto",
        "protos/cstrike15_usermessages_public.proto",
        "protos/netmessages_public.proto",
        "protos/steammessages.proto",
    ], &["protos/"])?;
    Ok(())
}
