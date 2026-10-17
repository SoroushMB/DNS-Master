//! Application state: catalog selection on the Home screen, bounded-parallel
//! testing with epoch-based cancellation, and ranked results.

use std::collections::BTreeSet;
use std::net::IpAddr;
use std::time::Instant;
use tui_input::Input;

use crate::catalog::{CatalogEntry, Category};
use crate::engine::{
    Command, Reply, SortColumn, TargetMeta, TestResult, build_targets, default_included,
    max_download_speed,
};
use crate::probe::Service;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Home,
    Testing,
    Results,
}

/// Watchdog slack added to the per-wave budget; even a wedged worker task
/// can never make the UI wait forever.
const DEADLINE_SLACK_SECS: u64 = 10;

/// Per-target hard budget used for watchdog math (seconds; matches engine).
const TARGET_BUDGET_SECS: u64 = 20;

pub struct App {
    pub state: AppState,

    // Home screen: built-in catalog + user additions.
    pub builtin: Vec<CatalogEntry>,
    pub catalog_version: u32,
    pub catalog_updated: String,
    pub included: BTreeSet<String>,
    pub category_filter: Option<Category>,
    pub custom: Vec<IpAddr>,
    pub input: Input,
    pub cursor: usize,
    pub error_message: Option<String>,

    // Testing/progress.
    pub targets: Vec<TargetMeta>,
    pub epoch: u64,
    pub lanes: usize,
    pub services: Vec<Service>,
    pub next_dispatch: usize,
    pub in_flight: Vec<TargetMeta>,
    pub last_result: Option<TestResult>,
    pub deadline: Option<Instant>,

    // Results.
    pub results: Vec<TestResult>,
    pub sort_column: SortColumn,
    pub sort_ascending: bool,
    pub table_offset: usize,
    /// Rows visible in the results table, refreshed on every draw (see `ui::render_results`).
    pub table_viewport: usize,
    pub export_path: Option<String>,

    // Plumbing.
    pub should_quit: bool,
    pub status_message: Option<(String, bool)>, // (message, is_error)
    pub tick_count: u64,
    pub tx: Option<tokio::sync::mpsc::Sender<Command>>,
    pub rx: Option<tokio::sync::mpsc::Receiver<Reply>>,
}

impl App {
    pub fn new(
        initial_dns: Vec<IpAddr>,
        builtin: Vec<CatalogEntry>,
        catalog_version: u32,
        catalog_updated: String,
        category_filter: Option<Category>,
        lanes: usize,
        services: Vec<Service>,
    ) -> Self {
        let included = default_included(&builtin, category_filter);
        Self {
            state: AppState::Home,
            builtin,
            catalog_version,
            catalog_updated,
            included,
            category_filter,
            custom: initial_dns,
            input: Input::default(),
            cursor: 0,
            error_message: None,
            targets: Vec::new(),
            epoch: 0,
            lanes: lanes.max(1),
            services,
            next_dispatch: 0,
            in_flight: Vec::new(),
            last_result: None,
            deadline: None,
            results: Vec::new(),
            sort_column: SortColumn::Score,
            sort_ascending: false,
            table_offset: 0,
            table_viewport: 0,
            export_path: None,
            should_quit: false,
            status_message: None,
            tick_count: 0,
            tx: None,
            rx: None,
        }
    }

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
    }

    // ---------------------------------------------------------------- Home

    fn visible_entries(&self) -> Vec<usize> {
        self.builtin
            .iter()
            .enumerate()
            .filter(|(_, e)| self.category_filter.is_none_or(|c| e.category == c))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn move_cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_cursor_down(&mut self) {
        let len = self.visible_entries().len();
        if len > 0 {
            self.cursor = (self.cursor + 1).min(len - 1);
        }
    }

    /// Toggle whether the entry under the cursor participates in the next run.
    pub fn toggle_include(&mut self) {
        let Some(entry_idx) = self.visible_entries().get(self.cursor).copied() else {
            return;
        };
        let Some(entry) = self.builtin.get(entry_idx) else {
            return;
        };
        if self.included.contains(&entry.name) {
            self.included.remove(&entry.name);
        } else {
            self.included.insert(entry.name.clone());
        }
    }

    /// Select every visible entry at once, or clear them if all are already
    /// selected. Respects the active category filter. Backs the Home `a` key:
    /// "benchmark ALL resolvers" instead of only the featured defaults.
    pub fn toggle_all_visible(&mut self) {
        let visible: Vec<String> = self
            .builtin
            .iter()
            .filter(|e| self.category_filter.is_none_or(|c| e.category == c))
            .map(|e| e.name.clone())
            .collect();
        if visible.is_empty() {
            return;
        }
        let all_in = visible.iter().all(|n| self.included.contains(n));
        for name in visible {
            if all_in {
                self.included.remove(&name);
            } else {
                self.included.insert(name);
            }
        }
    }

    /// Add the typed IP as a custom server.
    pub fn add_custom_from_input(&mut self) {
        let value = self.input.value().trim().to_string();
        match value.parse::<IpAddr>() {
            Ok(ip) => {
                if self.custom.contains(&ip) {
                    self.error_message = Some(format!("{} is already in the list", ip));
                } else {
                    self.custom.push(ip);
                    self.error_message = None;
                }
            }
            Err(_) => {
                let shown = if value.is_empty() {
                    "<empty>".to_string()
                } else {
                    value
                };
                self.error_message = Some(format!("Invalid IP address: {}", shown));
            }
        }
        self.input.reset();
    }

    pub fn remove_last_custom(&mut self) {
        self.custom.pop();
    }

    // ------------------------------------------------------------- Testing

    pub fn start_testing(&mut self) {
        let targets = build_targets(&self.custom, &self.builtin, &self.included);
        if targets.is_empty() {
            self.error_message =
                Some("Nothing selected — include catalog entries (space) or add IPs".into());
            return;
        }
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch_start(targets);
    }

    /// Re-run the same target set with a fresh epoch.
    pub fn rerun(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch_start(self.targets.clone());
    }

    fn epoch_start(&mut self, targets: Vec<TargetMeta>) {
        self.state = AppState::Testing;
        self.targets = targets;
        self.results.clear();
        self.in_flight.clear();
        self.last_result = None;
        self.next_dispatch = 0;
        self.table_offset = 0;
        self.error_message = None;
        self.status_message = None;

        let waves = self.targets.len().div_ceil(self.lanes);
        self.deadline = Some(
            Instant::now()
                + std::time::Duration::from_secs(
                    waves as u64 * TARGET_BUDGET_SECS + DEADLINE_SLACK_SECS,
                ),
        );

        for _ in 0..self.lanes.min(self.targets.len()) {
            self.dispatch_next();
        }
    }

    fn dispatch_next(&mut self) {
        if self.next_dispatch >= self.targets.len() {
            return;
        }
        let meta = self.targets[self.next_dispatch].clone();
        self.next_dispatch += 1;
        self.in_flight.push(meta.clone());
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(Command::Test {
                epoch: self.epoch,
                meta,
            });
        }
    }

    /// Drain worker replies; ignore stale epochs; watchdog stale runs.
    pub fn pump_results(&mut self) {
        if self.state != AppState::Testing || self.rx.is_none() {
            return;
        }

        while let Ok(reply) = self.rx.as_mut().unwrap().try_recv() {
            if reply.epoch != self.epoch {
                continue;
            }
            self.handle_result(reply.res);
        }

        if let Some(deadline) = self.deadline
            && Instant::now() > deadline
        {
            let in_flight = std::mem::take(&mut self.in_flight);
            for meta in in_flight {
                let mut res = crate::dns_utils::DnsTestResult::new(meta.ip);
                res.error = Some("Aborted (watchdog deadline reached)".into());
                let probes = self
                    .services
                    .iter()
                    .map(|s| (*s, crate::probe::ProbeStatus::Timeout))
                    .collect();
                self.record_done(TestResult {
                    meta,
                    dns: res,
                    probes,
                });
            }
            // Deadline hit: never leave the user hanging.
            self.finish_testing();
        }
    }

    fn handle_result(&mut self, res: TestResult) {
        self.in_flight.retain(|m| m.ip != res.meta.ip);
        self.record_done(res);
        if self.next_dispatch < self.targets.len() && self.state == AppState::Testing {
            self.dispatch_next();
        }
        if self.state == AppState::Testing && self.in_flight.is_empty() {
            self.finish_testing();
        }
    }

    fn record_done(&mut self, res: TestResult) {
        self.last_result = Some(res.clone());
        self.results.push(res);
    }

    fn finish_testing(&mut self) {
        let max = max_download_speed(&self.results);
        crate::engine::sort_results(
            &mut self.results,
            self.sort_column,
            self.sort_ascending,
            max,
        );
        self.state = AppState::Results;
        self.deadline = None;
    }

    /// Cancel current runs; return to Home. Old-epoch results are dropped.
    pub fn reset_to_home(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(Command::Stop);
        }
        self.state = AppState::Home;
        self.deadline = None;
    }

    // ------------------------------------------------------------ Results

    pub fn best(&self) -> Option<&TestResult> {
        let max = max_download_speed(&self.results);
        self.results.iter().max_by(|a, b| {
            a.score(max)
                .partial_cmp(&b.score(max))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn max_speed(&self) -> Option<f64> {
        max_download_speed(&self.results)
    }

    pub fn cycle_sort_column(&mut self) {
        self.sort_column = match self.sort_column {
            SortColumn::Score => SortColumn::Unlocked,
            SortColumn::Unlocked => SortColumn::Latency,
            SortColumn::Latency => SortColumn::Speed,
            SortColumn::Speed => SortColumn::Candidate,
            SortColumn::Candidate => SortColumn::Score,
        };
        self.sort_results();
    }

    pub fn toggle_sort_direction(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_results();
    }

    fn sort_results(&mut self) {
        let max = self.max_speed();
        crate::engine::sort_results(
            &mut self.results,
            self.sort_column,
            self.sort_ascending,
            max,
        );
        self.table_offset = 0;
    }

    pub fn scroll_table(&mut self, delta: i32) {
        let len = self.results.len();
        if len == 0 {
            return;
        }
        // Never scroll past the last full page: the furthest valid offset shows
        // the final `viewport` rows without trailing blank space.
        let viewport = self.table_viewport.max(1);
        let max_offset = len.saturating_sub(viewport);
        let cur = self.table_offset as i64 + delta as i64;
        self.table_offset = cur.clamp(0, max_offset as i64) as usize;
    }

    /// Apply the best result's IP to the system (see sys_dns docs).
    pub fn apply_best(&mut self) {
        if let Some(best) = self.best() {
            match crate::sys_dns::set_system_dns(best.dns.ip) {
                Ok(_) => {
                    self.status_message =
                        Some((format!("System DNS set to {}", best.dns.ip), false));
                }
                Err(e) => {
                    self.status_message = Some((format!("Failed to set system DNS: {e}"), true));
                }
            }
        } else {
            self.status_message = Some(("No valid results available.".into(), true));
        }
    }

    pub fn set_export_path(&mut self, path: String) {
        self.export_path = Some(path);
    }

    pub fn export_results(&mut self) {
        // Default to JSON in the current working directory when no --export
        // path was configured, so pressing `e` always produces a file.
        let path = self
            .export_path
            .clone()
            .unwrap_or_else(|| "dns-results.json".to_string());
        let is_csv = path.ends_with(".csv");
        match crate::export::write_results(&self.results, &path, is_csv) {
            Ok(n) => {
                self.status_message = Some((format!("Exported {} results to {}", n, path), false))
            }
            Err(e) => self.status_message = Some((format!("Export failed: {e}"), true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, cat: Category, featured: bool) -> CatalogEntry {
        CatalogEntry {
            name: name.into(),
            fa_name: String::new(),
            category: cat,
            ips: vec!["1.1.1.1".into()],
            doh: String::new(),
            tags: vec![],
            featured,
            carrier_only: false,
            notes: String::new(),
            links: vec![],
        }
    }

    fn app_with(builtin: Vec<CatalogEntry>, filter: Option<Category>) -> App {
        App::new(Vec::new(), builtin, 1, "test".into(), filter, 2, Vec::new())
    }

    #[test]
    fn select_all_includes_every_visible_entry() {
        let builtin = vec![
            entry("Featured", Category::Public, true),
            entry("Plain", Category::Public, false),
        ];
        let mut app = app_with(builtin, None);
        // Default: only the featured entry.
        assert_eq!(app.included.len(), 1);
        assert!(app.included.contains("Featured"));

        app.toggle_all_visible();
        assert_eq!(app.included.len(), 2);
        assert!(app.included.contains("Featured") && app.included.contains("Plain"));

        // A second press clears everything.
        app.toggle_all_visible();
        assert!(app.included.is_empty());
    }

    #[test]
    fn select_all_respects_category_filter() {
        let builtin = vec![
            entry("Pub", Category::Public, false),
            entry("Priv", Category::Privacy, false),
        ];
        let mut app = app_with(builtin, Some(Category::Privacy));
        app.toggle_all_visible();
        assert_eq!(app.included.len(), 1);
        assert!(app.included.contains("Priv"));
    }
}
