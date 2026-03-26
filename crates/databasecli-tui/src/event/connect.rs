use crossterm::event::KeyCode;

use crate::app::AppState;

pub fn handle_connect(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => app.go_home(),
        KeyCode::Up | KeyCode::Char('k') => app.connect_cursor_up(),
        KeyCode::Down | KeyCode::Char('j') => app.connect_cursor_down(),
        KeyCode::Char(' ') => app.toggle_connect_selection(),
        KeyCode::Enter => app.confirm_connect(),
        _ => {}
    }
}
