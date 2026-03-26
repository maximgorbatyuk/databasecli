use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use databasecli_core::health::HealthStatus;

use crate::app::AppState;
use crate::app::Screen;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

const ASCII_LOGO: &str = r#"
 ____    _  _____  _    ____    _    ____  _____ ____ _     ___
|  _ \  / \|_   _|/ \  | __ )  / \  / ___|| ____/ ___| |   |_ _|
| | | |/ _ \ | | / _ \ |  _ \ / _ \ \___ \|  _|| |   | |    | |
| |_| / ___ \| |/ ___ \| |_) / ___ \ ___) | |__| |___| |___ | |
|____/_/   \_\_/_/   \_\____/_/   \_\____/|_____\____|_____|___|
"#;

const GITHUB_URL: &str = "https://github.com/maximgorbatyuk/databasecli";
const AUTHOR: &str = "(c) maximgorbatyuk";

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();
    match app.active_screen {
        Screen::Home => draw_home(frame, app, area),
        Screen::CreateConfig => draw_create_config(frame, app, area),
        Screen::StoredDatabases => draw_stored_databases(frame, app, area),
        Screen::DatabaseHealth => draw_database_health(frame, app, area),
    }
}

fn draw_home(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // ASCII art logo
    for logo_line in ASCII_LOGO.trim_matches('\n').lines() {
        lines.push(Line::from(Span::styled(
            format!("  {logo_line}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));

    // GitHub link and author
    lines.push(Line::from(Span::styled(
        format!("  {GITHUB_URL}"),
        Style::default().fg(Color::Blue),
    )));
    lines.push(Line::from(Span::styled(
        format!("  {AUTHOR}"),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    // Current directory and config path
    lines.push(Line::from(Span::styled(
        format!("  Directory: {}", app.current_dir),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        format!("  Config:    {}", app.config_path),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    // Menu items
    for (i, item) in app.menu_items.iter().enumerate() {
        let is_selected = i == app.selected;

        let prefix = if is_selected { "  ➤ " } else { "    " };
        let name_style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::styled(
                prefix,
                if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(format!("{item}"), name_style),
            Span::styled(
                format!("  {}", item.description()),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    lines.push(Line::from(""));

    if let Some(ref msg) = app.status_message {
        lines.push(Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(Color::Green),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "  ↑/k ↓/j navigate  Enter select  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_create_config(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Create database.ini",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Config file will be created at:".to_string(),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  {}", app.config_path),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  The file will contain a commented template with an example connection.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if let Some(ref err) = app.error_message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {err}"),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter confirm  Esc back  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_stored_databases(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Stored Databases",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if app.databases.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No databases configured.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Create ~/.databasecli/databases.ini to add connections.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for db in &app.databases {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}", db.name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}:{}", db.host, db.port),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("  db={}", db.dbname),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  user={}", db.user),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Esc back  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}

fn draw_database_health(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Database Health",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Checking database connections..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if app.health_results.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No databases configured.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Create ~/.databasecli/databases.ini to add connections.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for r in &app.health_results {
            let (status_text, status_color) = match r.status {
                HealthStatus::Connected => ("Connected", Color::Green),
                HealthStatus::Failed => ("Failed", Color::Red),
            };

            let time_str = match r.response_time {
                Some(d) => format!("{:.0?}", d),
                None => "-".to_string(),
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}", r.name),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}:{}", r.host, r.port),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("  {status_text}"),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  ({time_str})"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            if let Some(ref err) = r.error {
                lines.push(Line::from(Span::styled(
                    format!("    └ {err}"),
                    Style::default().fg(Color::Red),
                )));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Esc back  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
