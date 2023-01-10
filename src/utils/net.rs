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
                    return Some(value.clone());
                }
            }
        };
    }

    // API addresses for IP lookup
    let addresses = ["https://api.ipify.org/", "https://ipv4.icanhazip.com/"];
    let mut value: Option<Ipv4Addr> = None;

    // Try all addresses using the first valid value
    for address in addresses {
        if let Ok(response) = reqwest::get(address).await {
            if let Ok(ip) = response.text().await {
                let ip = ip.trim().replace('\n', "");
                if let Ok(parsed) = ip.parse() {
                    value = Some(parsed);
                    break;
                }
            }
        }
    }

    // If we couldn't connect to any IP services its likely
    // we don't have internet lets try using our local address
    {
        if let Ok(IpAddr::V4(addr)) = local_ip_address::local_ip() {
            value = Some(addr)
        }
    }

    let value = value?;

    // Update cached value with the new address
    {
        let cached = &mut *PUBLIC_ADDR_CACHE.write().await;
        *cached = PublicAddrCache::Set {
            value: value.clone(),
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
