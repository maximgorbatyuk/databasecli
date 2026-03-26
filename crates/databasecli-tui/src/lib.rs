mod app;
mod event;
mod ui;

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self as ct_event, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use databasecli_core::config::{
    config_exists, create_default_config, load_databases, resolve_config_path,
};
use databasecli_core::health::check_all_health;

use app::{AppAction, AppState};

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let config_path = resolve_config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.databasecli/databases.ini".to_string());
    let has_config = config_exists().unwrap_or(false);
    let mut app = AppState::new(has_config, config_path);
    let mut health_rx: Option<mpsc::Receiver<Vec<databasecli_core::health::HealthResult>>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if ct_event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = ct_event::read()?
        {
            event::handle_key(&mut app, key);
        }

        if app.is_loading || health_rx.is_some() {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }

        if let Some(action) = app.take_action() {
            match action {
                AppAction::CreateConfig => {
                    match resolve_config_path().and_then(|p| {
                        create_default_config(&p)?;
                        Ok(p)
                    }) {
                        Ok(p) => {
                            let path_str = p.display().to_string();
                            app.on_config_created(path_str);
                        }
                        Err(e) => app.error_message = Some(e.to_string()),
                    }
                }
                AppAction::LoadDatabases => {
                    match resolve_config_path().and_then(|p| load_databases(&p)) {
                        Ok(configs) => app.databases = configs,
                        Err(e) => app.error_message = Some(e.to_string()),
                    }
                }
                AppAction::CheckHealth => {
                    let configs = match resolve_config_path().and_then(|p| load_databases(&p)) {
                        Ok(c) => c,
                        Err(e) => {
                            app.error_message = Some(e.to_string());
                            app.is_loading = false;
                            continue;
                        }
                    };

                    if configs.is_empty() {
                        app.health_results = Vec::new();
                        app.is_loading = false;
                    } else {
                        let (tx, rx) = mpsc::channel();
                        health_rx = Some(rx);
                        thread::spawn(move || {
                            let results = check_all_health(&configs);
                            let _ = tx.send(results);
                        });
                    }
                }
            }
        }

        if let Some(ref rx) = health_rx {
            match rx.try_recv() {
                Ok(results) => {
                    app.health_results = results;
                    app.is_loading = false;
                    health_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.error_message =
                        Some("Health check thread terminated unexpectedly".to_string());
                    app.is_loading = false;
                    health_rx = None;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
