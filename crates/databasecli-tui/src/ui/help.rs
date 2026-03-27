use std::sync::LazyLock;

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use databasecli_core::help::{HelpSection, build_help_sections};

use crate::app::AppState;

static HELP_SECTIONS: LazyLock<Vec<HelpSection>> = LazyLock::new(build_help_sections);

pub fn draw_help(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let sections = &*HELP_SECTIONS;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  databasecli — PostgreSQL database connection manager",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for section in sections {
        lines.push(Line::from(Span::styled(
            format!("  {}", section.title),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for item in &section.items {
            lines.push(Line::from(vec![Span::styled(
                format!("    {}", item.name),
                Style::default().fg(Color::White),
            )]));
            lines.push(Line::from(vec![Span::styled(
                format!("      {}", item.description),
                Style::default().fg(Color::DarkGray),
            )]));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "  ↑/k ↓/j scroll  Esc back  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
