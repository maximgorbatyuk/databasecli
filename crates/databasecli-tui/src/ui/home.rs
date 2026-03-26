use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

const ASCII_LOGO: &str = r#"
 ____    _  _____  _    ____    _    ____  _____ ____ _     ___
|  _ \  / \|_   _|/ \  | __ )  / \  / ___|| ____/ ___| |   |_ _|
| | | |/ _ \ | | / _ \ |  _ \ / _ \ \___ \|  _|| |   | |    | |
| |_| / ___ \| |/ ___ \| |_) / ___ \ ___) | |__| |___| |___ | |
|____/_/   \_\_/_/   \_\____/_/   \_\____/|_____\____|_____|___|
"#;

const GITHUB_URL: &str = "https://github.com/maximgorbatyuk/databasecli";
const AUTHOR: &str = "(c) maximgorbatyuk";

pub fn draw_home(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for logo_line in ASCII_LOGO.trim_matches('\n').lines() {
        lines.push(Line::from(Span::styled(
            format!("  {logo_line}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        format!("  {GITHUB_URL}"),
        Style::default().fg(Color::Blue),
    )));
    lines.push(Line::from(Span::styled(
        format!("  {AUTHOR}"),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    if app.connected_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("  Connected: {} database(s)", app.connected_count),
            Style::default().fg(Color::Green),
        )));
    }

    lines.push(Line::from(Span::styled(
        format!("  Directory: {}", app.current_dir),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        format!("  Config:    {}", app.config_path),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

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
