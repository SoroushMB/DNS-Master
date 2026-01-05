use anyhow::{Result, Context};
use std::fs;
use std::path::Path;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum Distro {
    Arch,
    Debian,
    Ubuntu,
    Kali,
    Mint,
    Manjaro,
    Docker,
    AndroidSDK,
    Unknown,
}

impl Distro {
    pub fn from_id(id: &str) -> Self {
        match id.to_lowercase().as_str() {
            "arch" => Distro::Arch,
            "debian" => Distro::Debian,
            "ubuntu" => Distro::Ubuntu,
            "kali" => Distro::Kali,
            "linuxmint" => Distro::Mint,
            "manjaro" => Distro::Manjaro,
            _ => Distro::Unknown,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Distro::Arch => "Arch",
            Distro::Debian => "Debian",
            Distro::Ubuntu => "Ubuntu",
            Distro::Kali => "Kali",
            Distro::Mint => "Mint",
            Distro::Manjaro => "Manjaro",
            Distro::Docker => "Docker",
            Distro::AndroidSDK => "Android SDK",
            Distro::Unknown => "Unknown",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            Distro::Arch => "🏔️",
            Distro::Debian => "🌀",
            Distro::Ubuntu => "🧡",
            Distro::Kali => "🐉",
            Distro::Mint => "🍃",
            Distro::Manjaro => "💚",
            Distro::Docker => "🐳",
            Distro::AndroidSDK => "🤖",
            Distro::Unknown => "❓",
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Mirror {
    pub name: String,
    pub url: String,
    pub distro: Distro,
}

#[derive(Debug, Clone)]
pub struct MirrorTestResult {
    pub name: String,
    #[allow(dead_code)]
    pub url: String,
    pub speed_mbps: Option<f64>,
    pub error: Option<String>,
}

pub fn detect_distro() -> Distro {
    if Path::new("/etc/os-release").exists() {
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("ID=") {
                    let id = line.trim_start_matches("ID=").trim_matches('"');
                    return Distro::from_id(id);
                }
            }
        }
    }
    Distro::Unknown
}

pub async fn test_mirror_speed(url: &str) -> Result<f64> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let start = std::time::Instant::now();
    let response = timeout(Duration::from_secs(10), client.get(url).send()).await
        .context("Request timed out")??;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to connect: {}", response.status()));
    }

    let mut total_bytes = 0;
    let mut stream = response.bytes_stream();
    
    // We only download a portion to test speed (e.g. 1MB)
    let max_bytes = 1024 * 1024; 
    
    use futures_util::StreamExt;
    while let Some(item) = stream.next().await {
        let chunk = item.context("Failed to read chunk")?;
        total_bytes += chunk.len();
        if total_bytes >= max_bytes {
            break;
        }
        if start.elapsed().as_secs() > 7 { // Hard limit for speed test portion
            break;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
    
    Ok(speed_mbps)
}
