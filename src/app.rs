use crate::dns_utils::{run_full_test, DnsTestResult};
use std::net::IpAddr;
use tui_input::Input;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Input,
    Testing,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    Ip,
    Latency,
    DownloadSpeed,
}

pub struct App {
    pub state: AppState,
    pub dns_servers: Vec<IpAddr>,
    pub input: Input,
    pub results: Vec<DnsTestResult>,
    pub last_result: Option<DnsTestResult>,
    pub best_result: Option<DnsTestResult>,
    pub testing_index: usize,
    pub sort_column: SortColumn,
    pub sort_ascending: bool,
    pub should_quit: bool,
    pub error_message: Option<String>,
    pub status_message: Option<(String, bool)>, // (message, is_error)
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: AppState::Input,
            dns_servers: Vec::new(),
            input: Input::default(),
            results: Vec::new(),
            last_result: None,
            best_result: None,
            testing_index: 0,
            sort_column: SortColumn::DownloadSpeed,
            sort_ascending: false, // Descending by default (fastest first)
            should_quit: false,
            error_message: None,
            status_message: None,
        }
    }
}

impl App {
    pub fn new(initial_dns: Vec<IpAddr>) -> Self {
        let mut app = Self::default();
        app.dns_servers = initial_dns;
        app
    }

    /// Add a DNS server to the list
    pub fn add_dns_server(&mut self) {
        let input_value = self.input.value().trim().to_string();
        if let Ok(ip) = input_value.parse::<IpAddr>() {
            if !self.dns_servers.contains(&ip) {
                self.dns_servers.push(ip);
                self.error_message = None;
            } else {
                self.error_message = Some(format!("{} is already in the list", ip));
            }
        } else {
            self.error_message = Some(format!("Invalid IP address: {}", input_value));
        }
        self.input.reset();
    }

    /// Remove the last DNS server from the list
    pub fn remove_last_dns_server(&mut self) {
        self.dns_servers.pop();
    }

    /// Start testing
    pub fn start_testing(&mut self) {
        if !self.dns_servers.is_empty() {
            self.state = AppState::Testing;
            self.testing_index = 0;
            self.results.clear();
        }
    }

    /// Get the next DNS server to test
    pub fn get_current_test_target(&self) -> Option<IpAddr> {
        self.dns_servers.get(self.testing_index).copied()
    }

    /// Record a test result and advance to the next server
    pub fn record_result(&mut self, result: DnsTestResult) {
        self.last_result = Some(result.clone());

        // Update best result (higher speed is better, then lower latency)
        if let Some(best) = &self.best_result {
            let is_better = match (result.download_speed_mbps, best.download_speed_mbps) {
                (Some(s1), Some(s2)) if (s1 - s2).abs() > 0.01 => s1 > s2,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                _ => {
                    // Speeds are similar or both None, compare latency
                    match (result.latency, best.latency) {
                        (Some(l1), Some(l2)) => l1 < l2,
                        (Some(_), None) => true,
                        (None, Some(_)) => false,
                        _ => false,
                    }
                }
            };
            if is_better {
                self.best_result = Some(result.clone());
            }
        } else if result.error.is_none() && (result.latency.is_some() || result.download_speed_mbps.is_some()) {
            self.best_result = Some(result.clone());
        }

        self.results.push(result);
        self.testing_index += 1;

        if self.testing_index >= self.dns_servers.len() {
            self.finish_testing();
        }
    }

    /// Finish testing and show results
    fn finish_testing(&mut self) {
        self.state = AppState::Results;
        self.sort_results();
    }

    /// Sort results based on current sort column and direction
    pub fn sort_results(&mut self) {
        let ascending = self.sort_ascending;
        match self.sort_column {
            SortColumn::Ip => {
                self.results.sort_by(|a, b| {
                    let cmp = a.ip.to_string().cmp(&b.ip.to_string());
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Latency => {
                self.results.sort_by(|a, b| {
                    let cmp = a.latency.cmp(&b.latency);
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::DownloadSpeed => {
                self.results.sort_by(|a, b| {
                    let cmp = a
                        .download_speed_mbps
                        .partial_cmp(&b.download_speed_mbps)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
        }
    }

    /// Cycle sort column
    pub fn cycle_sort_column(&mut self) {
        self.sort_column = match self.sort_column {
            SortColumn::Ip => SortColumn::Latency,
            SortColumn::Latency => SortColumn::DownloadSpeed,
            SortColumn::DownloadSpeed => SortColumn::Ip,
        };
        self.sort_results();
    }

    /// Toggle sort direction
    pub fn toggle_sort_direction(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_results();
    }

    /// Reset to input state
    pub fn reset(&mut self) {
        self.state = AppState::Input;
        self.results.clear();
        self.last_result = None;
        self.best_result = None;
        self.testing_index = 0;
        self.status_message = None;
    }

    /// Apply the fastest DNS to the system
    pub fn apply_fastest_dns(&mut self) {
        if let Some(best) = &self.best_result {
            match crate::sys_dns::set_system_dns(best.ip) {
                Ok(_) => {
                    self.status_message = Some((format!("Successfully set system DNS to {}", best.ip), false));
                }
                Err(e) => {
                    self.status_message = Some((format!("Failed to set system DNS: {}", e), true));
                }
            }
        } else {
            self.status_message = Some(("No valid test results available.".to_string(), true));
        }
    }

    /// Run a single test (async)
    pub async fn run_test_for_current(&mut self) -> bool {
        if let Some(ip) = self.get_current_test_target() {
            let result = run_full_test(ip).await;
            self.record_result(result);
            true
        } else {
            false
        }
    }
}
