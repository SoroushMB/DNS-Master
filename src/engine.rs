//! Shared benchmarking engine: bounded-parallel worker with epoch-based
//! cancellation. Used by both the TUI and the headless CLI mode.

use std::collections::{BTreeSet, HashMap};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinHandle;

use crate::catalog::{CatalogEntry, Category};
use crate::dns_utils;
use crate::probe::{ProbeStatus, Service, probe_services};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TargetRole {
    Primary,
    Secondary,
    Custom,
}

impl TargetRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetRole::Primary => "primary",
            TargetRole::Secondary => "secondary",
            TargetRole::Custom => "custom",
        }
    }
}

/// What to test: an IP plus the candidate it belongs to (for grouping).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetMeta {
    pub cand: Option<String>,
    pub ip: IpAddr,
    pub role: TargetRole,
    pub tags: Vec<Service>,
}

impl TargetMeta {
    pub fn custom(ip: IpAddr) -> Self {
        Self {
            cand: None,
            ip,
            role: TargetRole::Custom,
            tags: Vec::new(),
        }
    }

    pub fn label(&self) -> String {
        self.cand.clone().unwrap_or_else(|| self.ip.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Test { epoch: u64, meta: TargetMeta },
    Stop,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestResult {
    pub meta: TargetMeta,
    pub dns: dns_utils::DnsTestResult,
    pub probes: Vec<(Service, ProbeStatus)>,
}

impl TestResult {
    pub fn unlocked_count(&self) -> usize {
        self.probes.iter().filter(|(_, s)| s.is_unlocked()).count()
    }

    /// 0.0 .. 1.0 composite score:
    /// 40% download speed (normalized by best in batch), 30% unlocked services,
    /// 20% latency, 10% "resolver answered at all".
    pub fn score(&self, max_speed: Option<f64>) -> f64 {
        let speed_norm = match (self.dns.download_speed_mbps, max_speed) {
            (Some(s), Some(m)) if m > 0.0 => s / m,
            (Some(_), None) => 0.0,
            _ => 0.0,
        };
        let unlocked_norm = if self.probes.is_empty() {
            0.5 // no probes requested: neutral
        } else {
            self.unlocked_count() as f64 / self.probes.len() as f64
        };
        let latency_norm = match self.dns.latency {
            Some(l) => {
                let ms = l.as_secs_f64() * 1000.0;
                ((2500.0 - ms) / 2500.0).clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        let ok_norm = if self.dns.is_ok() { 1.0 } else { 0.0 };

        0.4 * speed_norm + 0.3 * unlocked_norm + 0.2 * latency_norm + 0.1 * ok_norm
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Score,
    Latency,
    Speed,
    Unlocked,
    Candidate,
}

pub fn max_download_speed(results: &[TestResult]) -> Option<f64> {
    results
        .iter()
        .filter_map(|r| r.dns.download_speed_mbps)
        .fold(None, |acc: Option<f64>, v| match acc {
            Some(m) if v > m => Some(v),
            Some(m) => Some(m),
            None => Some(v),
        })
}

/// In-place sort shared by the TUI and headless mode. Score columns need the
/// batch max speed for normalization.
pub fn sort_results(
    results: &mut [TestResult],
    column: SortColumn,
    ascending: bool,
    max_speed: Option<f64>,
) {
    let dir_factor = if ascending { 1 } else { -1 };
    results.sort_by(|a, b| {
        let ord = match column {
            SortColumn::Score => a
                .score(max_speed)
                .partial_cmp(&b.score(max_speed))
                .unwrap_or(std::cmp::Ordering::Equal),
            SortColumn::Latency => a.dns.latency.cmp(&b.dns.latency),
            SortColumn::Speed => a
                .dns
                .download_speed_mbps
                .partial_cmp(&b.dns.download_speed_mbps)
                .unwrap_or(std::cmp::Ordering::Equal),
            SortColumn::Unlocked => a.unlocked_count().cmp(&b.unlocked_count()),
            SortColumn::Candidate => a.meta.label().cmp(&b.meta.label()),
        };
        if dir_factor < 0 { ord.reverse() } else { ord }
    });
}

/// Which built-in entries start out selected (featured + matching filter).
pub fn default_included(
    builtin: &[CatalogEntry],
    category_filter: Option<Category>,
) -> BTreeSet<String> {
    builtin
        .iter()
        .filter(|e| e.featured && category_filter.is_none_or(|c| e.category == c))
        .map(|e| e.name.clone())
        .collect()
}

/// Target list: explicit CLI/imported IPs first, then each included catalog
/// entry as primary+secondary.
pub fn build_targets(
    custom: &[IpAddr],
    builtin: &[CatalogEntry],
    included: &BTreeSet<String>,
) -> Vec<TargetMeta> {
    let mut targets: Vec<TargetMeta> = Vec::new();
    let mut seen: BTreeSet<IpAddr> = BTreeSet::new();

    for &ip in custom {
        if seen.insert(ip) {
            targets.push(TargetMeta::custom(ip));
        }
    }
    for entry in builtin {
        if !included.contains(&entry.name) {
            continue;
        }
        let tags = entry.known_services();
        for (i, ip) in entry.ips().iter().enumerate() {
            let role = if i == 0 {
                TargetRole::Primary
            } else {
                TargetRole::Secondary
            };
            if seen.insert(*ip) {
                targets.push(TargetMeta {
                    cand: Some(entry.name.clone()),
                    ip: *ip,
                    role,
                    tags: tags.clone(),
                });
            }
        }
    }
    targets
}

/// Hard ceiling per target: DNS benchmark (≤7.5s) + probes (6s request window,
/// running concurrently) plus slack.
const TARGET_BUDGET: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct Reply {
    pub epoch: u64,
    pub res: TestResult,
}

type Engine = (mpsc::Sender<Command>, mpsc::Receiver<Reply>);

/// Start the background worker. Returns (command sender, reply receiver).
pub fn spawn_engine(lanes: usize, services: Arc<Vec<Service>>) -> Engine {
    let (tx_cmd, mut rx_cmd) = mpsc::channel::<Command>(8);
    let (tx_res, rx_res) = mpsc::channel::<Reply>(8);
    let sem = Arc::new(Semaphore::new(lanes.max(1)));

    tokio::spawn(async move {
        // epoch -> running task handles, so stale epochs can be aborted.
        let mut active: HashMap<u64, Vec<JoinHandle<()>>> = HashMap::new();

        while let Some(cmd) = rx_cmd.recv().await {
            match cmd {
                Command::Test { epoch, meta } => {
                    // Abort every older epoch's tasks: their answer can no longer
                    // influence the UI, but their bandwidth would corrupt the
                    // new measurements.
                    let stale: Vec<u64> = active.keys().copied().filter(|e| *e != epoch).collect();
                    for e in stale {
                        if let Some(handles) = active.remove(&e) {
                            for h in handles {
                                h.abort();
                            }
                        }
                    }

                    if let Ok(permit) = sem.clone().acquire_owned().await {
                        let services = services.clone();
                        let tx_res = tx_res.clone();
                        let handle = tokio::spawn(async move {
                            let res = run_target(meta, &services).await;
                            let _ = tx_res.send(Reply { epoch, res }).await;
                            drop(permit);
                        });
                        active.entry(epoch).or_default().push(handle);
                    }
                }
                Command::Stop => {
                    for (_, handles) in active.drain() {
                        for h in handles {
                            h.abort();
                        }
                    }
                }
            }
        }
        // Channel closed: abort whatever remains.
        for (_, handles) in active.drain() {
            for h in handles {
                h.abort();
            }
        }
    });

    (tx_cmd, rx_res)
}

async fn run_target(meta: TargetMeta, services: &[Service]) -> TestResult {
    let ip = meta.ip;

    // 1) Classic benchmark: latency + 1MB download through the resolver.
    let dns = match tokio::time::timeout(TARGET_BUDGET / 2, dns_utils::run_full_test(ip)).await {
        Ok(res) => res,
        Err(_) => {
            let mut r = dns_utils::DnsTestResult::new(ip);
            r.error = Some("Benchmark timed out".into());
            r
        }
    };

    // 2) Service-access probes, running concurrently.
    let probes = if services.is_empty() {
        Vec::new()
    } else {
        let map = probe_services(ip, services).await;
        services
            .iter()
            .filter_map(|s| map.get(s).map(|st| (*s, st.clone())))
            .collect()
    };

    TestResult { meta, dns, probes }
}

/// Run a batch to completion in headless mode (no TUI), applying a watchdog
/// so a hung worker can never wed the CLI. Results are returned unsorted
/// (call `sort_results` with max_download_speed before printing).
pub async fn collect_batch(
    targets: Vec<TargetMeta>,
    lanes: usize,
    services: Arc<Vec<Service>>,
) -> Vec<TestResult> {
    if targets.is_empty() {
        return Vec::new();
    }
    let (tx_cmd, mut rx_res) = spawn_engine(lanes, services);
    let epoch: u64 = 1;

    for meta in &targets {
        let _ = tx_cmd
            .send(Command::Test {
                epoch,
                meta: meta.clone(),
            })
            .await;
    }

    let deadline =
        tokio::time::Instant::now() + TARGET_BUDGET * (targets.len().div_ceil(lanes.max(1)) as u32);
    let mut results = Vec::with_capacity(targets.len());
    while results.len() < targets.len() && tokio::time::Instant::now() < deadline {
        let reply =
            tokio::time::timeout(deadline - tokio::time::Instant::now(), rx_res.recv()).await;
        match reply {
            Ok(Some(r)) if r.epoch == epoch => results.push(r.res),
            Ok(Some(_)) => continue, // stale epoch, ignore
            Ok(None) => break,       // engine gone
            Err(_) => break,         // watchdog fired
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns_utils::DnsTestResult;

    fn fake_result(speed: Option<f64>, latency: u64, ok: bool) -> TestResult {
        TestResult {
            meta: TargetMeta::custom("1.1.1.1".parse().unwrap()),
            dns: DnsTestResult {
                ip: "1.1.1.1".parse().unwrap(),
                latency: Some(Duration::from_millis(latency)),
                download_speed_mbps: speed,
                error: if ok { None } else { Some("boom".into()) },
            },
            probes: vec![
                (Service::Gemini, ProbeStatus::Unlocked),
                (Service::YouTube, ProbeStatus::Blocked),
            ],
        }
    }

    #[test]
    fn scoring_prefers_speed_and_unlocks() {
        let fast = fake_result(Some(50.0), 100, true);
        let slow = fake_result(Some(5.0), 400, true);
        let max = max_download_speed(&[fast.clone(), slow.clone()]);
        assert!(fast.score(max) > slow.score(max));

        // A blocked-everything result scores worse.
        let mut dull = fake_result(Some(50.0), 100, true);
        dull.probes = vec![
            (Service::Gemini, ProbeStatus::Timeout),
            (Service::YouTube, ProbeStatus::Timeout),
        ];
        assert!(fast.score(max) > dull.score(max));
    }

    #[test]
    fn sorting_respects_direction() {
        let mut v = vec![
            fake_result(Some(5.0), 400, true),
            fake_result(Some(50.0), 100, true),
        ];
        let max = max_download_speed(&v);
        sort_results(&mut v, SortColumn::Speed, false, max);
        assert_eq!(v[0].dns.download_speed_mbps, Some(50.0));
        sort_results(&mut v, SortColumn::Speed, true, max);
        assert_eq!(v[0].dns.download_speed_mbps, Some(5.0));
    }

    #[test]
    fn target_building_dedups_and_roles() {
        let entry = CatalogEntry {
            name: "Google Public DNS".into(),
            fa_name: String::new(),
            category: crate::catalog::Category::Public,
            ips: vec!["8.8.8.8".into(), "8.8.4.4".into()],
            doh: String::new(),
            tags: vec![],
            featured: true,
            carrier_only: false,
            notes: String::new(),
            links: vec![],
        };
        let mut included = BTreeSet::new();
        included.insert("Google Public DNS".to_string());
        let custom = vec![
            "8.8.8.8".parse::<IpAddr>().unwrap(),
            "9.9.9.9".parse().unwrap(),
        ];

        let targets = build_targets(&custom, &[entry], &included);
        // custom 8.8.8.8 first => catalog primary dedups; 9.9.9.9 + 8.8.4.4 join
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].role, TargetRole::Custom);
        assert!(targets.iter().any(|t| t.role == TargetRole::Secondary));
    }
}
