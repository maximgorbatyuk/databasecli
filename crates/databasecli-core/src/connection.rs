use std::collections::HashMap;

use crate::config::DatabaseConfig;
use crate::error::DatabaseCliError;

pub struct LiveConnection {
    pub config: DatabaseConfig,
    pub client: postgres::Client,
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

        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| DatabaseCliError::ConnectionFailed(format!("TLS error: {e}")))?;
        let connector = postgres_native_tls::MakeTlsConnector::new(connector);

        let mut client = postgres::Client::connect(&config.connection_string(), connector)
            .map_err(|e| DatabaseCliError::ConnectionFailed(e.to_string()))?;

        client
            .batch_execute("SET default_transaction_read_only = on; SET statement_timeout = '30s'")
            .map_err(|e| DatabaseCliError::QueryFailed(e.to_string()))?;

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
