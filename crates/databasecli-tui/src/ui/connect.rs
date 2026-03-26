use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_connect(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Connect to Databases",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Connecting..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
    }

    if app.databases.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No databases configured.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, db) in app.databases.iter().enumerate() {
            let is_cursor = i == app.connect_cursor;
            let is_selected = app.connect_selection.get(i).copied().unwrap_or(false);
            let is_connected = app.connected_names.contains(&db.name);

            let checkbox = if is_selected { "[x]" } else { "[ ]" };
            let prefix = if is_cursor { "  ➤ " } else { "    " };

            let name_style = if is_cursor {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_connected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let status = if is_connected { "  Connected" } else { "" };

            lines.push(Line::from(vec![
                Span::styled(prefix, name_style),
                Span::styled(format!("{checkbox} "), name_style),
                Span::styled(db.name.to_string(), name_style),
                Span::styled(
                    format!("  {}:{}", db.host, db.port),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  db={}", db.dbname),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(status, Style::default().fg(Color::Green)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Space toggle  Enter connect  Esc back  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
