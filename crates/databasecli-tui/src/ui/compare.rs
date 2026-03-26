use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_compare(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Compare",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  Run same query across all connected databases",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    let cursor = if app.input_mode { "_" } else { "" };
    lines.push(Line::from(vec![
        Span::styled("  SQL> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}{}", app.input_buffer, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Running on all databases..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if let Some(ref result) = app.compare_result {
        for qr in &result.results {
            lines.push(Line::from(Span::styled(
                format!(
                    "  --- {} --- {} row(s) ({:.0?})",
                    qr.database_name, qr.row_count, qr.execution_time
                ),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));

            if !qr.columns.is_empty() {
                let header: String = qr.columns.join("  ");
                lines.push(Line::from(Span::styled(
                    format!("  {header}"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )));

                for row in &qr.rows {
                    let row_str: String = row.join("  ");
                    lines.push(Line::from(Span::styled(
                        format!("  {row_str}"),
                        Style::default().fg(Color::White),
                    )));
                }
            }
            lines.push(Line::from(""));
        }

        for (name, err) in &result.errors {
            lines.push(Line::from(Span::styled(
                format!("  --- {name} ---"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  Error: {err}"),
                Style::default().fg(Color::Red),
            )));
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  i/Enter type  Enter run  Esc stop typing  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
