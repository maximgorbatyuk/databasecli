use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

pub fn draw_create_config(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
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
