use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_schema(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Schema",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Loading schema..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if let Some(ref results) = app.schema_results {
        for result in results {
            lines.push(Line::from(Span::styled(
                format!("  === {} ===", result.database_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            if result.tables.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No tables found.",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for table in &result.tables {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {}.{}", table.schema, table.name),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {} rows", table.row_count),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("  {}", table.total_size),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));

                    let pk_set: std::collections::HashSet<&str> = table
                        .primary_key_columns
                        .iter()
                        .map(|s| s.as_str())
                        .collect();

                    for col in &table.columns {
                        let pk = if pk_set.contains(col.name.as_str()) {
                            " PK"
                        } else {
                            ""
                        };
                        let nullable = if col.is_nullable { "NULL" } else { "NOT NULL" };
                        lines.push(Line::from(Span::styled(
                            format!("    {} {} {}{}", col.name, col.data_type, nullable, pk),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                    lines.push(Line::from(""));
                }
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  No connections. Use Connect first.",
            Style::default().fg(Color::DarkGray),
        )));
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
