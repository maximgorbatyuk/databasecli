use crossterm::event::KeyCode;

use crate::app::{AppState, ExecutePhase};

pub fn handle_execute(app: &mut AppState, code: KeyCode) {
    match app.execute_phase {
        ExecutePhase::PickDatabase => handle_picker(app, code),
        ExecutePhase::EditSql => handle_editor(app, code),
        ExecutePhase::Confirm => handle_confirm(app, code),
        ExecutePhase::Result => handle_result(app, code),
    }
}

fn handle_picker(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => app.go_home(),
        KeyCode::Up | KeyCode::Char('k') => app.execute_picker_up(),
        KeyCode::Down | KeyCode::Char('j') => app.execute_picker_down(),
        KeyCode::Enter => app.execute_picker_confirm(),
        _ => {}
    }
}

fn handle_editor(app: &mut AppState, code: KeyCode) {
    if app.execute_input_mode {
        match code {
            KeyCode::Esc => app.execute_input_mode = false,
            KeyCode::Enter => app.execute_submit_sql(),
            KeyCode::Backspace => {
                app.execute_sql_buffer.pop();
            }
            KeyCode::Char(c) => app.execute_sql_buffer.push(c),
            _ => {}
        }
    } else {
        match code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Esc => app.go_home(),
            KeyCode::Char('i') | KeyCode::Enter => app.execute_input_mode = true,
            KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
            _ => {}
        }
    }
}

fn handle_confirm(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => app.execute_confirm_yes(),
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => app.execute_confirm_no(),
        KeyCode::Char('q') => app.quit(),
        _ => {}
    }
}

fn handle_result(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => app.go_home(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        _ => {}
    }
}
