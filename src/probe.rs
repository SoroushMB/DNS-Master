//! Service access probes: empirically test *which* services a DNS server can
//! actually open (Gemini, YouTube, Telegram, X, ...), instead of trusting
//! marketing claims.
//!
//! Method: resolve each service domain through the candidate DNS, connect to
//! the returned IP with the proper SNI, classify the HTTP response:
//!   ✅ Unlocked      — reachable (2xx/3xx, no geo-block signature in body)
//!   ❌ Blocked       — service answered with 401/403/429/451 or a geo-block
//!                      interstitial (region not supported)
//!   ⚠️ Timeout       — DNS didn't resolve or the connection never established
//!                      (domestic filtering / dead resolver)
//!
//! Anti-sanction DNS resolvers may answer domains through SNI-forwarding
//! reverse proxies, which this probe transparently verifies end-to-end.

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::time::timeout;

use crate::dns_utils::create_resolver;

/// Total per-service probe envelope (DNS lookup + every attempt + body scan).
/// The outer `timeout(PROBE_TIMEOUT + 2s, task)` wrapper below is the hard
/// ceiling and is intentionally left intact.
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
/// Per-*attempt* HTTP request timeout. Deliberately shorter than the outer
/// envelope so that the ambiguous transport-error retry still fits inside it:
/// `3s + 400ms backoff + 3s ≈ 6.4s < PROBE_TIMEOUT + 2s (8s)`.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(3);
/// Delay before the single retry of an ambiguous transport error.
const RETRY_BACKOFF: Duration = Duration::from_millis(400);
/// Send attempts per service: the first try plus one cheap retry.
const MAX_ATTEMPTS: u32 = 2;
/// How much body to scan for geo-block interstitials.
const BODY_SCAN_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Gemini,
    Antigravity,
    YouTube,
    Telegram,
    GitHub,
    X,
    Spotify,
}

impl Service {
    pub const ALL: [Service; 7] = [
        Service::Gemini,
        Service::Antigravity,
        Service::YouTube,
        Service::Telegram,
        Service::GitHub,
        Service::X,
        Service::Spotify,
    ];

    /// Short column glyph used in the results table.
    pub fn glyph(&self) -> &'static str {
        match self {
            Service::Gemini => "G",
            Service::Antigravity => "A",
            Service::YouTube => "Y",
            Service::Telegram => "T",
            Service::GitHub => "H",
            Service::X => "X",
            Service::Spotify => "S",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Service::Gemini => "Gemini",
            Service::Antigravity => "Antigravity",
            Service::YouTube => "YouTube",
            Service::Telegram => "Telegram",
            Service::GitHub => "GitHub",
            Service::X => "X / Twitter",
            Service::Spotify => "Spotify",
        }
    }

    /// Lenient tag/CLI name mapping.
    pub fn from_tag(tag: &str) -> Option<Service> {
        match tag.trim().to_lowercase().as_str() {
            "gemini" | "google-ai" | "googleai" => Some(Service::Gemini),
            "antigravity" => Some(Service::Antigravity),
            "youtube" | "yt" => Some(Service::YouTube),
            "telegram" | "tg" => Some(Service::Telegram),
            "github" | "gh" | "octocat" => Some(Service::GitHub),
            "x" | "x.com" | "twitter" | "tw" => Some(Service::X),
            "spotify" => Some(Service::Spotify),
            // Known catalog tags that are *not* probed services
            "openai" | "chatgpt" | "docker" | "apple" | "steam" | "gaming" | "psn" | "xbox"
            | "stability" | "download" | "speed" | "privacy" | "security" | "adblock"
            | "stable-ping" => None,
            _ => None,
        }
    }

    /// Probe target host (for CLI display).
    pub fn host(&self) -> &'static str {
        self.def().host
    }

    fn def(&self) -> ProbeDef {
        match self {
            Service::Gemini => ProbeDef {
                host: "gemini.google.com",
                url: "https://gemini.google.com/",
                block_tokens: &[
                    "not available in your region",
                    "not available in your country",
                    "unsupported country",
                    "not supported in your",
                    "无法在您所在的国家/地区使用",
                    "در منطقه شما در دسترس نیست",
                ],
            },
            Service::Antigravity => ProbeDef {
                host: "antigravity.google",
                url: "https://antigravity.google/",
                block_tokens: &[
                    "not available in your region",
                    "not available in your country",
                    "unsupported country",
                ],
            },
            Service::YouTube => ProbeDef {
                host: "www.youtube.com",
                url: "https://www.youtube.com/generate_204",
                block_tokens: &[],
            },
            Service::Telegram => ProbeDef {
                host: "web.telegram.org",
                url: "https://web.telegram.org/a/",
                block_tokens: &[],
            },
            Service::GitHub => ProbeDef {
                host: "api.github.com",
                url: "https://api.github.com/zen",
                block_tokens: &["rate limit"],
            },
            Service::X => ProbeDef {
                host: "x.com",
                url: "https://x.com/",
                block_tokens: &[],
            },
            Service::Spotify => ProbeDef {
                host: "open.spotify.com",
                url: "https://open.spotify.com/",
                block_tokens: &[],
            },
        }
    }
}

struct ProbeDef {
    host: &'static str,
    url: &'static str,
    /// Substrings (lowercase) that mark the response as a geo-block page.
    block_tokens: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeStatus {
    /// Reachable end-to-end through this DNS.
    Unlocked,
    /// Service explicitly refused (HTTP 401/403/429/451 or geo-block page).
    Blocked,
    /// DNS didn't resolve or connection couldn't establish within timeout.
    Timeout,
    /// Unexpected client/transport error.
    Error(String),
}

impl ProbeStatus {
    pub fn is_unlocked(&self) -> bool {
        matches!(self, ProbeStatus::Unlocked)
    }

    /// Compact symbol for tables.
    pub fn symbol(&self) -> &'static str {
        match self {
            ProbeStatus::Unlocked => "✓",
            ProbeStatus::Blocked => "✗",
            ProbeStatus::Timeout => "⌛",
            ProbeStatus::Error(_) => "?",
        }
    }
}

impl fmt::Display for ProbeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeStatus::Unlocked => write!(f, "Unlocked"),
            ProbeStatus::Blocked => write!(f, "Blocked"),
            ProbeStatus::Timeout => write!(f, "Timeout"),
            ProbeStatus::Error(e) => write!(f, "Error: {e}"),
        }
    }
}

/// Prefer IPv4 when the resolver hands back both families (broadest device
/// compatibility for the probe connection).
fn pick_ip(addrs: &[IpAddr]) -> Option<IpAddr> {
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| addrs.first())
        .copied()
}

/// Walk a reqwest error's source chain down to the root cause message.
fn root_cause(e: &reqwest::Error) -> String {
    let mut msg = e.to_string();
    let mut cur = std::error::Error::source(e);
    while let Some(src) = cur {
        msg = src.to_string();
        cur = std::error::Error::source(src);
    }
    msg
}

/// Coarse reason a single request attempt failed, reduced to the only
/// distinction that affects classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendFailure {
    /// The connection could not be established (refused/reset before any
    /// response was seen): the strongest available signal of IP-level
    /// filtering toward the resolver's answer.
    ConnectRefused,
    /// The attempt elapsed without completing (connect or body-read timeout).
    Timeout,
    /// Any other client/transport failure (TLS hiccup, mid-body idle timeout,
    /// HTTP/2 stream error, ...). Ambiguous: *not* evidence of a geo-block.
    Request,
}

/// Final probe classification for a failed attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    Blocked,
    Timeout,
    Error,
}

/// Reduce a `reqwest` send error to what matters for classification.
/// `is_timeout()` is checked first so a connect *timeout* is reported as a
/// timeout rather than a refusal (`reqwest` also flags connect timeouts as
/// connect errors).
fn send_failure(e: &reqwest::Error) -> SendFailure {
    if e.is_timeout() {
        SendFailure::Timeout
    } else if e.is_connect() {
        SendFailure::ConnectRefused
    } else {
        SendFailure::Request
    }
}

/// Map a failed attempt to its final classification. Only a fast
/// connection-level refusal/reset counts as `Blocked`; every other transport
/// failure (including `is_request()`/`is_body()`) stays an ambiguous `Error`.
fn failure_kind(reason: SendFailure) -> FailureKind {
    match reason {
        SendFailure::ConnectRefused => FailureKind::Blocked,
        SendFailure::Timeout => FailureKind::Timeout,
        SendFailure::Request => FailureKind::Error,
    }
}

async fn probe_one(ip: IpAddr, def: ProbeDef) -> ProbeStatus {
    let host = def.host.to_string();
    let url = def.url.to_string();
    let tokens = def.block_tokens;

    let task = async move {
        let resolver = create_resolver(ip);
        let lookup = match resolver.lookup_ip(&host).await {
            Ok(l) => l,
            Err(_) => {
                return ProbeStatus::Timeout; // DNS itself didn't answer for this domain
            }
        };
        let addrs: Vec<IpAddr> = lookup.iter().collect();
        let resolved = match pick_ip(&addrs) {
            Some(r) => r,
            None => return ProbeStatus::Timeout,
        };

        let client = match Client::builder()
            .resolve(&host, SocketAddr::new(resolved, 443))
            .timeout(ATTEMPT_TIMEOUT)
            .connect_timeout(ATTEMPT_TIMEOUT)
            .build()
        {
            Ok(c) => c,
            Err(_) => return ProbeStatus::Error("client build failed".into()),
        };

        let mut attempt = 0;
        let resp = loop {
            attempt += 1;
            match client.get(&url).send().await {
                Ok(r) => break r,
                Err(e) => {
                    let reason = send_failure(&e);
                    // A single transport error is ambiguous (slow-streaming
                    // body, TLS hiccup, idle timeout mid-body, ...), so it is
                    // never a confident ✗ Blocked. Retry it once cheaply, then
                    // fall through to the final classification.
                    if reason == SendFailure::Request && attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        continue;
                    }
                    return match failure_kind(reason) {
                        // Fast connection-level refusal/reset toward the
                        // resolver's answer.
                        FailureKind::Blocked => ProbeStatus::Blocked,
                        FailureKind::Timeout => ProbeStatus::Timeout,
                        // Ambiguous client/transport failure.
                        FailureKind::Error => ProbeStatus::Error(root_cause(&e)),
                    };
                }
            }
        };

        let status = resp.status().as_u16();
        // Sniff a bounded prefix of the body for geo-block interstitials.
        let geo_blocked = if tokens.is_empty() {
            false
        } else {
            let mut acc: Vec<u8> = Vec::new();
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        acc.extend_from_slice(&bytes);
                        if acc.len() >= BODY_SCAN_BYTES {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let lower = String::from_utf8_lossy(&acc[..acc.len().min(BODY_SCAN_BYTES)])
                .to_ascii_lowercase();
            tokens.iter().any(|t| lower.contains(t))
        };
        if geo_blocked {
            return ProbeStatus::Blocked;
        }

        match status {
            200..=299 | 301..=399 => ProbeStatus::Unlocked,
            401 | 403 | 429 | 451 => ProbeStatus::Blocked,
            other => ProbeStatus::Error(format!("HTTP {other}")),
        }
    };

    match timeout(PROBE_TIMEOUT + Duration::from_secs(2), task).await {
        Ok(status) => status,
        Err(_) => ProbeStatus::Timeout,
    }
}

/// Probe every requested service concurrently for one resolver IP.
/// Results are keyed by service for deterministic table output.
pub async fn probe_services(ip: IpAddr, services: &[Service]) -> BTreeMap<Service, ProbeStatus> {
    let mut handles = tokio::task::JoinSet::new();
    for &svc in services {
        handles.spawn(async move {
            let def = svc.def();
            (svc, probe_one(ip, def).await)
        });
    }
    let mut out = BTreeMap::new();
    while let Some(joined) = handles.join_next().await {
        if let Ok((svc, status)) = joined {
            out.insert(svc, status);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_refusal_maps_to_blocked() {
        assert_eq!(
            failure_kind(SendFailure::ConnectRefused),
            FailureKind::Blocked
        );
    }

    #[test]
    fn request_error_is_ambiguous_not_blocked() {
        // is_request()/is_body() failures must never be a confident ✗ Blocked.
        assert_eq!(failure_kind(SendFailure::Request), FailureKind::Error);
    }

    #[test]
    fn timeout_maps_to_timeout() {
        assert_eq!(failure_kind(SendFailure::Timeout), FailureKind::Timeout);
    }
}
