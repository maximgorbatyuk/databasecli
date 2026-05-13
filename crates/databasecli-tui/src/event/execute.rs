use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{AppState, ExecutePhase};

pub fn handle_execute(app: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    // Ctrl+R is a global Execute-screen run shortcut. Some terminals
    // (notably tmux without `set -g xterm-keys on`, and a handful of older
    // emulators) drop the F5 function key, so the operator needs a
    // modifier-based fallback that every terminal forwards reliably.
    // Ctrl+R fires in both input-mode and read-mode so the operator can
    // run without first leaving the editor.
    if modifiers.contains(KeyModifiers::CONTROL)
        && matches!(code, KeyCode::Char('r') | KeyCode::Char('R'))
        && app.execute_phase == ExecutePhase::EditSql
    {
        app.execute_run();
        return;
    }

    match app.execute_phase {
        ExecutePhase::PickDatabase => handle_picker(app, code),
        ExecutePhase::EditSql => handle_editor(app, code, modifiers),
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

fn handle_editor(app: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    if app.execute_input_mode {
        // Edit mode: F5 runs the buffer; Enter inserts a newline so users can
        // type or paste multi-statement scripts (BEGIN/COMMIT, WITH ... DML).
        // Esc leaves edit mode without running; from there `i` re-enters,
        // `c` clears the buffer. Plain `Char(c)` is filtered to skip
        // control-modified keys (Ctrl+R fallback is handled in
        // handle_execute), so the buffer never absorbs a modifier shortcut.
        match code {
            KeyCode::Esc => app.execute_input_mode = false,
            KeyCode::F(5) => app.execute_run(),
            KeyCode::Enter => app.execute_sql_buffer.push('\n'),
            KeyCode::Backspace => {
                app.execute_sql_buffer.pop();
            }
            KeyCode::Tab => app.execute_sql_buffer.push_str("  "),
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                app.execute_sql_buffer.push(c)
            }
            _ => {}
        }
    } else {
        match code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Esc => app.go_home(),
            KeyCode::F(5) => app.execute_run(),
            KeyCode::Char('i') | KeyCode::Enter => app.execute_input_mode = true,
            KeyCode::Char('c') => app.execute_sql_buffer.clear(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, MenuItem};

    fn execute_screen_app(buffer: &str) -> AppState {
        let mut app = AppState::new(true, "test".to_string(), None);
        app.update_connection_state(vec!["only".to_string()]);
        let idx = app
            .menu_items
            .iter()
            .position(|m| matches!(m, MenuItem::Execute))
            .expect("Execute menu item exists");
        app.selected = idx;
        app.activate_selected();
        app.execute_sql_buffer = buffer.to_string();
        app
    }

    #[test]
    fn ctrl_r_runs_buffer_in_input_mode() {
        let mut app = execute_screen_app("INSERT INTO t VALUES (1)");
        assert!(app.execute_input_mode);
        handle_execute(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(app.execute_phase, ExecutePhase::Result);
        assert!(app.is_loading);
    }

    #[test]
    fn ctrl_r_runs_buffer_outside_input_mode() {
        let mut app = execute_screen_app("INSERT INTO t VALUES (1)");
        app.execute_input_mode = false;
        handle_execute(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(app.execute_phase, ExecutePhase::Result);
    }

    #[test]
    fn ctrl_r_uppercase_also_runs() {
        let mut app = execute_screen_app("INSERT INTO t VALUES (1)");
        handle_execute(&mut app, KeyCode::Char('R'), KeyModifiers::CONTROL);
        assert_eq!(app.execute_phase, ExecutePhase::Result);
    }

    #[test]
    fn plain_r_in_input_mode_appends_to_buffer() {
        let mut app = execute_screen_app("INSE");
        handle_execute(&mut app, KeyCode::Char('R'), KeyModifiers::NONE);
        assert_eq!(app.execute_sql_buffer, "INSER");
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
    }

    #[test]
    fn ctrl_modified_char_does_not_pollute_buffer() {
        let mut app = execute_screen_app("");
        // Random ctrl-letter shouldn't land in the buffer as a literal char.
        handle_execute(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(app.execute_sql_buffer, "");
    }

    #[test]
    fn f5_still_runs_when_modifier_path_doesnt_match() {
        let mut app = execute_screen_app("INSERT INTO t VALUES (1)");
        handle_execute(&mut app, KeyCode::F(5), KeyModifiers::NONE);
        assert_eq!(app.execute_phase, ExecutePhase::Result);
    }
}
