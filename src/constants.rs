/// The server version extracted from the Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The external address of the server. This address is whats used in
/// the system hosts file as a redirect so theres no need to use any
/// other address.
pub const EXTERNAL_HOST: &str = "gosredirector.ea.com";
