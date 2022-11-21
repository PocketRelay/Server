use reqwest;
use serde::Deserialize;

/// Retrieves the public IPv4 address of this machine using the ip4.seeip.org
/// API trimming the response to remove new lines.
pub async fn public_address() -> Option<String> {
    let result = reqwest::get("https://ip4.seeip.org/")
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let result = result.trim();
    let result = result.replace("\n", "");
    Some(result)
}

/// Structure for the lookup responses from the google DNS API
///
/// # Structure
///
/// ```
/// {
///   "Status": 0,
///   "TC": false,
///   "RD": true,
///   "RA": true,
///   "AD": false,
///   "CD": false,
///   "Question": [
///     {
///       "name": "gosredirector.ea.com.",
///       "type": 1
///     }
///   ],
///   "Answer": [
///     {
///       "name": "gosredirector.ea.com.",
///       "type": 1,
///       "TTL": 300,
///       "data": "159.153.64.175"
///     }
///   ],
///   "Comment": "Response from 2600:1403:a::43."
/// }
/// ```
#[derive(Deserialize)]
struct LookupResponse {
    #[serde(rename = "Answer")]
    answer: Vec<Answer>,
}

/// Structure for answer portion of request. Only the data value is
/// being used so only that is present here.
///
/// # Structure
/// ```
/// {
///   "name": "gosredirector.ea.com.",
///   "type": 1,
///   "TTL": 300,
///   "data": "159.153.64.175"
/// }
/// ```
#[derive(Deserialize)]
struct Answer {
    data: String,
}

/// Attempts to resolve the DNS host value of the provided hostname
/// uses the tokio lookup host function first but if the returned address
/// is a local address then the Google DNS lookup is used instead.
pub async fn lookup_host(value: &str) -> Option<String> {
    {
        let internal = lookup_tokio(value).await;
        if internal.is_some() {
            return internal;
        }
    }
    let url = format!("https://dns.google/resolve?name={value}&type=A");
    let mut request = reqwest::get(url)
        .await
        .ok()?
        .json::<LookupResponse>()
        .await
        .ok()?;

    request.answer.pop().map(|value| value.data)
}

/// Attempts to resolve the provided DNS entry using tokios function.
async fn lookup_tokio(value: &str) -> Option<String> {
    let internal = tokio::net::lookup_host(value).await.ok()?.next()?;
    let ip = internal.ip();

    // If the address is loopback then its probbably been overwritten in the
    // system hosts file.
    if ip.is_loopback() {
        return None;
    }

    Some(format!("{}", ip))
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
