mod app;
mod dns_utils;
mod ui;
mod file_loader;

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

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let app = App::new(initial_dns);
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
    loop {
        // Draw UI
        terminal.draw(|f| ui::ui(f, &app))?;

        // Handle testing state specially - we need to run tests
        if app.state == AppState::Testing {
            // Poll for events with a short timeout
            if event::poll(Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                        app.should_quit = true;
                    }
                }
            }

            if app.should_quit {
                return Ok(());
            }

            // Run the next test
            if !app.run_test_for_current().await {
                // No more tests, results are ready
            }
            continue;
        }

        // Wait for events
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
                    KeyCode::Backspace => {
                        if app.input.value().is_empty() {
                            app.remove_last_dns_server();
                        } else {
                            app.input.handle_event(&Event::Key(key));
                        }
                    }
                    _ => {
                        app.input.handle_event(&Event::Key(key));
                    }
                },
                AppState::Testing => {
                    // Handled above
                }
                AppState::Results => match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Char('s') => {
                        app.cycle_sort_column();
                    }
                    KeyCode::Char('d') => {
                        app.toggle_sort_direction();
                    }
                    KeyCode::Char('r') => {
                        app.reset();
                    }
                    _ => {}
                },
            }
        }
    }
}
