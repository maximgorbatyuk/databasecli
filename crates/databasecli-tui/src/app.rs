use std::fmt;

use databasecli_core::commands::analyze::TableProfile;
use databasecli_core::commands::compare::CompareResult;
use databasecli_core::commands::erd::ErdResult;
use databasecli_core::commands::query::QueryResultSet;
use databasecli_core::commands::sample::SampleResult;
use databasecli_core::commands::schema::SchemaResult;
use databasecli_core::commands::summary::DatabaseSummary;
use databasecli_core::commands::trend::TrendResult;
use databasecli_core::config::DatabaseConfig;
use databasecli_core::health::HealthResult;

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
    Sample,
    Analyze,
    Summary,
    Erd,
    Compare,
    Trend,
    Help,
}

impl MenuItem {
    pub fn description(&self) -> &'static str {
        match self {
            MenuItem::CreateConfig => "Create the databases.ini config file",
            MenuItem::Init => "Create config and .mcp.json for AI agents",
            MenuItem::Connect => "Select databases to connect to",
            MenuItem::StoredDatabases => "View all configured database connections",
            MenuItem::DatabaseHealth => "Check connectivity for all databases",
            MenuItem::Schema => "Full schema: tables, columns, types, PKs",
            MenuItem::Query => "Run read-only SQL query",
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
    RunInit,
    LoadDatabases,
    CheckHealth,
    ConnectDatabases(Vec<DatabaseConfig>),
    DisconnectDatabases(Vec<String>),
    RunSchema,
    RunQuery(String),
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
    pub mcp_path: String,
    pub current_dir: String,
    pub directory: Option<String>,

    // Connection state
    pub connected_count: usize,
    pub connected_names: Vec<String>,
    pub connect_cursor: usize,
    pub connect_selection: Vec<bool>,

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

    pending_action: Option<AppAction>,
}

impl AppState {
    pub fn new(
        config_exists: bool,
        config_path: String,
        mcp_path: String,
        directory: Option<String>,
    ) -> Self {
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
            mcp_path,
            current_dir,
            directory,
            connected_count: 0,
            connected_names: Vec::new(),
            connect_cursor: 0,
            connect_selection: Vec::new(),
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
            Screen::CreateConfig | Screen::Init => {
                // Just navigate to confirmation screen
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
        self.pending_action = Some(AppAction::RunInit);
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
}
