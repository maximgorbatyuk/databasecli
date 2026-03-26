use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

use super::SPINNER_FRAMES;

pub fn draw_trend(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Trend",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  Format: table timestamp_col [interval] [value_col]",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  Example: orders created_at day amount",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    let cursor = if app.input_mode { "_" } else { "" };
    lines.push(Line::from(vec![
        Span::styled("  Trend> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}{}", app.input_buffer, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));

    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Computing trend..."),
            Style::default().fg(Color::Yellow),
        )));
    } else if let Some(ref err) = app.error_message {
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    } else if let Some(ref result) = app.trend_result {
        lines.push(Line::from(Span::styled(
            format!(
                "  [{}] {} — by {}",
                result.database_name, result.table, result.interval
            ),
            Style::default().fg(Color::Green),
        )));
        lines.push(Line::from(""));

        if result.rows.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No data.",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let has_avg = result.rows.iter().any(|r| r.avg_value.is_some());
            let max_count = result
                .rows
                .iter()
                .map(|r| r.count)
                .max()
                .unwrap_or(1)
                .max(1);

            for row in &result.rows {
                let bar_len = ((row.count as f64 / max_count as f64) * 20.0) as usize;
                let bar = "█".repeat(bar_len);

                let mut text = format!("  {:<26} {:>8}", row.period, row.count);
                if has_avg {
                    let avg = row.avg_value.as_deref().unwrap_or("-");
                    text.push_str(&format!("  {:>12}", avg));
                }
                text.push_str(&format!("  {bar}"));

                lines.push(Line::from(Span::styled(
                    text,
                    Style::default().fg(Color::White),
                )));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  i/Enter type  Enter compute  Esc stop typing  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
