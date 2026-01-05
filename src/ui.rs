use crate::app::{App, AppMode, AppState, SortColumn};
use crate::dns_utils::DnsTestResult;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
    Frame,
};

pub fn ui(frame: &mut Frame, app: &App) {
    match app.mode {
        AppMode::Dns => {
            match app.state {
                AppState::Input => render_input_state(frame, app),
                AppState::Testing => render_testing_state(frame, app),
                AppState::Results => render_results_state(frame, app),
            }
        }
        AppMode::Mirror => {
            match app.state {
                AppState::Input => render_mirror_input_state(frame, app),
                AppState::Testing => render_testing_state(frame, app), // Sharing testing UI for now
                AppState::Results => render_mirror_results_state(frame, app),
            }
        }
    }
}

fn render_mirror_input_state(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(5),    // Mirror List
            Constraint::Length(3), // Info
            Constraint::Length(2), // Help
        ])
        .split(frame.area());

    let title = Paragraph::new(format!("🪞 Mirror Master - Distro: {} {}", app.detected_distro.emoji(), app.detected_distro.as_str()))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let mirror_items: Vec<ListItem> = app.mirrors.iter()
        .map(|m| {
            let content = Line::from(vec![
                Span::styled(format!("{} {:<20}", m.distro.emoji(), m.name), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(&m.url, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let mirrors_list = List::new(mirror_items)
        .block(Block::default().title("Mirrors to Test").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(mirrors_list, chunks[1]);

    let info = Paragraph::new(format!("📦 Loaded {} mirrors for {}. Use CSV to add more. ✨", app.mirrors.len(), app.detected_distro.as_str()))
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(info, chunks[2]);

    let help = Paragraph::new("⌨️ Tab: Start Testing | 🖱️ m: Switch to DNS Mode | 🛑 q: Quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
}

fn render_mirror_results_state(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(5),    // Table
            Constraint::Length(2), // Help
        ])
        .split(frame.area());

    let title = Paragraph::new("🏁 Mirror Test Results 🏁")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let header = Row::new(vec![
        Cell::from("📋 Mirror Name").style(Style::default().fg(Color::Cyan)),
        Cell::from("⚡ Speed (Mbps)").style(Style::default().fg(Color::Cyan)),
        Cell::from("🏷️ Status").style(Style::default().fg(Color::Cyan)),
    ]).height(1);

    let rows = app.mirror_results.iter().map(|result| {
        let speed_str = result.speed_mbps
            .map(|s| format!("{:.2}", s))
            .unwrap_or_else(|| "-".to_string());
        
        let (status, status_style) = if let Some(err) = &result.error {
            (format!("❌ {}", err), Style::default().fg(Color::Red))
        } else {
            ("✅ OK".to_string(), Style::default().fg(Color::Green))
        };

        Row::new(vec![
            Cell::from(result.name.clone()),
            Cell::from(speed_str),
            Cell::from(status).style(status_style),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ],
    )
    .header(header)
    .block(Block::default().title("Results").borders(Borders::ALL))
    .style(Style::default().fg(Color::White));

    frame.render_widget(table, chunks[1]);

    let help = Paragraph::new("⌨️ r: New test | 🖱️ m: Switch Mode | 🛑 q: Quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[2]);
}

fn render_input_state(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(5),    // DNS list
            Constraint::Length(3), // Input
            Constraint::Length(2), // Error message
            Constraint::Length(2), // Help
        ])
        .split(frame.area());

    // Title
    let title = Paragraph::new("🌐 DNS Speed Tester 🌐")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // DNS server list
    let items: Vec<ListItem> = app
        .dns_servers
        .iter()
        .enumerate()
        .map(|(i, ip)| {
            ListItem::new(format!("{}. {}", i + 1, ip))
                .style(Style::default().fg(Color::White))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("DNS Servers")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(list, chunks[1]);

    // Input field
    let width = chunks[2].width.saturating_sub(3) as usize;
    let scroll = app.input.visual_scroll(width);
    let input = Paragraph::new(app.input.value())
        .style(Style::default().fg(Color::Yellow))
        .scroll((0, scroll as u16))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("✍️ Enter DNS IP (press Enter to add)")
                .border_style(Style::default().fg(Color::Yellow)),
        );
    frame.render_widget(input, chunks[2]);

    // Set cursor position
    frame.set_cursor_position((
        chunks[2].x + (app.input.visual_cursor().saturating_sub(scroll) as u16) + 1,
        chunks[2].y + 1,
    ));

    // Error message
    let error_text = app.error_message.as_deref().unwrap_or("");
    let error = Paragraph::new(error_text).style(Style::default().fg(Color::Red));
    frame.render_widget(error, chunks[3]);

    // Help text
    let help = Paragraph::new("⌨️ Enter: Add DNS | 🖱️ Backspace: Remove | 📑 Tab: Start test | 🛑 q: Quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn get_spinner(tick: u64) -> &'static str {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    frames[(tick % frames.len() as u64) as usize]
}

fn get_pulse_color(tick: u64) -> Color {
    // Pulse between yellow and orange for a "gold" effect
    let intensity = (tick % 20) as i16;
    let val = if intensity < 10 { intensity } else { 20 - intensity };
    let r = 255;
    let g = 150 + (val * 10) as u8;
    let b = 0;
    Color::Rgb(r, g, b)
}

fn render_testing_state(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Length(9), // Status (Last & Top)
            Constraint::Min(10),   // Comparison Graph
            Constraint::Length(1), // Help
        ])
        .split(frame.area());

    // Title with spinner
    let spinner = get_spinner(app.tick_count);
    let title = Paragraph::new(format!("{} ⏳ Testing Servers... {}", spinner, spinner))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Progress bar
    let progress_ratio = if app.dns_servers.is_empty() {
        0.0
    } else {
        app.testing_index as f64 / app.dns_servers.len() as f64
    };

    let label_color = if progress_ratio >= 0.5 {
        Color::Black
    } else {
        Color::White
    };

    let gauge = Gauge::default()
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress_ratio * 100.0) as u16)
        .label(
            Span::styled(
                format!("{}/{}", app.testing_index, app.dns_servers.len()),
                Style::default().fg(label_color).add_modifier(Modifier::BOLD),
            )
        );
    frame.render_widget(gauge, chunks[1]);

    // Current test status and Result summaries
    let current_dns = app
        .get_current_test_target()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "Finishing...".to_string());

    let testing_block = Paragraph::new(Line::from(vec![
        Span::raw("🔍 Testing: "),
        Span::styled(current_dns, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::default().title("🔭 Current Server").borders(Borders::ALL));
    
    // Split the status area for Last Result and Top Result
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[2]);

    // Sub-layout for Current Server + Last Result in left pane
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(status_chunks[0]);

    frame.render_widget(testing_block, left_chunks[0]);

    // Last Result
    let last_content = if let Some(last) = &app.last_result {
        let latency_str = last.latency
            .map(|d| format!("{:.2}ms", d.as_secs_f64() * 1000.0))
            .unwrap_or_else(|| "-".to_string());
        
        let speed_str = last.download_speed_mbps
            .map(|s| format!("{:.2} Mbps", s))
            .unwrap_or_else(|| "-".to_string());

        let mut lines = vec![
            Line::from(vec![
                Span::styled("IP: ", Style::default().fg(Color::DarkGray)),
                Span::styled(last.ip.to_string(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Latency: ", Style::default().fg(Color::DarkGray)),
                Span::raw(latency_str),
            ]),
            Line::from(vec![
                Span::styled("Download: ", Style::default().fg(Color::DarkGray)),
                Span::raw(speed_str),
            ]),
        ];

        if let Some(err) = &last.error {
            lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(err, Style::default().fg(Color::Red)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled("OK", Style::default().fg(Color::Green)),
            ]));
        }
        lines
    } else {
        vec![Line::from("No tests completed yet.")]
    };

    let last_para = Paragraph::new(last_content)
        .block(Block::default().title("Last Result").borders(Borders::ALL));
    frame.render_widget(last_para, left_chunks[1]);

    // Top Result
    let top_content = if let Some(top) = &app.best_result {
        let latency_str = top.latency
            .map(|d| format!("{:.2}ms", d.as_secs_f64() * 1000.0))
            .unwrap_or_else(|| "-".to_string());
        
        let speed_str = top.download_speed_mbps
            .map(|s| format!("{:.2} Mbps", s))
            .unwrap_or_else(|| "-".to_string());

        vec![
            Line::from(vec![
                Span::styled("IP: ", Style::default().fg(Color::DarkGray)),
                Span::styled(top.ip.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Latency: ", Style::default().fg(Color::DarkGray)),
                Span::raw(latency_str),
            ]),
            Line::from(vec![
                Span::styled("Download: ", Style::default().fg(Color::DarkGray)),
                Span::styled(speed_str, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(Span::styled("🏆 FASTEST 🏆", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        ]
    } else {
        vec![Line::from("Awaiting best result...")]
    };

    // Top Result with pulsing border
    let pulse = get_pulse_color(app.tick_count);
    let top_para = Paragraph::new(top_content)
        .block(Block::default()
            .title("🏆 Top Result 🏆")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(pulse).add_modifier(Modifier::BOLD)));
    frame.render_widget(top_para, status_chunks[1]);

    // Comparison Chart with dynamic scaling and no memory leak
    let labels: Vec<String>;
    let chart_data: Vec<(&str, u64)>;
    
    match app.mode {
        AppMode::Dns => {
            labels = app.results.iter().map(|r| r.ip.to_string()).collect();
            chart_data = app.results.iter().enumerate()
                .map(|(i, r)| (labels[i].as_str(), r.download_speed_mbps.unwrap_or(0.0) as u64))
                .collect();
        }
        AppMode::Mirror => {
            labels = app.mirror_results.iter().map(|r| r.name.clone()).collect();
            chart_data = app.mirror_results.iter().enumerate()
                .map(|(i, r)| (labels[i].as_str(), r.speed_mbps.unwrap_or(0.0) as u64))
                .collect();
        }
    };

    let num_bars = chart_data.len() as u16;
    let available_width = chunks[3].width.saturating_sub(4); // Borders
    
    // Calculate bar width and gap dynamically.
    let (bar_width, bar_gap) = if num_bars > 20 {
        (2, 0)
    } else if num_bars > 10 {
        (4, 1)
    } else {
        (12, 2)
    };

    // If we have way too many bars to fit, limit the display
    let display_data = if num_bars * (bar_width + bar_gap) > available_width && num_bars > 10 {
        let max_visible = (available_width / (bar_width + bar_gap + 1)) as usize;
        let half = max_visible / 2;
        let mut combined = chart_data[..half.min(chart_data.len())].to_vec();
        let end_start = chart_data.len().saturating_sub(max_visible - half);
        combined.extend(chart_data[end_start..].to_vec());
        combined
    } else {
        chart_data
    };

    let chart = BarChart::default()
        .block(Block::default().title("📊 Download Speed Comparison (Mbps)").borders(Borders::ALL))
        .data(&display_data)
        .bar_width(bar_width)
        .bar_gap(bar_gap)
        .bar_style(Style::default().fg(Color::Green))
        .value_style(Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD));

    frame.render_widget(chart, chunks[3]);

    // Help
    let help = Paragraph::new("Please wait... (q: Quit)")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn render_results_state(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(5),    // Results table
            Constraint::Length(2), // Help
        ])
        .split(frame.area());

    // Title
    let title = Paragraph::new("🏁 Test Results 🏁")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Results table
    let header_cells = [
        create_header_cell("🖥️ DNS Server", SortColumn::Ip, app),
        create_header_cell("⏱️ Latency", SortColumn::Latency, app),
        create_header_cell("🚀 Download (Mbps)", SortColumn::DownloadSpeed, app),
        Cell::from("📋 Status"),
    ];
    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::Cyan))
        .height(1);

    let rows = app.results.iter().map(|result| {
        let latency_str = result
            .latency
            .map(|d| format!("{:.2}ms", d.as_secs_f64() * 1000.0))
            .unwrap_or_else(|| "-".to_string());

        let speed_str = result
            .download_speed_mbps
            .map(|s| format!("{:.2}", s))
            .unwrap_or_else(|| "-".to_string());

        let status = result
            .error
            .as_ref()
            .map(|e| format!("❌ {}", e))
            .unwrap_or_else(|| "✅ OK".to_string());

        let status_style = if result.error.is_some() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };

        Row::new(vec![
            Cell::from(result.ip.to_string()),
            Cell::from(latency_str),
            Cell::from(speed_str),
            Cell::from(status).style(status_style),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(15),
            Constraint::Length(18),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("📊 Results (sorted by download speed)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    )
    .style(Style::default().fg(Color::White));

    frame.render_widget(table, chunks[1]);

    // Help & Status
    let help_text = if let Some((msg, is_error)) = &app.status_message {
        let prefix = if *is_error { "Error: " } else { "Success: " };
        format!("{}{}", prefix, msg)
    } else {
        "s: Sort | d: Dir | r: New test | a: Apply Fastest | q: Quit".to_string()
    };
    
    let help_style = if let Some((_, is_error)) = &app.status_message {
        if *is_error {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let help = Paragraph::new(help_text)
        .style(help_style);
    frame.render_widget(help, chunks[2]);
}

fn create_header_cell<'a>(text: &'a str, column: SortColumn, app: &App) -> Cell<'a> {
    let is_current = app.sort_column == column;
    let arrow = if is_current {
        if app.sort_ascending { " ▲" } else { " ▼" }
    } else {
        ""
    };

    let style = if is_current {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    Cell::from(format!("{}{}", text, arrow)).style(style)
}
