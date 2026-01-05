mod mirror_utils;
mod app;
mod dns_utils;
mod ui;
mod file_loader;
mod sys_dns;

use anyhow::{Result, Context};
use app::{App, AppState};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::net::IpAddr;
use std::time::Duration;
use tui_input::backend::crossterm::EventHandler;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Initial DNS server IPs
    #[arg(short, long, value_delimiter = ',')]
    dns: Vec<IpAddr>,

    /// Path to a JSON file containing DNS IPs [{"ip": "..."}]
    #[arg(long)]
    json: Option<String>,

    /// Path to a CSV file containing DNS IPs (header "ip" required)
    #[arg(long)]
    csv: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let mut initial_dns = cli.dns;
    
    if let Some(json_path) = cli.json {
        let mut ips = file_loader::load_json(&json_path)
            .with_context(|| format!("Failed to load DNS from JSON: {}", json_path))?;
        initial_dns.append(&mut ips);
    }
    
    if let Some(csv_path) = cli.csv {
        let mut ips = file_loader::load_csv(&csv_path)
            .with_context(|| format!("Failed to load DNS from CSV: {}", csv_path))?;
        initial_dns.append(&mut ips);
    }
    
    // De-duplicate
    initial_dns.sort();
    initial_dns.dedup();

    let mut app = App::new(initial_dns);
    
    // Load mirrors
    let mirrors_path = "examples/mirrors.csv";
    match file_loader::load_mirrors(mirrors_path, app.detected_distro.clone()) {
        Ok(mirrors) => app.mirrors = mirrors,
        Err(e) => eprintln!("Warning: Failed to load mirrors: {}", e),
    }

    // Setup channels for background benchmarking
    let (tx_target, mut rx_target) = tokio::sync::mpsc::channel::<app::TestTarget>(1);
    let (tx_result, rx_result) = tokio::sync::mpsc::channel::<app::TestResult>(1);

    app.tx = Some(tx_target);
    app.rx = Some(rx_result);

    // Spawn background worker
    tokio::spawn(async move {
        while let Some(target) = rx_target.recv().await {
            match target {
                app::TestTarget::Dns(ip) => {
                    let res = dns_utils::run_full_test(ip).await;
                    let _ = tx_result.send(app::TestResult::Dns(res)).await;
                }
                app::TestTarget::Mirror(mirror) => {
                    match mirror_utils::test_mirror_speed(&mirror.url).await {
                        Ok(speed) => {
                            let _ = tx_result.send(app::TestResult::Mirror(mirror_utils::MirrorTestResult {
                                name: mirror.name,
                                url: mirror.url,
                                speed_mbps: Some(speed),
                                error: None,
                            })).await;
                        }
                        Err(e) => {
                            let _ = tx_result.send(app::TestResult::Mirror(mirror_utils::MirrorTestResult {
                                name: mirror.name,
                                url: mirror.url,
                                speed_mbps: None,
                                error: Some(e.to_string()),
                            })).await;
                        }
                    }
                }
            }
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(50);
    
    loop {
        // Draw UI
        terminal.draw(|f| ui::ui(f, &app))?;

        // Handle testing state specially - results are handled in app.update()
        if app.state == AppState::Testing {
            // Poll for events frequently
            if event::poll(Duration::from_millis(5))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                        app.should_quit = true;
                    }
                }
            }

            if app.should_quit {
                return Ok(());
            }

            // Process results and dispatch next tasks
            app.update();
            app.tick();
            continue;
        }

        // Normal state - block with timeout for animations
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.state {
                    AppState::Input => match key.code {
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            app.add_dns_server();
                        }
                        KeyCode::Tab => {
                            app.start_testing();
                        }
                        KeyCode::Char('m') => {
                            app.toggle_mode();
                        }
                        KeyCode::Backspace => {
                            app.remove_last_dns_server();
                        }
                        _ => {
                            app.input.handle_event(&Event::Key(key));
                        }
                    },
                    AppState::Results => match key.code {
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Char('r') => {
                            app.reset();
                        }
                        KeyCode::Char('s') => {
                            app.cycle_sort_column();
                        }
                        KeyCode::Char('d') => {
                            app.toggle_sort_direction();
                        }
                        KeyCode::Char('a') => {
                            app.apply_fastest_dns();
                        }
                        KeyCode::Char('m') => {
                            app.toggle_mode();
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        
        // Always tick when loop cycles
        app.tick();
    }
}
