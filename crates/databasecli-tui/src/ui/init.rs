use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::AppState;

pub fn draw_init(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Initialize Project",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Config file:",
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
            "  Select coding agents to configure MCP for:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    for (i, agent) in app.init_agents.iter().enumerate() {
        let selected = app.init_agent_selection.get(i).copied().unwrap_or(false);
        let cursor = if i == app.init_agent_cursor { ">" } else { " " };
        let check = if selected { "x" } else { " " };
        let label = format!(
            "  {} [{}] {} ({})",
            cursor,
            check,
            agent,
            agent.config_filename()
        );

        let style = if i == app.init_agent_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(Span::styled(label, style)));
    }

    if let Some(ref err) = app.error_message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {err}"),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Space toggle  j/k move  Enter confirm  Esc back  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
