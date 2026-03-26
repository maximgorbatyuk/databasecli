use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_summary(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Summary",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Loading summary..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if let Some(ref summaries) = app.summary_results {
        for summary in summaries {
            lines.push(Line::from(Span::styled(
                format!("  === {} ===", summary.database_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            lines.push(Line::from(Span::styled(
                format!("  Database size:  {}", summary.total_size),
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(Span::styled(
                format!("  Tables:         {}", summary.table_count),
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(Span::styled(
                format!("  Total rows:     {}", summary.total_rows),
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(Span::styled(
                format!("  Indexes:        {}", summary.index_count),
                Style::default().fg(Color::White),
            )));

            if !summary.largest_tables.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  Largest tables:",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )));

                for t in &summary.largest_tables {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("    {}", t.table_name),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            format!("  {} rows", t.row_count),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("  {}", t.total_size),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }
            lines.push(Line::from(""));
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
