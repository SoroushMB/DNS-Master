//! All TUI rendering: Home (catalog selection), Testing (animated, live) and
//! Results (ranked table with service-access badges).

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
};

use crate::app::{App, AppState};
use crate::catalog::Category;
use crate::engine::{SortColumn, TargetRole, TestResult};
use crate::probe::ProbeStatus;

pub fn ui(frame: &mut Frame, app: &App) {
    match app.state {
        AppState::Home => render_home(frame, app),
        AppState::Testing => render_testing(frame, app),
        AppState::Results => render_results(frame, app),
    }
}

// ---------------------------------------------------------------- helpers

fn spinner(tick: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[(tick % FRAMES.len() as u64) as usize]
}

fn pulse_color(tick: u64) -> Color {
    let intensity = (tick % 20) as i16;
    let val = if intensity < 10 {
        intensity
    } else {
        20 - intensity
    };
    Color::Rgb(255, 130 + val as u8 * 5, 0)
}

fn status_style(status: &ProbeStatus) -> Style {
    match status {
        ProbeStatus::Unlocked => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        ProbeStatus::Blocked => Style::default().fg(Color::Red),
        ProbeStatus::Timeout => Style::default().fg(Color::Yellow),
        ProbeStatus::Error(_) => Style::default().fg(Color::DarkGray),
    }
}

fn category_color(cat: &Category) -> Color {
    match cat {
        Category::Antisanction => Color::Magenta,
        Category::Public => Color::Blue,
        Category::Gaming => Color::Yellow,
        Category::Privacy => Color::Green,
        Category::Families => Color::Cyan,
    }
}

fn service_chip_style(status: Option<&ProbeStatus>) -> Style {
    match status {
        Some(st) => status_style(st),
        None => Style::default().fg(Color::DarkGray),
    }
}

fn fmt_ms(d: Option<std::time::Duration>) -> String {
    d.map(|t| format!("{:.0}ms", t.as_secs_f64() * 1000.0))
        .unwrap_or_else(|| "-".into())
}

fn fmt_speed(v: Option<f64>) -> String {
    v.map(|s| format!("{:.1}", s)).unwrap_or_else(|| "-".into())
}

// ------------------------------------------------------------------ Home

fn render_home(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title + stats
            Constraint::Min(6),    // Catalog list
            Constraint::Length(4), // Details of cursor entry
            Constraint::Length(3), // Custom input
            Constraint::Length(1), // Errors
            Constraint::Length(1), // Help
        ])
        .split(frame.area());

    let antisanction_n = app
        .builtin
        .iter()
        .filter(|e| e.category == Category::Antisanction)
        .count();
    let included_n = app
        .builtin
        .iter()
        .filter(|e| app.included.contains(&e.name))
        .count();
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "🌐 DNS-Master v2",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "catalog v{} ({}) · {} entries · {} featured · {} antisanction · {} custom",
                app.catalog_version,
                app.catalog_updated,
                app.builtin.len(),
                included_n,
                antisanction_n,
                app.custom.len()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Catalog list with selection checkboxes.
    let visible = app
        .builtin
        .iter()
        .enumerate()
        .filter(|(_, e)| app.category_filter.is_none_or(|c| e.category == c));
    let max_name: usize = app.builtin.iter().map(|e| e.name.len()).max().unwrap_or(10);

    let items: Vec<ListItem> = visible
        .map(|(i, e)| {
            let checked = if app.included.contains(&e.name) {
                "[✓]"
            } else {
                "[ ]"
            };
            let cursor_arrow = if i == app.cursor { "▶ " } else { "  " };
            let mut line = vec![
                Span::styled(cursor_arrow, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<4}", checked),
                    Style::default().fg(if app.included.contains(&e.name) {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:<w$}", e.name, w = max_name),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {:<16}", e.category.label()),
                    Style::default().fg(category_color(&e.category)),
                ),
                Span::styled(
                    format!(
                        " {:>15} {:>15}",
                        e.ips.first().cloned().unwrap_or_default(),
                        e.ips.get(1).cloned().unwrap_or_default()
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ];
            if e.iran_only() {
                line.push(Span::styled(" ⚠️", Style::default().fg(Color::Yellow)));
            }
            let services = e.known_services();
            if !services.is_empty() {
                line.push(Span::raw("  "));
            }
            for s in services {
                line.push(Span::styled(
                    format!("{}·", s.glyph()),
                    Style::default().fg(Color::Green),
                ));
            }
            if !e.doh.is_empty() {
                line.push(Span::styled(" 🔒", Style::default().fg(Color::DarkGray)));
            }
            ListItem::new(Line::from(line))
        })
        .collect();

    let mut list_state_note = String::new();
    list_state_note.push_str("Select [space] · Enter adds custom IP · Tab to benchmark selected");

    let list_title = if let Some(cat) = app.category_filter {
        format!("Catalog — filter: {}", cat.label())
    } else {
        "Catalog — built-in DNS servers".to_string()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(list, chunks[1]);

    // Details pane for the entry under the cursor.
    let detail_content: Vec<Line> = {
        let entry = app
            .builtin
            .iter()
            .filter(|e| app.category_filter.is_none_or(|c| e.category == c))
            .nth(app.cursor)
            .or_else(|| app.builtin.first());
        match entry {
            Some(e) => {
                let mut lines1: Vec<Line> = Vec::new();
                let mut head = vec![Span::styled(
                    e.name.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )];
                if !e.fa_name.is_empty() {
                    head.push(Span::styled(
                        format!(" {}", e.fa_name),
                        Style::default().fg(Color::Cyan),
                    ));
                }
                head.push(Span::styled(
                    format!(
                        "  ·  primary {}  secondary {}",
                        e.ips.first().cloned().unwrap_or_default(),
                        e.ips.get(1).cloned().unwrap_or_default()
                    ),
                    Style::default().fg(Color::DarkGray),
                ));
                if e.iran_only() {
                    head.push(Span::styled(
                        "  ⚠️ domestic-only (private/carrier range)",
                        Style::default().fg(Color::Yellow),
                    ));
                }
                lines1.push(Line::from(head));

                let mut mid: Vec<Span> = Vec::new();
                if !e.doh.is_empty() {
                    mid.push(Span::styled(
                        "🔒 DoH ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    mid.push(Span::styled(
                        e.doh.clone(),
                        Style::default().fg(Color::DarkGray),
                    ));
                    mid.push(Span::raw("   "));
                }
                let known = e.known_services();
                if !known.is_empty() {
                    mid.push(Span::styled(
                        "known for: ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    for s in known {
                        mid.push(Span::styled(
                            format!("{} ", s.label()),
                            Style::default().fg(Color::Green),
                        ));
                    }
                }
                lines1.push(Line::from(mid));

                if !e.notes.is_empty() {
                    lines1.push(Line::from(Span::styled(
                        e.notes.clone(),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                if !e.links.is_empty() {
                    let mut link_spans: Vec<Span> = Vec::new();
                    link_spans.push(Span::styled("🔗 ", Style::default().fg(Color::DarkGray)));
                    for (i, l) in e.links.iter().enumerate() {
                        if i > 0 {
                            link_spans.push(Span::raw(" · "));
                        }
                        link_spans.push(Span::styled(l.clone(), Style::default().fg(Color::Blue)));
                    }
                    lines1.push(Line::from(link_spans));
                }
                lines1
            }
            None => vec![Line::from(Span::styled(
                "Add a custom IP below, then press Tab to benchmark",
                Style::default().fg(Color::DarkGray),
            ))],
        }
    };
    let details = Paragraph::new(detail_content).block(
        Block::default()
            .title("Entry details")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(details, chunks[2]);

    // Custom input.
    let width = chunks[3].width.saturating_sub(3) as usize;
    let scroll = app.input.visual_scroll(width);
    let input = Paragraph::new(app.input.value())
        .style(Style::default().fg(Color::Yellow))
        .scroll((0, scroll as u16))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("✍️ Add custom DNS IP (Enter to add)")
                .border_style(Style::default().fg(Color::Yellow)),
        );
    frame.render_widget(input, chunks[3]);
    frame.set_cursor_position((
        chunks[3].x + (app.input.visual_cursor().saturating_sub(scroll) as u16) + 1,
        chunks[3].y + 1,
    ));

    let mut err_line = Vec::new();
    let error_text = app.error_message.as_deref().unwrap_or("");
    if !error_text.is_empty() {
        err_line.push(Span::styled(
            format!("❗ {}", error_text),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    } else if let Some(top) = app.custom.last() {
        err_line.push(Span::styled(
            format!("Custom servers: {} (newest: {})", app.custom.len(), top),
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        err_line.push(Span::styled(
            list_state_note,
            Style::default().fg(Color::DarkGray),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(err_line)), chunks[4]);

    let help = Paragraph::new(
        "⌨️  ↑/↓ cursor · Space include/exclude · Enter add IP · Backspace remove custom · Tab start test · q quit",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[5]);
}

// --------------------------------------------------------------- Testing

fn render_testing(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress gauge
            Constraint::Length(4), // Active lanes
            Constraint::Min(6),    // Last & Top result
            Constraint::Length(7), // Mini chart
            Constraint::Length(1), // Help
        ])
        .split(frame.area());

    let total = app.targets.len();
    let done = app.results.len();
    let title = Paragraph::new(format!(
        "{} Testing {} targets with {} lanes — {}/{} done (press r to cancel, q to quit)",
        spinner(app.tick_count),
        total,
        app.lanes,
        done,
        total
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let pct = (done * 100).checked_div(total).unwrap_or(0) as u16;
    let gauge = Gauge::default()
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(pct)
        .label(Span::styled(
            format!("{}/{}", done, total),
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(gauge, chunks[1]);

    // Active lanes.
    let mut lane_lines: Vec<Line> = (0..app.lanes)
        .map(|i| match app.in_flight.get(i) {
            Some(meta) => Line::from(vec![
                Span::styled(
                    format!(" ◈ Lane {}:", i + 1),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    meta.label(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    match meta.role {
                        TargetRole::Primary => "primary",
                        TargetRole::Secondary => "secondary",
                        TargetRole::Custom => "custom",
                    },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(format!(
                    " [{}] ",
                    spinner(app.tick_count.wrapping_add(i as u64))
                )),
            ]),
            None => Line::from(Span::styled(
                format!(" ◈ Lane {}: idling", i + 1),
                Style::default().fg(Color::DarkGray),
            )),
        })
        .collect();
    lane_lines.push(Line::from(Span::styled(
        format!(
            " Probes active: {}",
            app.services.iter().map(|s| s.glyph()).collect::<String>()
        ),
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(
        Paragraph::new(lane_lines)
            .block(Block::default().title("Active lanes").borders(Borders::ALL)),
        chunks[2],
    );

    // Last / best-so-far panes.
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[3]);

    let last_content: Vec<Line> = match &app.last_result {
        Some(res) => summary_lines(res),
        None => vec![Line::from("No tests completed yet.")],
    };
    frame.render_widget(
        Paragraph::new(last_content)
            .block(Block::default().title("Last result").borders(Borders::ALL)),
        status_chunks[0],
    );

    let best_content: Vec<Line> = match best_result(app) {
        Some(res) => {
            let mut lines = summary_lines(res);
            lines.push(Line::from(Span::styled(
                " 🏆 BEST SO FAR 🏆",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines
        }
        None => vec![Line::from("Awaiting results...")],
    };
    frame.render_widget(
        Paragraph::new(best_content).block(
            Block::default()
                .title("Top pick (score)")
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(pulse_color(app.tick_count))
                        .add_modifier(Modifier::BOLD),
                ),
        ),
        status_chunks[1],
    );

    // Mini chart of completed download speeds.
    let labels: Vec<String> = app
        .results
        .iter()
        .map(|r| {
            if r.meta.cand.is_some() {
                format!(
                    "{}·{}",
                    r.meta.cand.clone().unwrap(),
                    if r.meta.role == TargetRole::Secondary {
                        "S"
                    } else {
                        "P"
                    }
                )
            } else {
                r.dns.ip.to_string()
            }
        })
        .collect();
    let chart_data: Vec<(&str, u64)> = app
        .results
        .iter()
        .zip(labels.iter())
        .map(|(r, l)| (l.as_str(), r.dns.download_speed_mbps.unwrap_or(0.0) as u64))
        .collect();

    let chart = BarChart::default()
        .block(Block::default().title("Mbps so far").borders(Borders::ALL))
        .data(&chart_data)
        .bar_width(4)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::Green))
        .value_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(chart, chunks[4]);

    let help = Paragraph::new("⌨️  r cancel · q quit").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[5]);
}

fn best_result(app: &App) -> Option<&TestResult> {
    let max = crate::engine::max_download_speed(&app.results);
    app.results.iter().max_by(|a, b| {
        a.score(max)
            .partial_cmp(&b.score(max))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn summary_lines(res: &TestResult) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(" Target: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                res.meta.label(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(res.dns.ip.to_string(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" Latency: ", Style::default().fg(Color::DarkGray)),
            Span::raw(fmt_ms(res.dns.latency)),
            Span::raw("   "),
            Span::styled("Download: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} Mbps", fmt_speed(res.dns.download_speed_mbps)),
                Style::default().fg(Color::Green),
            ),
        ]),
    ];
    let mut chip_line: Vec<Span> = vec![Span::styled(
        " Access: ",
        Style::default().fg(Color::DarkGray),
    )];
    for (svc, st) in &res.probes {
        chip_line.push(Span::styled(
            format!("{}{} ", svc.glyph(), st.symbol()),
            service_chip_style(Some(st)),
        ));
    }
    chip_line.push(Span::styled(
        "(G Gemini·A Antigrav·Y YouTube·T Telegram·H GitHub·X X·S Spotify)",
        Style::default().fg(Color::DarkGray),
    ));
    lines.push(Line::from(chip_line));

    if let Some(err) = &res.dns.error {
        lines.push(Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
        ]));
    }
    lines
}

// --------------------------------------------------------------- Results

pub fn render_results(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title + best banner
            Constraint::Min(6),    // Table
            Constraint::Length(2), // Status / help
        ])
        .split(frame.area());

    let best_txt: String = if let Some(b) = best_result(app) {
        format!(
            "🏆 Best: {} ({} {}) — {}/{} services reachable",
            b.meta.label(),
            b.dns.ip,
            b.meta.role.as_str(),
            b.unlocked_count(),
            b.probes.len()
        )
    } else {
        "No results available — press r to re-run".to_string()
    };

    let title = Paragraph::new(format!(
        "🏁 Results ({}) — {}",
        sort_label(app.sort_column, app.sort_ascending),
        best_txt
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Header w/ sort arrows.
    let arrow = |col: SortColumn, app: &App| -> &'static str {
        if app.sort_column == col {
            if app.sort_ascending { " ▲" } else { " ▼" }
        } else {
            ""
        }
    };

    let mut header_cells = vec![
        Cell::from(format!("Ping{}", arrow(SortColumn::Latency, app)))
            .style(Style::default().fg(Color::Cyan)),
        Cell::from(format!("Mbps{}", arrow(SortColumn::Speed, app)))
            .style(Style::default().fg(Color::Cyan)),
    ];
    for svc in &app.services {
        header_cells.push(
            Cell::from(svc.glyph().to_string()).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    header_cells.push(
        Cell::from(format!("OK{}", arrow(SortColumn::Unlocked, app)))
            .style(Style::default().fg(Color::Cyan)),
    );
    header_cells.push(
        Cell::from(format!("Score{}", arrow(SortColumn::Score, app))).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    );

    let header = Row::new(header_cells).bottom_margin(0);

    let mut rows: Vec<Row> = Vec::new();
    let max_speed = crate::engine::max_download_speed(&app.results);
    for res in &app.results {
        // Candidate cell.
        let role_tag = match res.meta.role {
            TargetRole::Primary => "P",
            TargetRole::Secondary => "S",
            TargetRole::Custom => "C",
        };
        let cand_cell = Cell::from(Line::from(vec![
            Span::styled(
                res.meta.label(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" [{}]", role_tag),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Service badge cells (one column per tracked service).
        let mut svc_cells: Vec<Cell> = Vec::new();
        for target_svc in &app.services {
            let found = res
                .probes
                .iter()
                .find(|(s, _)| s == target_svc)
                .map(|(_, st)| st.clone());
            let style = service_chip_style(found.as_ref());
            svc_cells.push(
                Cell::from(found.map(|s| s.symbol().to_string()).unwrap_or_default()).style(style),
            );
        }

        let unlocked = res.unlocked_count() as u64;
        let unlocked_style = if !app.services.is_empty() && unlocked == app.services.len() as u64 {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if unlocked > 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Red)
        };

        let score = res.score(max_speed);
        let score_style = if score >= 0.7 {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if score >= 0.4 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut cells = vec![
            cand_cell,
            Cell::from(fmt_ms(res.dns.latency)),
            Cell::from(fmt_speed(res.dns.download_speed_mbps)),
        ];
        cells.extend(svc_cells);
        cells.push(Cell::from(unlocked.to_string()).style(unlocked_style));
        cells.push(Cell::from(format!("{:.3}", score)).style(score_style));
        rows.push(Row::new(cells));
    }

    let mut widths: Vec<Constraint> = vec![
        Constraint::Length(22),
        Constraint::Length(10),
        Constraint::Length(9),
    ];
    for _ in &app.services {
        widths.push(Constraint::Length(2));
    }
    widths.push(Constraint::Length(6));
    widths.push(Constraint::Length(9));

    let visible_height = chunks[1].height.saturating_sub(3) as usize; // borders + header
    let rows: Vec<Row> = rows
        .into_iter()
        .skip(app.table_offset)
        .take(visible_height)
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(format!(
                    "Ranked servers ({} rows — e export)",
                    app.results.len()
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(table, chunks[1]);

    // Footer: status message or help.
    let (footer_text, footer_style) = match &app.status_message {
        Some((msg, is_error)) => (
            format!("{} {}", if *is_error { "❗" } else { "✔" }, msg),
            if *is_error {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            },
        ),
        None => (
            "⌨️  s sort | d direction | ↑/↓ scroll | a apply best to system | e export JSON/CSV | r re-run | h home | q quit".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    };
    frame.render_widget(Paragraph::new(footer_text).style(footer_style), chunks[2]);
}

fn sort_label(col: SortColumn, ascending: bool) -> String {
    let name = match col {
        SortColumn::Score => "composite score",
        SortColumn::Latency => "ping",
        SortColumn::Speed => "speed",
        SortColumn::Unlocked => "services unlocked",
        SortColumn::Candidate => "candidate",
    };
    format!("{}, {}", name, if ascending { "asc" } else { "desc" })
}
