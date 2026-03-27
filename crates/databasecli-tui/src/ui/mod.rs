mod analyze;
mod compare;
mod connect;
mod create_config;
mod databases;
mod erd;
mod health;
mod help;
mod home;
mod query;
mod sample;
mod schema;
mod summary;
mod trend;

use ratatui::Frame;

use crate::app::{AppState, Screen};

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();
    match app.active_screen {
        Screen::Home => home::draw_home(frame, app, area),
        Screen::CreateConfig => create_config::draw_create_config(frame, app, area),
        Screen::Connect => connect::draw_connect(frame, app, area),
        Screen::StoredDatabases => databases::draw_stored_databases(frame, app, area),
        Screen::DatabaseHealth => health::draw_database_health(frame, app, area),
        Screen::Schema => schema::draw_schema(frame, app, area),
        Screen::Query => query::draw_query(frame, app, area),
        Screen::Sample => sample::draw_sample(frame, app, area),
        Screen::Analyze => analyze::draw_analyze(frame, app, area),
        Screen::Summary => summary::draw_summary(frame, app, area),
        Screen::Erd => erd::draw_erd(frame, app, area),
        Screen::Compare => compare::draw_compare(frame, app, area),
        Screen::Trend => trend::draw_trend(frame, app, area),
        Screen::Help => help::draw_help(frame, app, area),
    }
}
