//! Export test results to JSON or CSV.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::engine::TestResult;
use crate::probe::ProbeStatus;

pub fn write_results(results: &[TestResult], path: &str, csv: bool) -> Result<usize> {
    if csv {
        write_csv(results, path)
    } else {
        write_json(results, path)
    }
}

fn write_json(results: &[TestResult], path: &str) -> Result<usize> {
    let json = serde_json::to_string_pretty(results)?;
    fs::write(path, json + "\n").with_context(|| format!("Failed writing {path}"))?;
    Ok(results.len())
}

fn write_csv(results: &[TestResult], path: &str) -> Result<usize> {
    let mut writer = csv::Writer::from_path(Path::new(path))
        .with_context(|| format!("Failed writing {path}"))?;
    writer.write_record([
        "candidate",
        "ip",
        "role",
        "latency_ms",
        "download_mbps",
        "unlocked",
        "checked",
        "status",
        "error",
    ])?;
    let mut n = 0;
    for r in results {
        let latency = r
            .dns
            .latency
            .map(|d| format!("{:.2}", d.as_secs_f64() * 1000.0))
            .unwrap_or_default();
        let speed = r
            .dns
            .download_speed_mbps
            .map(|s| format!("{:.2}", s))
            .unwrap_or_default();
        // services in probe order: G✓Y✗H⌛
        let checked: String = r
            .probes
            .iter()
            .map(|(s, st)| format!("{}{}", s.glyph(), st.symbol()))
            .collect::<Vec<_>>()
            .join("");
        let unlocked_display = r.unlocked_count().to_string();
        writer.write_record([
            r.meta.label(),
            r.dns.ip.to_string(),
            r.meta.role.as_str().to_string(),
            latency,
            speed,
            unlocked_display,
            checked,
            probe_summary(r),
            r.dns.error.clone().unwrap_or_default(),
        ])?;
        n += 1;
    }
    writer.flush()?;
    Ok(n)
}

fn probe_summary(r: &TestResult) -> String {
    if r.probes.is_empty() {
        return "-".into();
    }
    r.probes
        .iter()
        .map(|(s, st)| {
            format!(
                "{}:{}",
                s.label(),
                match st {
                    ProbeStatus::Unlocked => "OK",
                    ProbeStatus::Blocked => "BLOCKED",
                    ProbeStatus::Timeout => "TIMEOUT",
                    ProbeStatus::Error(e) => e.as_str(),
                }
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns_utils::DnsTestResult;
    use crate::engine::TargetMeta;

    #[test]
    fn writes_json_and_csv() {
        let r = TestResult {
            meta: TargetMeta {
                cand: Some("Test".to_string()),
                ip: "1.2.3.4".parse().unwrap(),
                role: crate::engine::TargetRole::Primary,
                tags: vec![],
            },
            dns: DnsTestResult {
                ip: "1.2.3.4".parse().unwrap(),
                latency: Some(std::time::Duration::from_millis(42)),
                download_speed_mbps: Some(12.5),
                error: None,
            },
            probes: vec![(crate::probe::Service::Gemini, ProbeStatus::Unlocked)],
        };

        let dir = std::env::temp_dir().join("dns-master-test");
        std::fs::create_dir_all(&dir).unwrap();
        let jpath = dir.join("out.json");
        let cpath = dir.join("out.csv");

        assert_eq!(
            write_json(std::slice::from_ref(&r), jpath.to_str().unwrap()).unwrap(),
            1
        );
        assert_eq!(write_csv(&[r], cpath.to_str().unwrap()).unwrap(), 1);
        assert!(jpath.exists());
        let csv_text = fs::read_to_string(&cpath).unwrap();
        assert!(csv_text.contains("Test,1.2.3.4,primary,42"));
    }
}
