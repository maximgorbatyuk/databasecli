use std::fmt;

use databasecli_core::config::DatabaseConfig;
use databasecli_core::health::HealthResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Home,
    CreateConfig,
    StoredDatabases,
    DatabaseHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    CreateConfig,
    StoredDatabases,
    DatabaseHealth,
}

impl MenuItem {
    pub fn description(&self) -> &'static str {
        match self {
            MenuItem::CreateConfig => "Create the databases.ini config file",
            MenuItem::StoredDatabases => "View all configured database connections",
            MenuItem::DatabaseHealth => "Check connectivity for all databases",
        }
    }

    pub fn screen(&self) -> Screen {
        match self {
            MenuItem::CreateConfig => Screen::CreateConfig,
            MenuItem::StoredDatabases => Screen::StoredDatabases,
            MenuItem::DatabaseHealth => Screen::DatabaseHealth,
        }
    }
}

impl fmt::Display for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItem::CreateConfig => write!(f, "Create database.ini"),
            MenuItem::StoredDatabases => write!(f, "Stored Databases"),
            MenuItem::DatabaseHealth => write!(f, "Database Health"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    CreateConfig,
    LoadDatabases,
    CheckHealth,
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
    pending_action: Option<AppAction>,
}

impl AppState {
    pub fn new(config_exists: bool, config_path: String) -> Self {
        let mut menu_items = Vec::new();
        if !config_exists {
            menu_items.push(MenuItem::CreateConfig);
        }
        menu_items.push(MenuItem::StoredDatabases);
        menu_items.push(MenuItem::DatabaseHealth);

        let current_dir = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".to_string());

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
        let screen = item.screen();

        match screen {
            Screen::CreateConfig => {
                // Just navigate to confirmation screen; action fires on Enter there
            }
            Screen::StoredDatabases => {
                self.pending_action = Some(AppAction::LoadDatabases);
            }
            Screen::DatabaseHealth => {
                self.pending_action = Some(AppAction::CheckHealth);
                self.is_loading = true;
            }
            _ => {}
        }

        self.active_screen = screen;
        self.scroll_offset = 0;
    }

    pub fn confirm_create_config(&mut self) {
        self.pending_action = Some(AppAction::CreateConfig);
    }

    pub fn go_home(&mut self) {
        self.active_screen = Screen::Home;
        self.scroll_offset = 0;
        self.error_message = None;
        self.status_message = None;
        self.is_loading = false;
        self.spinner_frame = 0;
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

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn take_action(&mut self) -> Option<AppAction> {
        self.pending_action.take()
    }
}
