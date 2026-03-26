use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_analyze(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Analyze",
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
            format!("  {frame_char} Analyzing table..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if let Some(ref profile) = app.analyze_result {
        lines.push(Line::from(Span::styled(
            format!(
                "  [{}] {}.{} — {} rows",
                profile.database_name, profile.schema, profile.table, profile.total_rows
            ),
            Style::default().fg(Color::Green),
        )));
        lines.push(Line::from(""));

        for col in &profile.columns {
            lines.push(Line::from(Span::styled(
                format!("  {} ({})", col.name, col.data_type),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!(
                    "    Nulls: {}/{} ({:.1}%)  Distinct: {}",
                    col.null_count, col.total_rows, col.null_pct, col.distinct_count
                ),
                Style::default().fg(Color::DarkGray),
            )));

            if let Some(ref min) = col.min_value {
                let mut stats = format!("    Min: {min}");
                if let Some(ref max) = col.max_value {
                    stats.push_str(&format!("  Max: {max}"));
                }
                if let Some(ref avg) = col.avg_value {
                    stats.push_str(&format!("  Avg: {avg}"));
                }
                lines.push(Line::from(Span::styled(
                    stats,
                    Style::default().fg(Color::DarkGray),
                )));
            }

            if !col.top_values.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    Top values:",
                    Style::default().fg(Color::DarkGray),
                )));
                for (val, freq) in col.top_values.iter().take(5) {
                    let truncated = if val.len() > 30 {
                        format!("{}...", &val[..27])
                    } else {
                        val.clone()
                    };
                    lines.push(Line::from(Span::styled(
                        format!("      {truncated:<30} {freq}"),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  i/Enter type  Enter analyze  Esc stop typing  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
