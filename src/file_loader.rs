use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufReader;
use std::net::IpAddr;
use serde::Deserialize;

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
        let ip = record.ip.parse::<IpAddr>()
            .with_context(|| format!("Invalid IP address in JSON: {}", record.ip))?;
        ips.push(ip);
    }
    Ok(ips)
}

/// Load DNS addresses from a CSV file.
/// Expected format: ip
///                 8.8.8.8
///                 1.1.1.1
pub fn load_csv(path: &str) -> Result<Vec<IpAddr>> {
    let file = File::open(path).with_context(|| format!("Failed to open CSV file: {}", path))?;
    let mut reader = csv::Reader::from_reader(file);
    
    let mut ips = Vec::new();
    for result in reader.deserialize() {
        let record: DnsRecord = result.with_context(|| format!("Failed to parse CSV record in: {}", path))?;
        let ip = record.ip.parse::<IpAddr>()
            .with_context(|| format!("Invalid IP address in CSV: {}", record.ip))?;
        ips.push(ip);
    }
    Ok(ips)
}
