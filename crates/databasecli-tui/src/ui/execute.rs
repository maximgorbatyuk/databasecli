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
        footer_for(app.execute_phase),
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
    let cursor = if app.execute_input_mode { "_" } else { "" };
    lines.push(Line::from(vec![
        Span::styled("  SQL> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}{}", app.execute_sql_buffer, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
}

fn draw_confirm(lines: &mut Vec<Line>, app: &AppState) {
    let kind = app
        .execute_pending_kind
        .map(|k| format!("{k:?}").to_uppercase())
        .unwrap_or_else(|| "STATEMENT".to_string());
    let db = app
        .execute_database
        .clone()
        .unwrap_or_else(|| "<none>".to_string());

    lines.push(Line::from(Span::styled(
        format!("  About to run a {kind} statement on {db}:"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("    {}", app.execute_sql_buffer.trim()),
        Style::default().fg(Color::White),
    )));
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

    let Some(ref result) = app.execute_result else {
        lines.push(Line::from(Span::styled(
            "  (no result yet)",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    };

    let formatted = format_execute_result(result);
    for line in formatted.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {line}"),
            Style::default().fg(Color::Green),
        )));
    }
}

fn footer_for(phase: ExecutePhase) -> &'static str {
    match phase {
        ExecutePhase::PickDatabase => "  j/k navigate  Enter pick  Esc back  q quit",
        ExecutePhase::EditSql => "  i/Enter type  Enter run  Esc stop typing  q quit",
        ExecutePhase::Confirm => "  y confirm  n/Esc cancel  q quit",
        ExecutePhase::Result => "  Esc back  q quit",
    }
}
