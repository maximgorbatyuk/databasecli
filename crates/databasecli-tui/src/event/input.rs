use crossterm::event::KeyCode;

use crate::app::AppState;

pub fn handle_input_screen(app: &mut AppState, code: KeyCode) {
    if app.input_mode {
        match code {
            KeyCode::Esc => app.input_mode = false,
            KeyCode::Enter => app.submit_input(),
            KeyCode::Backspace => {
                app.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                app.input_buffer.push(c);
            }
            _ => {}
        }
    } else {
        match code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Esc => app.go_home(),
            KeyCode::Char('i') | KeyCode::Enter => app.input_mode = true,
            KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
            _ => {}
        }
    }
}
