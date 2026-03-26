use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::app::{AppState, Screen};

fn normalize_to_qwerty(c: char) -> char {
    match c {
        'й' => 'q',
        'ц' => 'w',
        'у' => 'e',
        'к' => 'r',
        'е' => 't',
        'н' => 'y',
        'г' => 'u',
        'ш' => 'i',
        'щ' => 'o',
        'з' => 'p',
        'ф' => 'a',
        'ы' => 's',
        'в' => 'd',
        'а' => 'f',
        'п' => 'g',
        'р' => 'h',
        'о' => 'j',
        'л' => 'k',
        'д' => 'l',
        'я' => 'z',
        'ч' => 'x',
        'с' => 'c',
        'м' => 'v',
        'и' => 'b',
        'т' => 'n',
        'ь' => 'm',
        _ => c,
    }
}

pub fn handle_key(app: &mut AppState, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }

    let code = match key.code {
        KeyCode::Char(c) => KeyCode::Char(normalize_to_qwerty(c)),
        other => other,
    };

    match app.active_screen {
        Screen::Home => handle_home(app, code),
        Screen::CreateConfig => handle_create_config(app, code),
        Screen::StoredDatabases => handle_stored_databases(app, code),
        Screen::DatabaseHealth => handle_database_health(app, code),
    }
}

fn handle_home(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Enter => app.activate_selected(),
        _ => {}
    }
}

fn handle_create_config(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => app.go_home(),
        KeyCode::Enter => app.confirm_create_config(),
        _ => {}
    }
}

fn handle_stored_databases(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => app.go_home(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        _ => {}
    }
}

fn handle_database_health(app: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => app.go_home(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        _ => {}
    }
}
