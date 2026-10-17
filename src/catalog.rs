//! Built-in DNS catalog: curated, versioned, embedded in the binary.
//!
//! The catalog ships inside the executable via `include_str!` so the app works
//! fully offline. User-provided CSV/JSON files (see `file_loader`) can add
//! more servers or override built-in entries by IP.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::IpAddr;

use crate::probe::Service;

pub const BUILTIN_CATALOG: &str = include_str!("../assets/dns_catalog.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Antisanction,
    Public,
    Gaming,
    Privacy,
    Families,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Antisanction => "🇮🇷 Anti-Sanction",
            Category::Public => "🌍 Public",
            Category::Gaming => "🎮 Gaming",
            Category::Privacy => "🛡️ Privacy",
            Category::Families => "👨‍👩‍👧 Families",
        }
    }
}

pub fn parse_category(s: &str) -> Result<Category> {
    let id = s.trim().to_lowercase();
    match id.as_str() {
        "antisanction" | "anti-sanction" | "sanctions" | "ایران" => Ok(Category::Antisanction),
        "public" => Ok(Category::Public),
        "gaming" | "game" => Ok(Category::Gaming),
        "privacy" => Ok(Category::Privacy),
        "families" | "family" => Ok(Category::Families),
        "all" => anyhow::bail!("'all' is not a single category"),
        other => anyhow::bail!(
            "Unknown category '{other}'. Options: antisanction, public, gaming, privacy, families"
        ),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    #[serde(default)]
    pub fa_name: String,
    pub category: Category,
    /// Primary first, secondary second (if present).
    pub ips: Vec<String>,
    #[serde(default)]
    pub doh: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub featured: bool,
    /// Force the "domestic-only" warning even for public-range Iranian carrier IPs.
    #[serde(default)]
    pub carrier_only: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub links: Vec<String>,
}

impl CatalogEntry {
    /// Parsed addresses. May contain fewer than two entries (e.g. single-IP services).
    pub fn ips(&self) -> Vec<IpAddr> {
        self.ips
            .iter()
            .filter_map(|s| s.trim().parse::<IpAddr>().ok())
            .collect()
    }

    /// `true` when every announced resolver address sits in private/carrier
    /// space (10.x, 172.16-31.x, 192.168.x) — these only resolve on Iranian ISPs.
    pub fn iran_only(&self) -> bool {
        self.carrier_only || {
            let ips = self.ips();
            !ips.is_empty()
                && ips.iter().all(|ip| match ip {
                    IpAddr::V4(v4) => v4.is_private(),
                    IpAddr::V6(v6) => v6.is_loopback(),
                })
        }
    }

    /// Services this entry is *known* for — used for tagging & home-screen chips.
    pub fn known_services(&self) -> Vec<Service> {
        self.tags
            .iter()
            .filter_map(|t| Service::from_tag(t))
            .collect()
    }

    /// `true` when the entry is a *content-filtering* profile that blocks adult
    /// sites — Cloudflare Families (adult), CleanBrowsing family/adult, AdGuard
    /// Family, OpenDNS FamilyShield and Yandex Family. These carry the
    /// `adult-filter` catalog tag so the UI can label them unmistakably.
    ///
    /// Such resolvers **reduce** the number of sites you can reach (the opposite
    /// of "open as many sites as possible"), so the Home screen flags them and
    /// the details pane spells out the trade-off.
    pub fn blocks_adult(&self) -> bool {
        self.tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case("adult-filter"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogFile {
    pub version: u32,
    #[serde(default)]
    pub updated: String,
    pub entries: Vec<CatalogEntry>,
}

pub fn load_builtin() -> Result<CatalogFile> {
    let file: CatalogFile = serde_json::from_str(BUILTIN_CATALOG)
        .with_context(|| "Failed to parse embedded DNS catalog (build artifact issue)")?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builtin() {
        let cat = load_builtin().expect("builtin catalog must parse");
        assert!(cat.version >= 2);
        assert!(!cat.entries.is_empty());
        // Primary first / secondary second, at least one resolvable IP each.
        for entry in &cat.entries {
            assert!(!entry.name.is_empty());
            assert!(
                !entry.ips().is_empty(),
                "entry {} has no valid IP",
                entry.name
            );
        }
    }

    #[test]
    fn flags_private_range_entries() {
        let cat = load_builtin().unwrap();
        let p403 = cat.entries.iter().find(|e| e.name.contains("403")).unwrap();
        assert!(p403.iran_only());
        let pishgaman = cat
            .entries
            .iter()
            .find(|e| e.name.contains("Pishgaman"))
            .unwrap();
        assert!(pishgaman.iran_only());
        let google = cat
            .entries
            .iter()
            .find(|e| e.name.contains("Google"))
            .unwrap();
        assert!(!google.iran_only());
    }

    #[test]
    fn flags_adult_filters() {
        let cat = load_builtin().unwrap();
        // Content-filtering profiles must be tagged ...
        for name in [
            "Cloudflare Families (adult)",
            "CleanBrowsing (adult)",
            "CleanBrowsing (family)",
            "AdGuard Family",
            "OpenDNS FamilyShield",
            "Yandex Family",
        ] {
            let e = cat
                .entries
                .iter()
                .find(|e| e.name == name)
                .unwrap_or_else(|| panic!("missing entry {name}"));
            assert!(
                e.blocks_adult(),
                "{name} should be flagged as an adult filter"
            );
        }
        // ... and malware-only / plain resolvers must not be.
        for name in [
            "Google Public DNS",
            "Cloudflare Families (malware)",
            "CleanBrowsing (security)",
        ] {
            let e = cat.entries.iter().find(|e| e.name == name).unwrap();
            assert!(!e.blocks_adult(), "{name} must NOT be an adult filter");
        }
    }

    #[test]
    fn maps_service_tags() {
        let cat = load_builtin().unwrap();
        let shecan = cat.entries.iter().find(|e| e.name == "Shecan").unwrap();
        let services = shecan.known_services();
        assert!(services.contains(&Service::Gemini));
        assert!(services.contains(&Service::YouTube));
    }
}
