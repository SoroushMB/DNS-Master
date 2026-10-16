mod app;
mod catalog;
mod dns_utils;
mod engine;
mod export;
mod file_loader;
mod probe;
mod sys_dns;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tui_input::backend::crossterm::EventHandler;

use app::{App, AppState};
use catalog::{load_builtin, parse_category};
use engine::Command;
use probe::Service;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "DNS-Master v2 — benchmark DNS servers and empirically check which ones unlock Gemini, YouTube, Telegram & more (from Iran or anywhere).",
    long_about = None
)]
struct Cli {
    /// Initial custom DNS server IPs (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    dns: Vec<IpAddr>,

    /// Path to a JSON file containing DNS IPs [{"ip": "..."}]
    #[arg(long)]
    json: Option<String>,

    /// Path to a CSV file containing DNS IPs (header "ip" required)
    #[arg(long)]
    csv: Option<String>,

    /// Only show this catalog category (antisanction|public|gaming|privacy|families)
    #[arg(long)]
    category: Option<String>,

    /// Comma-separated services to verify: gemini,antigravity,youtube,telegram,github,x,spotify
    /// ("all" by default; "none" disables probing)
    #[arg(long)]
    verify: Option<String>,

    /// Parallel test lanes (default 2 — conservative, best for Iranian ISPs)
    #[arg(long, default_value_t = 2)]
    lanes: usize,

    /// Headless mode: run tests, print a ranked table and exit (no TUI)
    #[arg(long)]
    non_interactive: bool,

    /// Save results to a file (extension decides format: .csv or .json)
    #[arg(long)]
    export: Option<String>,

    /// After testing, apply the best server as system DNS (needs sudo/Admin)
    #[arg(long)]
    apply: bool,

    /// Typos hidden shots helper: prints the checked services and exits
    #[arg(long, hide = true)]
    show_services: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load custom servers from CLI and files.
    let mut custom: Vec<IpAddr> = cli.dns.clone();
    if let Some(json_path) = &cli.json {
        let ips = file_loader::load_json(json_path)
            .with_context(|| format!("Failed to load DNS from JSON: {json_path}"))?;
        custom.extend(ips);
    }
    if let Some(csv_path) = &cli.csv {
        let ips = file_loader::load_csv(csv_path)
            .with_context(|| format!("Failed to load DNS from CSV: {csv_path}"))?;
        custom.extend(ips);
    }
    sort_dedup(&mut custom);

    // Built-in catalog.
    let catalog = load_builtin().context("Loading built-in DNS catalog")?;
    let category_filter = match &cli.category {
        Some(c) => Some(parse_category(c).context("Invalid --category")?),
        None => None,
    };

    // Services to probe.
    if cli.show_services {
        println!("Probeable services:");
        for s in Service::ALL {
            println!("  {:<14} {}", s.label(), s.host());
        }
        return Ok(());
    }
    let services = resolve_services(cli.verify.as_deref());

    // Headless path: run everything, print, exit.
    if cli.non_interactive {
        return headless_run(&cli, custom, &catalog.entries, category_filter, services);
    }

    // Interactive TUI path.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        let mut app = App::new(
            custom,
            catalog.entries.clone(),
            catalog.version,
            catalog.updated.clone(),
            category_filter,
            cli.lanes,
            services.clone(),
        );
        if let Some(path) = cli.export {
            app.set_export_path(path);
        }
        run_tui(app).await
    })?;

    Ok(())
}

fn sort_dedup(v: &mut Vec<IpAddr>) {
    v.sort();
    v.dedup();
}

/// Parse --verify: default all, "none" disables.
fn resolve_services(verify: Option<&str>) -> Vec<Service> {
    let Some(spec) = verify else {
        return Service::ALL.to_vec();
    };
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("none") || spec.is_empty() {
        return Vec::new();
    }
    if spec.eq_ignore_ascii_case("all") {
        return Service::ALL.to_vec();
    }
    let mut picked = Vec::new();
    for part in spec.split(',') {
        match part.trim().to_lowercase().as_str() {
            "gemini" => picked.push(Service::Gemini),
            "antigravity" => picked.push(Service::Antigravity),
            "youtube" | "yt" => picked.push(Service::YouTube),
            "telegram" | "tg" => picked.push(Service::Telegram),
            "github" | "gh" => picked.push(Service::GitHub),
            "x" | "twitter" | "x.com" => picked.push(Service::X),
            "spotify" => picked.push(Service::Spotify),
            "" => {}
            other => eprintln!(
                "Warning: unknown service '{other}' (options: gemini, antigravity, youtube, telegram, github, x, spotify, all, none)"
            ),
        }
    }
    picked
}

// -------------------------------------------------------------- headless

fn headless_run(
    cli: &Cli,
    custom: Vec<IpAddr>,
    builtin: &[catalog::CatalogEntry],
    category_filter: Option<catalog::Category>,
    services: Vec<Service>,
) -> Result<()> {
    let included = engine::default_included(builtin, category_filter);
    let targets = engine::build_targets(&custom, builtin, &included);
    if targets.is_empty() {
        anyhow::bail!("No targets selected: add IPs via -d/--json/--csv or adjust --category");
    }

    println!(
        "🚀 Testing {} targets at {} lanes ({} service probes)...",
        targets.len(),
        cli.lanes,
        services.len()
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let results: Vec<engine::TestResult> = rt.block_on(async {
        engine::collect_batch(targets, cli.lanes, Arc::new(services.clone())).await
    });

    let max = engine::max_download_speed(&results);
    let mut out = results;
    engine::sort_results(&mut out, engine::SortColumn::Score, false, max);

    // Ranked table.
    println!();
    println!(
        "{:<4} {:<24} {:<18} {:>9} {:>10} {:<24} {:>7}",
        "#", "Candidate", "IP", "Ping", "Mbps", "Access", "Score"
    );
    for (i, r) in out.iter().enumerate() {
        let access: String = r
            .probes
            .iter()
            .map(|(s, st)| format!("{}{}", s.glyph(), st.symbol()))
            .collect();
        println!(
            "{:<4} {:<24} {:<18} {:>9} {:>10} {:<24} {:>7.3}",
            i + 1,
            truncate(&r.meta.label(), 24),
            r.dns.ip.to_string(),
            fmt_ms(&r.dns.latency),
            fmt_speed(&r.dns.download_speed_mbps),
            access,
            r.score(max)
        );
    }
    println!();
    println!(
        "Legend: ✓ unlocked · ✗ blocked · ⌛ timeout · ? error  —  G Gemini A Antigravity Y YouTube T Telegram H GitHub X X S Spotify"
    );
    if let Some(best) = out.first() {
        println!(
            "🏆 Best overall: {} ({}) — score {:.3}",
            best.meta.label(),
            best.dns.ip,
            best.score(max)
        );
    }

    if let Some(path) = &cli.export {
        let is_csv = path.ends_with(".csv");
        let n = export::write_results(&out, path, is_csv)?;
        println!("📄 Exported {} results to {}", n, path);
    }

    if cli.apply
        && let Some(best) = out.first()
    {
        if best.dns.is_ok() {
            match sys_dns::set_system_dns(best.dns.ip) {
                Ok(()) => println!("✅ System DNS set to {}", best.dns.ip),
                Err(e) => {
                    eprintln!("❌ Failed to set system DNS: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("❌ Best result was not a working server; not applying.");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{}…", cut)
    }
}

fn fmt_ms(d: &Option<Duration>) -> String {
    d.map(|t| format!("{:.0}ms", t.as_secs_f64() * 1000.0))
        .unwrap_or_else(|| "-".into())
}

fn fmt_speed(v: &Option<f64>) -> String {
    v.map(|s| format!("{:.1}", s)).unwrap_or_else(|| "-".into())
}

// ------------------------------------------------------------------ TUI

async fn run_tui(mut app: App) -> Result<()> {
    // Start the engine once, wired to the app's epoch-based commands.
    let services = Arc::new(app.services.clone());
    let (tx, rx) = engine::spawn_engine(app.lanes, services);
    app.tx = Some(tx);
    app.rx = Some(rx);

    // Setup terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app).await;

    // Tear down: stop engine, restore terminal.
    if let Some(tx) = &app.tx {
        let _ = tx.send(Command::Stop).await;
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }
    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(50);

    loop {
        terminal.draw(|f| ui::ui(f, app))?;

        if app.state == AppState::Testing {
            // During testing: pump results at high frequency, only handle r/q.
            if event::poll(Duration::from_millis(10))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('r') => app.reset_to_home(),
                    _ => {}
                }
            }
            if app.should_quit {
                return Ok(());
            }
            app.pump_results();
            app.tick();
            continue;
        }

        if event::poll(tick_rate)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key(app, key);
            if app.should_quit {
                return Ok(());
            }
        }
        app.pump_results();
        app.tick();
    }
}

fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match app.state {
        AppState::Home => match key.code {
            KeyCode::Up => app.move_cursor_up(),
            KeyCode::Down => app.move_cursor_down(),
            KeyCode::Enter => app.add_custom_from_input(),
            KeyCode::Backspace => {
                if app.input.value().is_empty() {
                    app.remove_last_custom();
                } else {
                    app.input.handle_event(&Event::Key(key));
                }
            }
            KeyCode::Tab => app.start_testing(),
            KeyCode::Char(' ') => app.toggle_include(),
            KeyCode::Char('q') => {
                if key.modifiers.is_empty() {
                    app.should_quit = true;
                } else {
                    app.input.handle_event(&Event::Key(key));
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.should_quit = true;
            }
            _ => {
                app.input.handle_event(&Event::Key(key));
            }
        },
        AppState::Results => match key.code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('r') => app.rerun(),
            KeyCode::Char('s') => app.cycle_sort_column(),
            KeyCode::Char('d') => app.toggle_sort_direction(),
            KeyCode::Char('a') => app.apply_best(),
            KeyCode::Char('e') => app.export_results(),
            KeyCode::Char('h') => app.reset_to_home(),
            KeyCode::Up | KeyCode::Left => app.scroll_table(-1),
            KeyCode::Down | KeyCode::Right => app.scroll_table(1),
            KeyCode::PageUp => app.scroll_table(-10),
            KeyCode::PageDown => app.scroll_table(10),
            _ => {}
        },
        _ => {}
    }
}
