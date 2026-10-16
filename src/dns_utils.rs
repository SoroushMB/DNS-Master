//! DNS resolution & download benchmarking against a specific resolver IP.

use anyhow::{Context, Result, anyhow};
use hickory_resolver::Resolver;
use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::proto::xfer::Protocol;
use reqwest::Client;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

const TEST_DOMAIN: &str = "www.google.com";
const DOWNLOAD_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=1000000"; // 1MB
const DOWNLOAD_TIMEOUT_SECS: u64 = 7;

const LATENCY_ATTEMPTS: usize = 2;
const DOWNLOAD_ATTEMPTS: usize = 2;

pub type TokioResolver = Resolver<TokioConnectionProvider>;

/// Create a DNS resolver pointed at a specific server (plain UDP/53).
pub fn create_resolver(dns_ip: IpAddr) -> TokioResolver {
    let socket_addr = SocketAddr::new(dns_ip, 53);
    let name_server = NameServerConfig::new(socket_addr, Protocol::Udp);
    let mut config = ResolverConfig::new();
    config.add_name_server(name_server);

    let mut opts = ResolverOpts::default();
    opts.timeout = Duration::from_secs(2);
    opts.attempts = 1;

    Resolver::builder_with_config(config, TokioConnectionProvider::default())
        .with_options(opts)
        .build()
}

/// Measure the latency (resolution time) for a given DNS server, `n` attempts.
pub async fn test_latency(dns_ip: IpAddr) -> Result<Duration> {
    let resolver = create_resolver(dns_ip);
    let mut last_err = None;
    for attempt in 0..LATENCY_ATTEMPTS {
        let start = Instant::now();
        match resolver.lookup_ip(TEST_DOMAIN).await {
            Ok(_) => return Ok(start.elapsed()),
            Err(e) => last_err = Some(e),
        }
        if attempt + 1 < LATENCY_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    let e = last_err.context("resolver produced no error")?;
    anyhow::bail!(
        "Failed to resolve {} via {} ({} attempts): {e}",
        TEST_DOMAIN,
        dns_ip,
        LATENCY_ATTEMPTS
    );
}

/// Measure download speed (in Mbps) by resolving a URL's host through the
/// candidate DNS and pinning the connection to the resolved IP — this checks
/// how well the DNS routes us to a fast CDN edge.
pub async fn test_download_speed(dns_ip: IpAddr) -> Result<f64> {
    let resolver = create_resolver(dns_ip);

    let url = reqwest::Url::parse(DOWNLOAD_TEST_URL)?;
    let host = url.host_str().context("Invalid URL: no host")?.to_string();

    let mut last_err = None;
    for attempt in 0..DOWNLOAD_ATTEMPTS {
        let resolved = match resolver.lookup_ip(&host).await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(anyhow!("DNS lookup failed: {e}"));
                continue;
            }
        };
        let resolved_ip = match resolved.iter().next() {
            Some(ip) => ip,
            None => {
                last_err = Some(anyhow::anyhow!("DNS returned no addresses"));
                continue;
            }
        };

        let client = Client::builder()
            .resolve(&host, SocketAddr::new(resolved_ip, 443))
            .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
            .build()?;

        let start = Instant::now();
        match async {
            let response = client.get(DOWNLOAD_TEST_URL).send().await?;
            let bytes = response.bytes().await?;
            Ok::<f64, anyhow::Error>({
                let bytes_downloaded = bytes.len() as f64;
                let seconds = start.elapsed().as_secs_f64();
                if seconds > 0.0 {
                    (bytes_downloaded * 8.0 / 1_000_000.0) / seconds
                } else {
                    0.0
                }
            })
        }
        .await
        {
            Ok(speed) => return Ok(speed),
            Err(e) => last_err = Some(e),
        }
        if attempt + 1 < DOWNLOAD_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    let e = last_err.context("download test produced no error")?;
    anyhow::bail!("Download test failed via {}: {e}", dns_ip);
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DnsTestResult {
    pub ip: IpAddr,
    pub latency: Option<Duration>,
    pub download_speed_mbps: Option<f64>,
    pub error: Option<String>,
}

impl DnsTestResult {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            ip,
            latency: None,
            download_speed_mbps: None,
            error: None,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none() && (self.latency.is_some() || self.download_speed_mbps.is_some())
    }
}

/// Run a full test (latency + download speed) for a given DNS server.
/// Enforces a hard 7.5-second limit for the entire process.
pub async fn run_full_test(dns_ip: IpAddr) -> DnsTestResult {
    let mut result = DnsTestResult::new(dns_ip);

    let test_future = async {
        match test_latency(dns_ip).await {
            Ok(latency) => result.latency = Some(latency),
            Err(e) => {
                result.error = Some(format!("Latency test failed: {}", e));
                return result;
            }
        }

        match test_download_speed(dns_ip).await {
            Ok(speed) => result.download_speed_mbps = Some(speed),
            Err(e) => {
                result.error = Some(format!("Download test failed: {}", e));
            }
        }
        result
    };

    match tokio::time::timeout(Duration::from_millis(7500), test_future).await {
        Ok(res) => res,
        Err(_) => {
            let mut timeout_result = DnsTestResult::new(dns_ip);
            timeout_result.error = Some("Test timed out (exceeded 7.5s)".to_string());
            timeout_result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_result_requires_value() {
        let mut r = DnsTestResult::new("8.8.8.8".parse().unwrap());
        assert!(!r.is_ok());
        r.latency = Some(Duration::from_millis(50));
        assert!(r.is_ok());
        r.error = Some("x".into());
        assert!(!r.is_ok());
    }
}
