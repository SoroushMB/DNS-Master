//! User-supplied server lists (adds to — or replaces — the built-in catalog).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::net::IpAddr;

#[derive(Deserialize)]
struct DnsRecord {
    ip: String,
}

/// Load DNS addresses from a JSON file.
/// Expected format: [{"ip": "8.8.8.8"}, {"ip": "1.1.1.1"}]
pub fn load_json(path: &str) -> Result<Vec<IpAddr>> {
    let file = File::open(path).with_context(|| format!("Failed to open JSON file: {}", path))?;
    let reader = BufReader::new(file);
    let records: Vec<DnsRecord> = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse JSON file: {}", path))?;

    let mut ips = Vec::new();
    for record in records {
        let ip = record
            .ip
            .parse::<IpAddr>()
            .with_context(|| format!("Invalid IP address in JSON: {}", record.ip))?;
        ips.push(ip);
    }
    Ok(ips)
}

/// Load DNS addresses from a CSV file with a header row `ip`.
pub fn load_csv(path: &str) -> Result<Vec<IpAddr>> {
    let file = File::open(path).with_context(|| format!("Failed to open CSV file: {}", path))?;
    let mut reader = csv::Reader::from_reader(file);

    let mut ips = Vec::new();
    for result in reader.deserialize() {
        let record: DnsRecord =
            result.with_context(|| format!("Failed to parse CSV record in: {}", path))?;
        let ip = record
            .ip
            .parse::<IpAddr>()
            .with_context(|| format!("Invalid IP address in CSV: {}", record.ip))?;
        ips.push(ip);
    }
    Ok(ips)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_and_json_examples_parse() {
        let from_csv = load_csv("examples/dns.csv").expect("examples/dns.csv");
        assert!(!from_csv.is_empty());
        let from_json = load_json("examples/dns.json").expect("examples/dns.json");
        assert!(!from_json.is_empty());
        assert_eq!(from_csv.len(), from_json.len());
    }
}
