use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{AppState, ExecutePhase};
use databasecli_core::commands::execute::format_execute_result;

use super::SPINNER_FRAMES;

pub fn draw_execute(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Execute (write/DDL — local only, never reachable from MCP)",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if let Some(ref db) = app.execute_database {
        lines.push(Line::from(vec![
            Span::styled("  Target: ", Style::default().fg(Color::DarkGray)),
            Span::styled(db.clone(), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
    }

    match app.execute_phase {
        ExecutePhase::PickDatabase => draw_picker(&mut lines, app),
        ExecutePhase::EditSql => draw_editor(&mut lines, app),
        ExecutePhase::Confirm => draw_confirm(&mut lines, app),
        ExecutePhase::Result => draw_result(&mut lines, app),
    }

    if let Some(ref err) = app.error_message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Error: {err}"),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        footer_for(app.execute_phase, app.execute_input_mode),
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}

fn draw_picker(lines: &mut Vec<Line>, app: &AppState) {
    lines.push(Line::from(Span::styled(
        "  Select target database:",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if app.connected_names.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No connected databases. Connect first.",
            Style::default().fg(Color::Red),
        )));
        return;
    }

    for (i, name) in app.connected_names.iter().enumerate() {
        let marker = if i == app.execute_db_cursor { ">" } else { " " };
        let style = if i == app.execute_db_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format!("  {marker} {name}"),
            style,
        )));
    }
}

fn draw_editor(lines: &mut Vec<Line>, app: &AppState) {
    // Mode pill: gives an at-a-glance signal of where the next keystroke
    // lands. Without it, a user re-entering input mode on a populated
    // buffer has no obvious indication that typing now appends to the
    // buffer instead of scrolling.
    let (mode_label, mode_color) = if app.execute_input_mode {
        ("● TYPING", Color::Green)
    } else {
        ("○ scroll", Color::DarkGray)
    };
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            mode_label,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  SQL (Enter inserts newline, F5 or Ctrl+R run):",
            Style::default().fg(Color::Yellow),
        ),
    ]));
    lines.push(Line::from(""));

    if app.execute_sql_buffer.is_empty() {
        if app.execute_input_mode {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "▌",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  (start typing or paste here)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "  (empty — press `i` to start typing or paste here)",
                Style::default().fg(Color::DarkGray),
            )));
        }
        return;
    }

    let buffer_lines: Vec<&str> = app.execute_sql_buffer.split('\n').collect();
    let last_idx = buffer_lines.len().saturating_sub(1);
    for (idx, line) in buffer_lines.iter().enumerate() {
        if idx == last_idx && app.execute_input_mode {
            // Render the cursor as a bold green block (▌) on its own span
            // so it stands out from the surrounding white text. Using a
            // distinct color is more reliable than a blink modifier, which
            // many terminals ignore.
            lines.push(Line::from(vec![
                Span::styled(format!("  {line}"), Style::default().fg(Color::White)),
                Span::styled(
                    "▌",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(Color::White),
            )));
        }
    }
}

fn draw_confirm(lines: &mut Vec<Line>, app: &AppState) {
    let db = app
        .execute_database
        .clone()
        .unwrap_or_else(|| "<none>".to_string());

    let n = app.execute_destructive_items.len();
    let heading = if n == 1 {
        format!("  About to run a destructive statement on {db}:")
    } else {
        format!("  About to run {n} destructive statements on {db}:")
    };
    lines.push(Line::from(Span::styled(
        heading,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for item in &app.execute_destructive_items {
        lines.push(Line::from(Span::styled(
            item.clone(),
            Style::default().fg(Color::White),
        )));
    }

    let total = app.execute_pending_statements.len();
    if total > n {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "  ({} more non-destructive statement(s) will also run.)",
                total - n
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  This will modify the database. Proceed? [y/N]",
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )));
}

fn draw_result(lines: &mut Vec<Line>, app: &AppState) {
    if app.is_loading {
        let frame_char = SPINNER_FRAMES[app.spinner_frame % SPINNER_FRAMES.len()];
        lines.push(Line::from(Span::styled(
            format!("  {frame_char} Executing..."),
            Style::default().fg(Color::Yellow),
        )));
        return;
    }

    if app.execute_results.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no result yet)",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    }

    let total = app.execute_results.len();
    lines.push(Line::from(Span::styled(
        format!("  Ran {total} statement(s):"),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (idx, result) in app.execute_results.iter().enumerate() {
        let line_no = app
            .execute_pending_statements
            .get(idx)
            .map(|s| s.start_line);
        let header = match line_no {
            Some(n) if n > 0 => format!("  -- line {n}: {}", result.command_tag),
            _ => format!("  -- {}", result.command_tag),
        };
        lines.push(Line::from(Span::styled(
            header,
            Style::default().fg(Color::Cyan),
        )));
        for body_line in format_execute_result(result).lines() {
            lines.push(Line::from(Span::styled(
                format!("  {body_line}"),
                Style::default().fg(Color::Green),
            )));
        }
        if idx + 1 < total {
            lines.push(Line::from(""));
        }
    }
}

fn footer_for(phase: ExecutePhase, input_mode: bool) -> &'static str {
    match phase {
        ExecutePhase::PickDatabase => "  j/k navigate  Enter pick  Esc back  q quit",
        ExecutePhase::EditSql => {
            if input_mode {
                "  F5/Ctrl+R run  Enter newline  Backspace delete  Esc stop typing"
            } else {
                "  i/Enter type  F5/Ctrl+R run  c clear  j/k scroll  Esc back  q quit"
            }
        }
        ExecutePhase::Confirm => "  y confirm  n/Esc cancel  q quit",
        ExecutePhase::Result => "  Esc back  q quit",
    }
}
