use crate::app::{App, AppState, SortColumn};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
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
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Min(5),    // Status
            Constraint::Length(2), // Help
        ])
        .split(frame.area());

    // Title
    let title = Paragraph::new("Testing DNS Servers...")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Progress bar
    let progress = if app.dns_servers.is_empty() {
        0.0
    } else {
        app.testing_index as f64 / app.dns_servers.len() as f64
    };

    let gauge = Gauge::default()
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress * 100.0) as u16)
        .label(format!(
            "{}/{}",
            app.testing_index,
            app.dns_servers.len()
        ));
    frame.render_widget(gauge, chunks[1]);

    // Current test status
    let current_dns = app
        .get_current_test_target()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "Finishing...".to_string());

    let status = Paragraph::new(format!("Testing: {}", current_dns))
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title("Current Test")
                .borders(Borders::ALL),
        );
    frame.render_widget(status, chunks[2]);

    // Help
    let help = Paragraph::new("Please wait... (q: Quit)")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[3]);
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

    // Help
    let help = Paragraph::new("s: Cycle sort column | d: Toggle direction | r: New test | q: Quit")
        .style(Style::default().fg(Color::DarkGray));
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
