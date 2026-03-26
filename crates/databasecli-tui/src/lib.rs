mod app;
mod event;
mod ui;

use std::io;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
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

use databasecli_core::commands::analyze::analyze_table;
use databasecli_core::commands::compare::compare_query;
use databasecli_core::commands::erd::build_erd;
use databasecli_core::commands::query::execute_query;
use databasecli_core::commands::sample::sample_table;
use databasecli_core::commands::schema::dump_schema;
use databasecli_core::commands::summary::summarize;
use databasecli_core::commands::trend::{TrendInterval, TrendParams, compute_trend};
use databasecli_core::config::{
    config_exists_with_base, create_default_config, load_databases, resolve_config_path_with_base,
};
use databasecli_core::connection::ConnectionManager;
use databasecli_core::health::check_all_health;

use app::{AppAction, AppState};

pub fn run(directory: Option<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, directory);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

enum BackgroundResult {
    Health(Vec<databasecli_core::health::HealthResult>),
    Connected {
        names: Vec<String>,
        errors: Vec<String>,
    },
    Schema(Result<Vec<databasecli_core::commands::schema::SchemaResult>, String>),
    Query(Result<databasecli_core::commands::query::QueryResultSet, String>),
    Sample(Result<databasecli_core::commands::sample::SampleResult, String>),
    Analyze(Result<databasecli_core::commands::analyze::TableProfile, String>),
    Summary(Result<Vec<databasecli_core::commands::summary::DatabaseSummary>, String>),
    Erd(Result<databasecli_core::commands::erd::ErdResult, String>),
    Compare(Result<databasecli_core::commands::compare::CompareResult, String>),
    Trend(Result<databasecli_core::commands::trend::TrendResult, String>),
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    directory: Option<String>,
) -> Result<()> {
    let base = directory.as_deref();
    let config_path = resolve_config_path_with_base(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.databasecli/databases.ini".to_string());
    let has_config = config_exists_with_base(base).unwrap_or(false);
    let mut app = AppState::new(has_config, config_path, directory);
    let conn_manager = Arc::new(Mutex::new(ConnectionManager::new()));
    let mut bg_rx: Option<mpsc::Receiver<BackgroundResult>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if ct_event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = ct_event::read()?
        {
            event::handle_key(&mut app, key);
        }

        if app.is_loading || bg_rx.is_some() {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }

        if let Some(action) = app.take_action() {
            match action {
                AppAction::CreateConfig => {
                    match resolve_config_path_with_base(app.directory.as_deref()).and_then(|p| {
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
                    match resolve_config_path_with_base(app.directory.as_deref())
                        .and_then(|p| load_databases(&p))
                    {
                        Ok(configs) => {
                            app.databases = configs;
                            app.on_databases_loaded();
                        }
                        Err(e) => app.error_message = Some(e.to_string()),
                    }
                }
                AppAction::CheckHealth => {
                    let configs = match resolve_config_path_with_base(app.directory.as_deref())
                        .and_then(|p| load_databases(&p))
                    {
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
                        bg_rx = Some(rx);
                        thread::spawn(move || {
                            let results = check_all_health(&configs);
                            let _ = tx.send(BackgroundResult::Health(results));
                        });
                    }
                }
                AppAction::ConnectDatabases(configs) => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let mut errors = Vec::new();
                        for config in &configs {
                            if let Err(e) = manager.connect(config) {
                                errors.push(format!("{}: {}", config.name, e));
                            }
                        }
                        let names = manager.connected_names();
                        let _ = tx.send(BackgroundResult::Connected { names, errors });
                    });
                }
                AppAction::DisconnectDatabases(names) => {
                    let mut manager = conn_manager.lock().unwrap();
                    for name in &names {
                        let _ = manager.disconnect(name);
                    }
                    let remaining = manager.connected_names();
                    app.update_connection_state(remaining);
                }
                AppAction::RunSchema => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let mut results = Vec::new();
                        for (_, conn) in manager.iter_mut() {
                            match dump_schema(conn, Some("public")) {
                                Ok(r) => results.push(r),
                                Err(e) => {
                                    let _ = tx.send(BackgroundResult::Schema(Err(e.to_string())));
                                    return;
                                }
                            }
                        }
                        let _ = tx.send(BackgroundResult::Schema(Ok(results)));
                    });
                }
                AppAction::RunQuery(sql) => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        // Run on first connected database
                        let result = manager
                            .iter_mut()
                            .next()
                            .map(|(_, conn)| execute_query(conn, &sql).map_err(|e| e.to_string()));
                        match result {
                            Some(r) => {
                                let _ = tx.send(BackgroundResult::Query(r));
                            }
                            None => {
                                let _ = tx.send(BackgroundResult::Query(Err(
                                    "No active connections".to_string(),
                                )));
                            }
                        }
                    });
                }
                AppAction::RunSample(table_name) => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let result = manager.iter_mut().next().map(|(_, conn)| {
                            sample_table(conn, &table_name, Some("public"), Some(20), None)
                                .map_err(|e| e.to_string())
                        });
                        match result {
                            Some(r) => {
                                let _ = tx.send(BackgroundResult::Sample(r));
                            }
                            None => {
                                let _ = tx.send(BackgroundResult::Sample(Err(
                                    "No active connections".to_string(),
                                )));
                            }
                        }
                    });
                }
                AppAction::RunAnalyze(table_name) => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let result = manager.iter_mut().next().map(|(_, conn)| {
                            analyze_table(conn, &table_name, Some("public"))
                                .map_err(|e| e.to_string())
                        });
                        match result {
                            Some(r) => {
                                let _ = tx.send(BackgroundResult::Analyze(r));
                            }
                            None => {
                                let _ = tx.send(BackgroundResult::Analyze(Err(
                                    "No active connections".to_string(),
                                )));
                            }
                        }
                    });
                }
                AppAction::RunSummary => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let mut results = Vec::new();
                        for (_, conn) in manager.iter_mut() {
                            match summarize(conn) {
                                Ok(r) => results.push(r),
                                Err(e) => {
                                    let _ = tx.send(BackgroundResult::Summary(Err(e.to_string())));
                                    return;
                                }
                            }
                        }
                        let _ = tx.send(BackgroundResult::Summary(Ok(results)));
                    });
                }
                AppAction::RunErd => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let result = manager.iter_mut().next().map(|(_, conn)| {
                            build_erd(conn, Some("public")).map_err(|e| e.to_string())
                        });
                        match result {
                            Some(r) => {
                                let _ = tx.send(BackgroundResult::Erd(r));
                            }
                            None => {
                                let _ = tx.send(BackgroundResult::Erd(Err(
                                    "No active connections".to_string(),
                                )));
                            }
                        }
                    });
                }
                AppAction::RunCompare(sql) => {
                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let result = compare_query(&mut manager, &sql).map_err(|e| e.to_string());
                        let _ = tx.send(BackgroundResult::Compare(result));
                    });
                }
                AppAction::RunTrend(input) => {
                    // Parse input: "table timestamp_col [interval] [value_col]"
                    let parts: Vec<&str> = input.split_whitespace().collect();
                    if parts.len() < 2 {
                        app.error_message =
                            Some("Format: table timestamp_col [interval] [value_col]".to_string());
                        app.is_loading = false;
                        continue;
                    }

                    let table = parts[0].to_string();
                    let timestamp_column = parts[1].to_string();
                    let interval = if parts.len() > 2 {
                        match TrendInterval::parse_interval(parts[2]) {
                            Ok(i) => i,
                            Err(e) => {
                                app.error_message = Some(e.to_string());
                                app.is_loading = false;
                                continue;
                            }
                        }
                    } else {
                        TrendInterval::Day
                    };
                    let value_column = parts.get(3).map(|s| s.to_string());

                    let params = TrendParams {
                        table,
                        schema: "public".to_string(),
                        timestamp_column,
                        interval,
                        value_column,
                        limit: Some(30),
                    };

                    let mgr = Arc::clone(&conn_manager);
                    let (tx, rx) = mpsc::channel();
                    bg_rx = Some(rx);
                    thread::spawn(move || {
                        let mut manager = mgr.lock().unwrap();
                        let result = manager.iter_mut().next().map(|(_, conn)| {
                            compute_trend(conn, &params).map_err(|e| e.to_string())
                        });
                        match result {
                            Some(r) => {
                                let _ = tx.send(BackgroundResult::Trend(r));
                            }
                            None => {
                                let _ = tx.send(BackgroundResult::Trend(Err(
                                    "No active connections".to_string(),
                                )));
                            }
                        }
                    });
                }
            }
        }

        if let Some(ref rx) = bg_rx {
            match rx.try_recv() {
                Ok(result) => {
                    match result {
                        BackgroundResult::Health(results) => {
                            app.health_results = results;
                        }
                        BackgroundResult::Connected { names, errors } => {
                            app.update_connection_state(names);
                            if !errors.is_empty() {
                                app.error_message = Some(errors.join("\n"));
                            } else {
                                app.status_message =
                                    Some(format!("{} database(s) connected", app.connected_count));
                            }
                            // Update selection to reflect new state
                            app.on_databases_loaded();
                        }
                        BackgroundResult::Schema(Ok(results)) => {
                            app.schema_results = Some(results);
                        }
                        BackgroundResult::Schema(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Query(Ok(result)) => {
                            app.query_result = Some(result);
                        }
                        BackgroundResult::Query(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Sample(Ok(result)) => {
                            app.sample_result = Some(result);
                        }
                        BackgroundResult::Sample(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Analyze(Ok(result)) => {
                            app.analyze_result = Some(result);
                        }
                        BackgroundResult::Analyze(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Summary(Ok(results)) => {
                            app.summary_results = Some(results);
                        }
                        BackgroundResult::Summary(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Erd(Ok(result)) => {
                            app.erd_result = Some(result);
                        }
                        BackgroundResult::Erd(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Compare(Ok(result)) => {
                            app.compare_result = Some(result);
                        }
                        BackgroundResult::Compare(Err(e)) => {
                            app.error_message = Some(e);
                        }
                        BackgroundResult::Trend(Ok(result)) => {
                            app.trend_result = Some(result);
                        }
                        BackgroundResult::Trend(Err(e)) => {
                            app.error_message = Some(e);
                        }
                    }
                    app.is_loading = false;
                    bg_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.error_message =
                        Some("Background operation terminated unexpectedly".to_string());
                    app.is_loading = false;
                    bg_rx = None;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
