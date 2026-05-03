use std::fmt;

use databasecli_core::commands::analyze::TableProfile;
use databasecli_core::commands::compare::CompareResult;
use databasecli_core::commands::erd::ErdResult;
use databasecli_core::commands::execute::{ExecuteResult, StatementKind, classify_statement};
use databasecli_core::commands::query::QueryResultSet;
use databasecli_core::commands::sample::SampleResult;
use databasecli_core::commands::schema::SchemaResult;
use databasecli_core::commands::summary::DatabaseSummary;
use databasecli_core::commands::trend::TrendResult;
use databasecli_core::config::DatabaseConfig;
use databasecli_core::health::HealthResult;
use databasecli_core::init::CodingAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Home,
    CreateConfig,
    Init,
    Connect,
    StoredDatabases,
    DatabaseHealth,
    Schema,
    Query,
    Execute,
    Sample,
    Analyze,
    Summary,
    Erd,
    Compare,
    Trend,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    CreateConfig,
    Init,
    Connect,
    StoredDatabases,
    DatabaseHealth,
    Schema,
    Query,
    Execute,
    Sample,
    Analyze,
    Summary,
    Erd,
    Compare,
    Trend,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutePhase {
    PickDatabase,
    EditSql,
    Confirm,
    Result,
}

impl MenuItem {
    pub fn description(&self) -> &'static str {
        match self {
            MenuItem::CreateConfig => "Create the databases.ini config file",
            MenuItem::Init => "Create config and configure MCP for coding agents",
            MenuItem::Connect => "Select databases to connect to",
            MenuItem::StoredDatabases => "View all configured database connections",
            MenuItem::DatabaseHealth => "Check connectivity for all databases",
            MenuItem::Schema => "Full schema: tables, columns, types, PKs",
            MenuItem::Query => "Run read-only SQL query",
            MenuItem::Execute => "Run a write/DDL SQL statement (local only; not exposed via MCP)",
            MenuItem::Sample => "Preview rows from a table",
            MenuItem::Analyze => "Profile a table: nulls, cardinality, top values",
            MenuItem::Summary => "Overview: table counts, sizes, largest tables",
            MenuItem::Erd => "Entity-relationship diagram: PKs and FKs",
            MenuItem::Compare => "Same query across all connected databases",
            MenuItem::Trend => "Time-series: counts/averages by interval",
            MenuItem::Help => "Commands, keys, config, MCP, security reference",
        }
    }

    pub fn screen(&self) -> Screen {
        match self {
            MenuItem::CreateConfig => Screen::CreateConfig,
            MenuItem::Init => Screen::Init,
            MenuItem::Connect => Screen::Connect,
            MenuItem::StoredDatabases => Screen::StoredDatabases,
            MenuItem::DatabaseHealth => Screen::DatabaseHealth,
            MenuItem::Schema => Screen::Schema,
            MenuItem::Query => Screen::Query,
            MenuItem::Execute => Screen::Execute,
            MenuItem::Sample => Screen::Sample,
            MenuItem::Analyze => Screen::Analyze,
            MenuItem::Summary => Screen::Summary,
            MenuItem::Erd => Screen::Erd,
            MenuItem::Compare => Screen::Compare,
            MenuItem::Trend => Screen::Trend,
            MenuItem::Help => Screen::Help,
        }
    }

    pub fn requires_connection(&self) -> bool {
        matches!(
            self,
            MenuItem::Schema
                | MenuItem::Query
                | MenuItem::Execute
                | MenuItem::Sample
                | MenuItem::Analyze
                | MenuItem::Summary
                | MenuItem::Erd
                | MenuItem::Compare
                | MenuItem::Trend
        )
    }
}

impl fmt::Display for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItem::CreateConfig => write!(f, "Create database.ini"),
            MenuItem::Init => write!(f, "Initialize Project"),
            MenuItem::Connect => write!(f, "Connect"),
            MenuItem::StoredDatabases => write!(f, "Stored Databases"),
            MenuItem::DatabaseHealth => write!(f, "Database Health"),
            MenuItem::Schema => write!(f, "Schema"),
            MenuItem::Query => write!(f, "Query"),
            MenuItem::Execute => write!(f, "Execute"),
            MenuItem::Sample => write!(f, "Sample"),
            MenuItem::Analyze => write!(f, "Analyze"),
            MenuItem::Summary => write!(f, "Summary"),
            MenuItem::Erd => write!(f, "ERD"),
            MenuItem::Compare => write!(f, "Compare"),
            MenuItem::Trend => write!(f, "Trend"),
            MenuItem::Help => write!(f, "Help"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppAction {
    CreateConfig,
    RunInit(Vec<CodingAgent>),
    LoadDatabases,
    CheckHealth,
    ConnectDatabases(Vec<DatabaseConfig>),
    DisconnectDatabases(Vec<String>),
    RunSchema,
    RunQuery(String),
    ExecuteStatement { database: String, sql: String },
    RunSample(String),
    RunAnalyze(String),
    RunSummary,
    RunErd,
    RunCompare(String),
    RunTrend(String),
}

pub struct AppState {
    pub menu_items: Vec<MenuItem>,
    pub selected: usize,
    pub active_screen: Screen,
    pub should_quit: bool,
    pub databases: Vec<DatabaseConfig>,
    pub health_results: Vec<HealthResult>,
    pub is_loading: bool,
    pub spinner_frame: usize,
    pub scroll_offset: u16,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub config_path: String,
    pub current_dir: String,
    pub directory: Option<String>,

    // Connection state
    pub connected_count: usize,
    pub connected_names: Vec<String>,
    pub connect_cursor: usize,
    pub connect_selection: Vec<bool>,

    // Init agent selection
    pub init_agents: Vec<CodingAgent>,
    pub init_agent_selection: Vec<bool>,
    pub init_agent_cursor: usize,

    // Input state
    pub input_buffer: String,
    pub input_mode: bool,

    // Result state
    pub schema_results: Option<Vec<SchemaResult>>,
    pub query_result: Option<QueryResultSet>,
    pub sample_result: Option<SampleResult>,
    pub analyze_result: Option<TableProfile>,
    pub summary_results: Option<Vec<DatabaseSummary>>,
    pub erd_result: Option<ErdResult>,
    pub compare_result: Option<CompareResult>,
    pub trend_result: Option<TrendResult>,

    // Execute screen state — explicit phase machine instead of overloading input_mode/query_result
    pub execute_phase: ExecutePhase,
    pub execute_db_cursor: usize,
    pub execute_database: Option<String>,
    pub execute_sql_buffer: String,
    pub execute_input_mode: bool,
    pub execute_pending_kind: Option<StatementKind>,
    pub execute_result: Option<ExecuteResult>,

    pending_action: Option<AppAction>,
}

impl AppState {
    pub fn new(config_exists: bool, config_path: String, directory: Option<String>) -> Self {
        let mut menu_items = Vec::new();
        if !config_exists {
            menu_items.push(MenuItem::CreateConfig);
        }
        menu_items.push(MenuItem::Init);
        menu_items.push(MenuItem::Connect);
        menu_items.push(MenuItem::StoredDatabases);
        menu_items.push(MenuItem::DatabaseHealth);
        menu_items.push(MenuItem::Schema);
        menu_items.push(MenuItem::Query);
        menu_items.push(MenuItem::Execute);
        menu_items.push(MenuItem::Sample);
        menu_items.push(MenuItem::Analyze);
        menu_items.push(MenuItem::Summary);
        menu_items.push(MenuItem::Erd);
        menu_items.push(MenuItem::Compare);
        menu_items.push(MenuItem::Trend);
        menu_items.push(MenuItem::Help);

        let current_dir = directory.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(unknown)".to_string())
        });

        Self {
            menu_items,
            selected: 0,
            active_screen: Screen::Home,
            should_quit: false,
            databases: Vec::new(),
            health_results: Vec::new(),
            is_loading: false,
            spinner_frame: 0,
            scroll_offset: 0,
            error_message: None,
            status_message: None,
            config_path,
            current_dir,
            directory,
            connected_count: 0,
            connected_names: Vec::new(),
            connect_cursor: 0,
            connect_selection: Vec::new(),
            init_agents: CodingAgent::ALL.to_vec(),
            init_agent_selection: vec![false; CodingAgent::ALL.len()],
            init_agent_cursor: 0,
            input_buffer: String::new(),
            input_mode: false,
            schema_results: None,
            query_result: None,
            sample_result: None,
            analyze_result: None,
            summary_results: None,
            erd_result: None,
            compare_result: None,
            trend_result: None,
            execute_phase: ExecutePhase::PickDatabase,
            execute_db_cursor: 0,
            execute_database: None,
            execute_sql_buffer: String::new(),
            execute_input_mode: false,
            execute_pending_kind: None,
            execute_result: None,
            pending_action: None,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.menu_items.len() {
            self.selected += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn activate_selected(&mut self) {
        let item = &self.menu_items[self.selected];

        // Check if connected databases are required
        if item.requires_connection() && self.connected_count == 0 {
            self.status_message = Some("Connect to a database first.".to_string());
            return;
        }

        let screen = item.screen();

        match screen {
            Screen::CreateConfig => {
                // Just navigate to confirmation screen
            }
            Screen::Init => {
                self.init_agent_selection = vec![false; self.init_agents.len()];
                self.init_agent_cursor = 0;
            }
            Screen::Connect => {
                self.pending_action = Some(AppAction::LoadDatabases);
            }
            Screen::StoredDatabases => {
                self.pending_action = Some(AppAction::LoadDatabases);
            }
            Screen::DatabaseHealth => {
                self.pending_action = Some(AppAction::CheckHealth);
                self.is_loading = true;
            }
            Screen::Schema => {
                self.pending_action = Some(AppAction::RunSchema);
                self.is_loading = true;
                self.schema_results = None;
            }
            Screen::Query => {
                self.input_mode = true;
                self.input_buffer.clear();
                self.query_result = None;
            }
            Screen::Execute => {
                self.enter_execute_screen();
            }
            Screen::Sample => {
                self.input_mode = true;
                self.input_buffer.clear();
                self.sample_result = None;
            }
            Screen::Analyze => {
                self.input_mode = true;
                self.input_buffer.clear();
                self.analyze_result = None;
            }
            Screen::Summary => {
                self.pending_action = Some(AppAction::RunSummary);
                self.is_loading = true;
                self.summary_results = None;
            }
            Screen::Erd => {
                self.pending_action = Some(AppAction::RunErd);
                self.is_loading = true;
                self.erd_result = None;
            }
            Screen::Compare => {
                self.input_mode = true;
                self.input_buffer.clear();
                self.compare_result = None;
            }
            Screen::Trend => {
                self.input_mode = true;
                self.input_buffer.clear();
                self.trend_result = None;
            }
            _ => {}
        }

        self.active_screen = screen;
        self.scroll_offset = 0;
        self.error_message = None;
    }

    pub fn confirm_create_config(&mut self) {
        self.pending_action = Some(AppAction::CreateConfig);
    }

    pub fn confirm_init(&mut self) {
        let chosen: Vec<CodingAgent> = self
            .init_agents
            .iter()
            .enumerate()
            .filter(|(i, _)| self.init_agent_selection.get(*i).copied().unwrap_or(false))
            .map(|(_, a)| *a)
            .collect();
        self.pending_action = Some(AppAction::RunInit(chosen));
    }

    pub fn init_agent_cursor_up(&mut self) {
        if self.init_agent_cursor > 0 {
            self.init_agent_cursor -= 1;
        }
    }

    pub fn init_agent_cursor_down(&mut self) {
        if self.init_agent_cursor + 1 < self.init_agents.len() {
            self.init_agent_cursor += 1;
        }
    }

    pub fn toggle_init_agent(&mut self) {
        if let Some(val) = self.init_agent_selection.get_mut(self.init_agent_cursor) {
            *val = !*val;
        }
    }

    pub fn on_init_completed(&mut self, message: String, config_created: bool) {
        if config_created {
            self.menu_items
                .retain(|item| *item != MenuItem::CreateConfig);
        }
        self.selected = 0;
        self.active_screen = Screen::Home;
        self.scroll_offset = 0;
        self.error_message = None;
        self.is_loading = false;
        self.spinner_frame = 0;
        self.status_message = Some(message);
    }

    pub fn go_home(&mut self) {
        self.active_screen = Screen::Home;
        self.scroll_offset = 0;
        self.error_message = None;
        self.status_message = None;
        self.is_loading = false;
        self.spinner_frame = 0;
        self.input_mode = false;
        self.execute_input_mode = false;
    }

    pub fn on_config_created(&mut self, path: String) {
        self.menu_items
            .retain(|item| *item != MenuItem::CreateConfig);
        self.selected = 0;
        self.active_screen = Screen::Home;
        self.scroll_offset = 0;
        self.error_message = None;
        self.is_loading = false;
        self.spinner_frame = 0;
        self.status_message = Some(format!("Config created at {path}"));
    }

    // Connect screen methods
    pub fn connect_cursor_up(&mut self) {
        if self.connect_cursor > 0 {
            self.connect_cursor -= 1;
        }
    }

    pub fn connect_cursor_down(&mut self) {
        if self.connect_cursor + 1 < self.databases.len() {
            self.connect_cursor += 1;
        }
    }

    pub fn toggle_connect_selection(&mut self) {
        if let Some(val) = self.connect_selection.get_mut(self.connect_cursor) {
            *val = !*val;
        }
    }

    pub fn confirm_connect(&mut self) {
        let to_connect: Vec<DatabaseConfig> = self
            .databases
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                self.connect_selection.get(*i).copied().unwrap_or(false)
                    && !self.connected_names.contains(&self.databases[*i].name)
            })
            .map(|(_, db)| db.clone())
            .collect();

        let to_disconnect: Vec<String> = self
            .connected_names
            .iter()
            .filter(|name| {
                let idx = self.databases.iter().position(|d| d.name == **name);
                match idx {
                    Some(i) => !self.connect_selection.get(i).copied().unwrap_or(false),
                    None => true,
                }
            })
            .cloned()
            .collect();

        if !to_disconnect.is_empty() {
            self.pending_action = Some(AppAction::DisconnectDatabases(to_disconnect));
        }
        if !to_connect.is_empty() {
            self.is_loading = true;
            self.pending_action = Some(AppAction::ConnectDatabases(to_connect));
        }
    }

    pub fn on_databases_loaded(&mut self) {
        // Initialize connect selection based on current connection state
        self.connect_selection = self
            .databases
            .iter()
            .map(|db| self.connected_names.contains(&db.name))
            .collect();
        self.connect_cursor = 0;
    }

    pub fn update_connection_state(&mut self, names: Vec<String>) {
        self.connected_count = names.len();
        self.connected_names = names;
    }

    // Input methods
    pub fn submit_input(&mut self) {
        if self.input_buffer.trim().is_empty() {
            return;
        }
        self.input_mode = false;
        let input = self.input_buffer.clone();
        self.error_message = None;
        self.is_loading = true;

        match self.active_screen {
            Screen::Query => {
                self.query_result = None;
                self.pending_action = Some(AppAction::RunQuery(input));
            }
            Screen::Sample => {
                self.sample_result = None;
                self.pending_action = Some(AppAction::RunSample(input));
            }
            Screen::Analyze => {
                self.analyze_result = None;
                self.pending_action = Some(AppAction::RunAnalyze(input));
            }
            Screen::Compare => {
                self.compare_result = None;
                self.pending_action = Some(AppAction::RunCompare(input));
            }
            Screen::Trend => {
                self.trend_result = None;
                self.pending_action = Some(AppAction::RunTrend(input));
            }
            _ => {}
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn take_action(&mut self) -> Option<AppAction> {
        self.pending_action.take()
    }

    /// True when any screen has its text-input buffer active. Used by the key
    /// router to suppress QWERTY normalization while the user is typing SQL.
    /// Centralised so adding a future screen with its own input flag is a
    /// one-line change here.
    pub fn is_typing(&self) -> bool {
        self.input_mode || self.execute_input_mode
    }

    // Execute screen state machine -------------------------------------------------

    fn enter_execute_screen(&mut self) {
        self.execute_sql_buffer.clear();
        self.execute_input_mode = false;
        self.execute_pending_kind = None;
        self.execute_result = None;
        self.execute_db_cursor = 0;

        match self.connected_names.as_slice() {
            [single] => {
                self.execute_database = Some(single.clone());
                self.execute_phase = ExecutePhase::EditSql;
                self.execute_input_mode = true;
            }
            [] => unreachable!(
                "MenuItem::Execute::requires_connection() must prevent activation when no databases are connected"
            ),
            _ => {
                self.execute_database = None;
                self.execute_phase = ExecutePhase::PickDatabase;
            }
        }
    }

    pub fn execute_picker_up(&mut self) {
        if self.execute_db_cursor > 0 {
            self.execute_db_cursor -= 1;
        }
    }

    pub fn execute_picker_down(&mut self) {
        if self.execute_db_cursor + 1 < self.connected_names.len() {
            self.execute_db_cursor += 1;
        }
    }

    pub fn execute_picker_confirm(&mut self) {
        if let Some(name) = self.connected_names.get(self.execute_db_cursor) {
            self.execute_database = Some(name.clone());
            self.execute_phase = ExecutePhase::EditSql;
            self.execute_input_mode = true;
            self.execute_sql_buffer.clear();
            self.error_message = None;
        }
    }

    pub fn execute_submit_sql(&mut self) {
        let sql = self.execute_sql_buffer.trim().to_string();
        if sql.is_empty() {
            return;
        }
        self.execute_input_mode = false;
        self.error_message = None;

        let kind = classify_statement(&sql);
        match kind {
            StatementKind::Read => {
                self.error_message =
                    Some("Read-only SQL — use the Query screen instead.".to_string());
                self.execute_input_mode = true;
            }
            StatementKind::Unsupported => {
                self.error_message = Some(
                    "Statement not supported by Execute (no WITH, no procedural bodies, single statement only)."
                        .to_string(),
                );
                self.execute_input_mode = true;
            }
            StatementKind::Destructive => {
                self.execute_pending_kind = Some(kind);
                self.execute_phase = ExecutePhase::Confirm;
            }
            StatementKind::Write => {
                self.execute_pending_kind = Some(kind);
                self.dispatch_execute();
            }
        }
    }

    pub fn execute_confirm_yes(&mut self) {
        if self.execute_phase != ExecutePhase::Confirm {
            return;
        }
        self.dispatch_execute();
    }

    pub fn execute_confirm_no(&mut self) {
        if self.execute_phase != ExecutePhase::Confirm {
            return;
        }
        self.execute_phase = ExecutePhase::EditSql;
        self.execute_pending_kind = None;
        self.execute_input_mode = true;
    }

    fn dispatch_execute(&mut self) {
        let Some(database) = self.execute_database.clone() else {
            self.error_message = Some("No database selected.".to_string());
            return;
        };
        let sql = self.execute_sql_buffer.trim().to_string();
        if sql.is_empty() {
            self.error_message = Some("SQL is empty.".to_string());
            return;
        }
        self.execute_phase = ExecutePhase::Result;
        self.execute_result = None;
        self.is_loading = true;
        self.pending_action = Some(AppAction::ExecuteStatement { database, sql });
    }

    pub fn on_execute_completed(&mut self, result: ExecuteResult) {
        self.execute_result = Some(result);
        self.execute_phase = ExecutePhase::Result;
        self.is_loading = false;
        self.execute_pending_kind = None;
    }

    pub fn on_execute_failed(&mut self, message: String) {
        self.error_message = Some(message);
        self.execute_phase = ExecutePhase::EditSql;
        self.execute_input_mode = true;
        self.execute_pending_kind = None;
        self.is_loading = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_connections(names: &[&str]) -> AppState {
        let mut app = AppState::new(true, "test".to_string(), None);
        app.update_connection_state(names.iter().map(|s| s.to_string()).collect());
        // Activate via menu so requires_connection logic still applies.
        let idx = app
            .menu_items
            .iter()
            .position(|m| matches!(m, MenuItem::Execute))
            .expect("Execute menu item exists");
        app.selected = idx;
        app.activate_selected();
        app
    }

    #[test]
    fn one_connected_database_skips_picker() {
        let app = app_with_connections(&["only"]);
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
        assert_eq!(app.execute_database.as_deref(), Some("only"));
        assert!(app.execute_input_mode, "input mode should be on");
    }

    #[test]
    fn multiple_connected_databases_enter_picker() {
        let app = app_with_connections(&["alpha", "beta"]);
        assert_eq!(app.execute_phase, ExecutePhase::PickDatabase);
        assert_eq!(app.execute_database, None);
        assert!(!app.execute_input_mode);
    }

    #[test]
    fn picker_confirm_sets_database_and_advances() {
        let mut app = app_with_connections(&["alpha", "beta"]);
        app.execute_picker_down();
        app.execute_picker_confirm();
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
        assert_eq!(app.execute_database.as_deref(), Some("beta"));
        assert!(app.execute_input_mode);
    }

    #[test]
    fn destructive_statement_enters_confirm_phase() {
        let mut app = app_with_connections(&["only"]);
        app.execute_sql_buffer = "DELETE FROM t".to_string();
        app.execute_submit_sql();
        assert_eq!(app.execute_phase, ExecutePhase::Confirm);
        assert_eq!(app.execute_pending_kind, Some(StatementKind::Destructive));
        assert!(app.pending_action.is_none(), "must wait for confirmation");
    }

    #[test]
    fn write_statement_dispatches_immediately() {
        let mut app = app_with_connections(&["only"]);
        app.execute_sql_buffer = "INSERT INTO t VALUES (1)".to_string();
        app.execute_submit_sql();
        assert_eq!(app.execute_phase, ExecutePhase::Result);
        assert!(app.is_loading);
        match app.take_action() {
            Some(AppAction::ExecuteStatement { database, sql }) => {
                assert_eq!(database, "only");
                assert_eq!(sql, "INSERT INTO t VALUES (1)");
            }
            other => panic!("expected ExecuteStatement, got {other:?}"),
        }
    }

    #[test]
    fn read_statement_in_exec_is_rejected_with_error() {
        let mut app = app_with_connections(&["only"]);
        app.execute_sql_buffer = "SELECT 1".to_string();
        app.execute_submit_sql();
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
        assert!(app.execute_input_mode);
        assert!(
            app.error_message
                .as_deref()
                .unwrap_or("")
                .contains("Read-only"),
            "expected read-only guidance, got: {:?}",
            app.error_message
        );
        assert!(app.pending_action.is_none());
    }

    #[test]
    fn unsupported_statement_in_exec_is_rejected_with_error() {
        let mut app = app_with_connections(&["only"]);
        app.execute_sql_buffer = "WITH x AS (SELECT 1) SELECT * FROM x".to_string();
        app.execute_submit_sql();
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
        assert!(app.execute_input_mode);
        assert!(app.error_message.is_some());
        assert!(app.pending_action.is_none());
    }

    #[test]
    fn confirm_no_returns_to_editor_without_dispatch() {
        let mut app = app_with_connections(&["only"]);
        app.execute_sql_buffer = "DROP TABLE t".to_string();
        app.execute_submit_sql();
        assert_eq!(app.execute_phase, ExecutePhase::Confirm);
        app.execute_confirm_no();
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
        assert_eq!(app.execute_pending_kind, None);
        assert!(app.execute_input_mode);
        assert!(app.pending_action.is_none());
        // Buffer is preserved so the user can edit instead of retyping.
        assert_eq!(app.execute_sql_buffer, "DROP TABLE t");
    }

    #[test]
    fn confirm_yes_dispatches_destructive_action() {
        let mut app = app_with_connections(&["only"]);
        app.execute_sql_buffer = "DROP TABLE t".to_string();
        app.execute_submit_sql();
        assert_eq!(app.execute_phase, ExecutePhase::Confirm);
        app.execute_confirm_yes();
        assert_eq!(app.execute_phase, ExecutePhase::Result);
        assert!(app.is_loading);
        match app.take_action() {
            Some(AppAction::ExecuteStatement { database, sql }) => {
                assert_eq!(database, "only");
                assert_eq!(sql, "DROP TABLE t");
            }
            other => panic!("expected ExecuteStatement, got {other:?}"),
        }
    }

    #[test]
    fn execute_completion_populates_result_and_clears_loading() {
        use databasecli_core::commands::execute::ExecuteResult;
        use std::time::Duration;

        let mut app = app_with_connections(&["only"]);
        app.is_loading = true;
        app.on_execute_completed(ExecuteResult {
            database_name: "only".to_string(),
            command_tag: "DELETE 3".to_string(),
            affected_rows: Some(3),
            columns: Vec::new(),
            rows: Vec::new(),
            execution_time: Duration::from_millis(1),
        });
        assert_eq!(app.execute_phase, ExecutePhase::Result);
        assert!(!app.is_loading);
        assert!(app.execute_result.is_some());
    }

    #[test]
    fn execute_failure_returns_user_to_editor() {
        let mut app = app_with_connections(&["only"]);
        app.is_loading = true;
        app.on_execute_failed("connection refused".to_string());
        assert_eq!(app.execute_phase, ExecutePhase::EditSql);
        assert!(app.execute_input_mode);
        assert!(!app.is_loading);
        assert_eq!(app.error_message.as_deref(), Some("connection refused"));
    }
}
