use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufReader;
use std::net::IpAddr;
use serde::Deserialize;
use crate::mirror_utils::{Distro, Mirror};

#[derive(Deserialize)]
struct DnsRecord {
    ip: String,
}

#[derive(Deserialize)]
struct MirrorRecord {
    name: String,
    url: String,
    distro: String,
}

pub fn load_mirrors(csv_path: &str, current_distro: Distro) -> Result<Vec<Mirror>> {
    let file = File::open(csv_path).with_context(|| format!("Failed to open Mirror CSV: {}", csv_path))?;
    let mut reader = csv::Reader::from_reader(file);
    
    let mut mirrors = Vec::new();
    for result in reader.deserialize() {
        let record: MirrorRecord = result.with_context(|| format!("Failed to parse Mirror record in: {}", csv_path))?;
        let distro = match record.distro.to_lowercase().as_str() {
            "arch" => Distro::Arch,
            "debian" => Distro::Debian,
            "ubuntu" => Distro::Ubuntu,
            "kali" => Distro::Kali,
            "mint" => Distro::Mint,
            "manjaro" => Distro::Manjaro,
            "docker" => Distro::Docker,
            "androidsdk" => Distro::AndroidSDK,
            _ => Distro::Unknown,
        };

        // Include mirror if it's for the current distro OR a global service
        if distro == current_distro || 
           distro == Distro::Docker || 
           distro == Distro::AndroidSDK {
            mirrors.push(Mirror {
                name: record.name,
                url: record.url,
                distro,
            });
        }
    }
    
    Ok(mirrors)
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
