//! Utility for retrieving the public IP address of this machine.

/// Retrieves the public IPv4 address of this machine using the ipv4.icanhazip.com
/// API trimming the response to remove new lines.
pub async fn public_address() -> Option<String> {
    let result = reqwest::get("https://ipv4.icanhazip.com/")
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let result = result.trim();
    let result = result.replace("\n", "");
    Some(result)
}

#[cfg(test)]
mod test {
    use super::public_address;

    /// Test function for ensuring that the public address returned
    /// from `public_address` is actually an IPv4 address
    #[tokio::test]
    async fn test_public_address() {
        let value = public_address()
            .await
            .expect("Failed to retriever public address");

        let parts = value.split(".").collect::<Vec<_>>();

        assert_eq!(parts.len(), 4);

        let parts = parts
            .iter()
            .filter_map(|value| value.parse::<u8>().ok())
            .collect::<Vec<_>>();

        assert_eq!(parts.len(), 4);
    }
}
