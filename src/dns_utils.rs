use anyhow::{Context, Result};
use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::proto::xfer::Protocol;
use hickory_resolver::Resolver;
use reqwest::Client;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

const TEST_DOMAIN: &str = "www.google.com";
const DOWNLOAD_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=1000000"; // 1MB file
const DOWNLOAD_TIMEOUT_SECS: u64 = 7; // Slightly less than 7.5 to be safe

type TokioResolver = Resolver<TokioConnectionProvider>;

/// Create a DNS resolver configured to use a specific DNS server.
fn create_resolver(dns_ip: IpAddr) -> TokioResolver {
    let socket_addr = SocketAddr::new(dns_ip, 53);
    let name_server = NameServerConfig::new(socket_addr, Protocol::Udp);
    let mut config = ResolverConfig::new();
    config.add_name_server(name_server);

    let mut opts = ResolverOpts::default();
    opts.timeout = Duration::from_secs(2); // Reduced to wait less for slow DNS
    opts.attempts = 1;

    Resolver::builder_with_config(config, TokioConnectionProvider::default())
        .with_options(opts)
        .build()
}

/// Measure the latency (resolution time) for a given DNS server.
pub async fn test_latency(dns_ip: IpAddr) -> Result<Duration> {
    let resolver = create_resolver(dns_ip);
    let start = Instant::now();
    resolver
        .lookup_ip(TEST_DOMAIN)
        .await
        .with_context(|| format!("Failed to resolve {} via {}", TEST_DOMAIN, dns_ip))?;
    Ok(start.elapsed())
}

/// Measure download speed (in Mbps) by resolving a URL through a specific DNS and downloading.
///
/// This test checks how well the DNS routes us to a fast CDN edge.
/// Returns the download speed in Megabits per second (Mbps).
pub async fn test_download_speed(dns_ip: IpAddr) -> Result<f64> {
    // First, resolve the download URL's domain using the target DNS
    let resolver = create_resolver(dns_ip);

    // Parse the host from the URL
    let url = reqwest::Url::parse(DOWNLOAD_TEST_URL)?;
    let host = url
        .host_str()
        .context("Invalid URL: no host")?;

    // Resolve the host
    let response = resolver.lookup_ip(host).await?;
    let resolved_ip = response
        .iter()
        .next()
        .context("DNS returned no addresses")?;

    // Build a client that connects to the resolved IP
    // We override DNS resolution by connecting directly to the IP
    let client = Client::builder()
        .resolve(host, SocketAddr::new(resolved_ip, 443))
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build()?;

    // Perform the download and measure time
    let start = Instant::now();
    let response = client.get(DOWNLOAD_TEST_URL).send().await?;
    let bytes = response.bytes().await?;
    let elapsed = start.elapsed();

    // Calculate speed in Mbps (Megabits per second)
    let bytes_downloaded = bytes.len() as f64;
    let bits_downloaded = bytes_downloaded * 8.0;
    let megabits = bits_downloaded / 1_000_000.0;
    let seconds = elapsed.as_secs_f64();

    if seconds > 0.0 {
        Ok(megabits / seconds)
    } else {
        Ok(0.0)
    }
}

#[derive(Debug, Clone)]
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
}

/// Run a full test (latency + download speed) for a given DNS server.
/// Enforces a hard 7.5-second limit for the entire process.
pub async fn run_full_test(dns_ip: IpAddr) -> DnsTestResult {
    let mut result = DnsTestResult::new(dns_ip);

    let test_future = async {
        // Test latency
        match test_latency(dns_ip).await {
            Ok(latency) => result.latency = Some(latency),
            Err(e) => {
                result.error = Some(format!("Latency test failed: {}", e));
                return result;
            }
        }

        // Test download speed
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
