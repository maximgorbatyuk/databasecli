use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_sample(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Sample",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let cursor = if app.input_mode { "_" } else { "" };
    lines.push(Line::from(vec![
        Span::styled("  Table> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}{}", app.input_buffer, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Fetching rows..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if let Some(ref result) = app.sample_result {
        lines.push(Line::from(Span::styled(
            format!(
                "  [{}] {} — {} of ~{} rows",
                result.database_name,
                result.table,
                result.rows_returned,
                result.total_rows_in_table
            ),
            Style::default().fg(Color::Green),
        )));
        lines.push(Line::from(""));

        if !result.columns.is_empty() {
            let header: String = result.columns.join("  ");
            lines.push(Line::from(Span::styled(
                format!("  {header}"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {}", "-".repeat(header.len())),
                Style::default().fg(Color::DarkGray),
            )));

            for row in &result.rows {
                let row_str: String = row.join("  ");
                lines.push(Line::from(Span::styled(
                    format!("  {row_str}"),
                    Style::default().fg(Color::White),
                )));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  i/Enter type  Enter fetch  Esc stop typing  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
