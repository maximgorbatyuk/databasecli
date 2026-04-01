use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use databasecli_core::health::HealthStatus;

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_database_health(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
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
            "  Create .databasecli/databases.ini to add connections.",
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
