use crossterm::event::KeyCode;

use crate::app::AppState;

pub fn handle_home(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Enter => app.activate_selected(),
        _ => {}
    }
}
