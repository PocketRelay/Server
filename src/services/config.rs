use embeddy::Embedded;
use log::error;
use me3_coalesced_parser::Coalesced;
use std::path::Path;

/// Embedded copy of the default known talk files
#[derive(Embedded)]
#[folder = "src/resources/data/tlk"]
struct DefaultTlkFiles;

/// Attempts to load a talk file from a local file
pub async fn local_talk_file(lang: &str) -> std::io::Result<Vec<u8>> {
    let file_name = format!("{}.tlk", lang);
    let local_path = format!("data/{}", file_name);
    let local_path = Path::new(&local_path);
    tokio::fs::read(local_path).await
}

/// Loads a fallback talk file from the embedded talk files list
/// using the specified language. Will fallback to default if the
/// language is not found.
pub fn fallback_talk_file(lang: &str) -> &'static [u8] {
    let file_name = format!("{}.tlk", lang);

    // Fallback to embedded tlk files
    DefaultTlkFiles::get(&file_name)
        // Fallback to default tlk
        .unwrap_or_else(|| {
            DefaultTlkFiles::get("default.tlk").expect("Server missing default embedded tlk file")
        })
}

/// Embedded default coalesced
static DEFAULT_COALESCED: &[u8] = include_bytes!("../resources/data/coalesced.json");

/// Attempts to load the local coalesced file from the data folder
pub async fn local_coalesced_file() -> std::io::Result<Coalesced> {
    let local_path = Path::new("data/coalesced.json");
    let bytes = tokio::fs::read(local_path).await?;

    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(err) => {
            error!("Failed to parse server coalesced: {}", err);

            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to parse server coalesced",
            ))
        }
    }
}

/// Loads the fallback coalesced from the embedded bytes
pub fn fallback_coalesced_file() -> Coalesced {
    serde_json::from_slice(DEFAULT_COALESCED)
        // Game cannot run without a proper coalesced
        .expect("Server fallback coalesced is malformed")
}
