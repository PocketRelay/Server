//! Module for retrieving data from the official Mass Effect 3 Servers

use dnsclient::{sync::DNSClient, UpstreamServer};

/// The address of the DNS server to use for DNS lookups
const DNS_ADDRESS: (&str, u16) = ("1.1.1.1", 53);

/// Attempts to find the real address for gosredirector.ea.com
/// will use the cloudflare DNS in order to bypass any possible
/// redirects present in the system hosts file.
///
/// Will return None if this process failed.
fn obtain_real_address() -> Option<String> {
    let upstream = UpstreamServer::new(DNS_ADDRESS);
    let dns_client = DNSClient::new(vec![upstream]);
    let result = dns_client.query_a(name).ok()?;
    let result = result.pop()?;
    Some(format!("{}", result))
}

/// Structure for the retrievier system which contains the host address
/// for gosredirector.ea.com for making requests to the real redirector
pub struct Retriever {
    redirector_host: String,
    main_host: String,
}

impl Retriever {
    pub async fn new() -> Option<Retriever> {
        tokio::spawn(Self::new_sync).await.ok()
    }

    fn new_sync() -> Option<Retriever> {
        let redirector_host = obtain_real_address()?;
        let retriever = Retriever { redirector_host };
        Some(retriever)
    }
}
