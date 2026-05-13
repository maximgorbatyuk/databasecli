use std::collections::HashMap;

use crate::config::DatabaseConfig;
use crate::error::DatabaseCliError;

/// A live database connection plus the config that opened it.
///
/// The `client` field is `pub(crate)` so command modules inside
/// `databasecli-core` can call it directly, while crates that depend on the
/// core (`databasecli-mcp`, `databasecli-tui`, `databasecli-cli`) cannot.
/// This is defense-in-depth: even though `default_transaction_read_only = on`
/// blocks writes server-side for `ConnectionManager`-owned connections,
/// keeping the raw `postgres::Client` out of MCP source means an AI agent
/// can't trick MCP into calling `client.execute(...)` with an arbitrary
/// payload. Combined with the `mcp_does_not_reference_writable_helpers`
/// guard test, the only legitimate write path is the local `exec` flow,
/// which goes through `execute_normalized` / `execute_script`.
pub struct LiveConnection {
    pub config: DatabaseConfig,
    pub(crate) client: postgres::Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionMode {
    ReadOnly,
    LocalExec,
}

fn open_client(
    config: &DatabaseConfig,
    mode: ConnectionMode,
) -> Result<postgres::Client, DatabaseCliError> {
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| DatabaseCliError::ConnectionFailed(format!("TLS error: {e}")))?;
    let connector = postgres_native_tls::MakeTlsConnector::new(connector);

    let mut client = postgres::Client::connect(&config.connection_string(), connector)
        .map_err(|e| DatabaseCliError::ConnectionFailed(e.to_string()))?;

    let setup = match mode {
        ConnectionMode::ReadOnly => {
            "SET default_transaction_read_only = on; SET statement_timeout = '30s'"
        }
        ConnectionMode::LocalExec => "SET statement_timeout = '30s'",
    };

    client
        .batch_execute(setup)
        .map_err(|e| DatabaseCliError::QueryFailed(e.to_string()))?;

    Ok(client)
}

/// Open a fresh writable connection for a single local CLI/TUI execution.
///
/// Sets `statement_timeout = '30s'` but does NOT set
/// `default_transaction_read_only`. This connection is intended for one-shot
/// `exec` use by the local operator and must NEVER be reachable from the
/// MCP surface.
pub fn connect_for_local_exec(config: &DatabaseConfig) -> Result<LiveConnection, DatabaseCliError> {
    let client = open_client(config, ConnectionMode::LocalExec)?;
    Ok(LiveConnection {
        config: config.clone(),
        client,
    })
}

pub struct ConnectionManager {
    connections: HashMap<String, LiveConnection>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn connect(&mut self, config: &DatabaseConfig) -> Result<(), DatabaseCliError> {
        if self.connections.contains_key(&config.name) {
            return Err(DatabaseCliError::AlreadyConnected(config.name.clone()));
        }

        let client = open_client(config, ConnectionMode::ReadOnly)?;

        self.connections.insert(
            config.name.clone(),
            LiveConnection {
                config: config.clone(),
                client,
            },
        );
        Ok(())
    }

    pub fn disconnect(&mut self, name: &str) -> Result<(), DatabaseCliError> {
        self.connections
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| DatabaseCliError::NotConnected(name.to_string()))
    }

    pub fn disconnect_all(&mut self) {
        self.connections.clear();
    }

    pub fn connected_names(&self) -> Vec<String> {
        self.connections.keys().cloned().collect()
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut LiveConnection> {
        self.connections.get_mut(name)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut LiveConnection)> {
        self.connections.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
