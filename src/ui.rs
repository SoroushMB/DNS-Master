use crate::app::{App, AppState, SortColumn};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
    Frame,
};

pub fn ui(frame: &mut Frame, app: &App) {
    match app.state {
        AppState::Input => render_input_state(frame, app),
        AppState::Testing => render_testing_state(frame, app),
        AppState::Results => render_results_state(frame, app),
    }
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
    let title = Paragraph::new("DNS Speed Tester")
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
                .title("Enter DNS IP (press Enter to add)")
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
    let help = Paragraph::new("Enter: Add DNS | Backspace: Remove last | Tab: Start test | q: Quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
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

    // Title
    let title = Paragraph::new("Testing DNS Servers...")
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
        Span::raw("Testing: "),
        Span::styled(current_dns, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::default().title("Current Server").borders(Borders::ALL));
    
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

    let top_para = Paragraph::new(top_content)
        .block(Block::default().title("Top Result").borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)));
    frame.render_widget(top_para, status_chunks[1]);

    // Comparison Chart
    let chart_data: Vec<(&str, u64)> = app.results.iter()
        .map(|r| {
            let label = Box::leak(r.ip.to_string().into_boxed_str());
            let val = r.download_speed_mbps.unwrap_or(0.0) as u64;
            (label as &str, val)
        })
        .collect();

    let chart = BarChart::default()
        .block(Block::default().title("Download Speed Comparison (Mbps)").borders(Borders::ALL))
        .data(&chart_data)
        .bar_width(12)
        .bar_gap(2)
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
    let title = Paragraph::new("Test Results")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Results table
    let header_cells = [
        create_header_cell("DNS Server", SortColumn::Ip, app),
        create_header_cell("Latency", SortColumn::Latency, app),
        create_header_cell("Download (Mbps)", SortColumn::DownloadSpeed, app),
        Cell::from("Status"),
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
            .map(|e| e.clone())
            .unwrap_or_else(|| "OK".to_string());

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
            .title("Results (sorted by download speed)")
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
