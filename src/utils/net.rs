use std::{
    net::{IpAddr, Ipv4Addr},
    time::{Duration, SystemTime},
};

use reqwest;
use serde::Deserialize;
use tokio::sync::RwLock;

/// Caching structure for the public address value
enum PublicAddrCache {
    /// The value hasn't yet been computed
    Unset,
    /// The value has been computed
    Set {
        /// The public address value
        value: Ipv4Addr,
        /// The system time the cache expires at
        expires: SystemTime,
    },
}

/// Cache value for storing the public address
static PUBLIC_ADDR_CACHE: RwLock<PublicAddrCache> = RwLock::const_new(PublicAddrCache::Unset);

/// Cache public address for 2 hours
const ADDR_CACHE_TIME: Duration = Duration::from_secs(60 * 60 * 2);

/// Retrieves the public address of the server either using the cached
/// value if its not expired or fetching the new value from the API using
/// `fetch_public_addr`
pub async fn public_address() -> Option<Ipv4Addr> {
    {
        let cached = &*PUBLIC_ADDR_CACHE.read().await;
        match cached {
            PublicAddrCache::Unset => {}
            PublicAddrCache::Set { value, expires } => {
                let time = SystemTime::now();
                if time.lt(expires) {
                    return Some(*value);
                }
            }
        };
    }

    // API addresses for IP lookup
    let addresses = ["https://api.ipify.org/", "https://ipv4.icanhazip.com/"];
    let mut value: Option<Ipv4Addr> = None;

    // Try all addresses using the first valid value
    for address in addresses {
        let response = match reqwest::get(address).await {
            Ok(value) => value,
            Err(_) => continue,
        };

        let ip = match response.text().await {
            Ok(value) => value.trim().replace('\n', ""),
            Err(_) => continue,
        };

        if let Ok(parsed) = ip.parse() {
            value = Some(parsed);
            break;
        }
    }

    // If we couldn't connect to any IP services its likely
    // we don't have internet lets try using our local address
    if value.is_none() {
        if let Ok(IpAddr::V4(addr)) = local_ip_address::local_ip() {
            value = Some(addr)
        }
    }

    let value = value?;

    // Update cached value with the new address
    {
        let cached = &mut *PUBLIC_ADDR_CACHE.write().await;
        *cached = PublicAddrCache::Set {
            value,
            expires: SystemTime::now() + ADDR_CACHE_TIME,
        };
    }

    Some(value)
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

/// Attempts to resolve the address value of the provided host. First attempts
/// to use the system DNS with tokio but if the resolved address is loopback it
/// is ignored and the google HTTP DNS will be attempted instead
///
/// `host` The host to lookup
pub async fn lookup_host(host: &str) -> Option<String> {
    // Attempt to lookup using the system DNS
    {
        let tokio = tokio::net::lookup_host(host)
            .await
            .ok()
            .and_then(|mut value| value.next());

        if let Some(tokio) = tokio {
            let ip = tokio.ip();
            // Loopback value means it was probbably redirected in the hosts file
            // so those are ignored
            if !ip.is_loopback() {
                return Some(format!("{}", ip));
            }
        }
    }

    // Attempt to lookup using google HTTP DNS
    let url = format!("https://dns.google/resolve?name={host}&type=A");
    let mut request = reqwest::get(url)
        .await
        .ok()?
        .json::<LookupResponse>()
        .await
        .ok()?;

    let answer = request.answer.pop()?;
    Some(answer.data)
}
