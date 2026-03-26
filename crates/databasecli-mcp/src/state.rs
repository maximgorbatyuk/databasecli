use std::sync::{Arc, Mutex};

use rmcp::ErrorData as McpError;

use databasecli_core::config::{DatabaseConfig, load_databases, resolve_config_path_with_base};
use databasecli_core::connection::ConnectionManager;

pub struct McpSessionState {
    pub manager: Arc<Mutex<ConnectionManager>>,
    pub configs: Vec<DatabaseConfig>,
}

impl McpSessionState {
    pub fn new(directory: Option<&str>) -> anyhow::Result<Self> {
        let path = resolve_config_path_with_base(directory)?;
        let configs = match load_databases(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "Failed to load database configs from {}: {e}",
                    path.display()
                );
                Vec::new()
            }
        };
        Ok(Self {
            manager: Arc::new(Mutex::new(ConnectionManager::new())),
            configs,
        })
    }

    pub fn find_config(&self, name: &str) -> Option<&DatabaseConfig> {
        self.configs.iter().find(|c| c.name == name)
    }

    /// Run a blocking closure with mutable access to the ConnectionManager.
    /// Handles mutex poisoning and spawn_blocking errors.
    pub async fn with_manager<F, R>(&self, f: F) -> Result<R, McpError>
    where
        F: FnOnce(&mut ConnectionManager) -> R + Send + 'static,
        R: Send + 'static,
    {
        let manager = Arc::clone(&self.manager);
        tokio::task::spawn_blocking(move || {
            let mut mgr = manager
                .lock()
                .map_err(|_| McpError::internal_error("Connection state corrupted", None))?;
            Ok(f(&mut mgr))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join: {e}"), None))?
    }
}
