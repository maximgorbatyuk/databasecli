use std::time::{Duration, Instant};

use crate::config::DatabaseConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Connected,
    Failed,
}

#[derive(Debug, Clone)]
pub struct HealthResult {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub status: HealthStatus,
    pub response_time: Option<Duration>,
    pub error: Option<String>,
}

pub fn check_health(config: &DatabaseConfig) -> HealthResult {
    let start = Instant::now();

    let connector = match native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return HealthResult {
                name: config.name.clone(),
                host: config.host.clone(),
                port: config.port,
                dbname: config.dbname.clone(),
                status: HealthStatus::Failed,
                response_time: Some(start.elapsed()),
                error: Some(format!("TLS error: {e}")),
            };
        }
    };

    let connector = postgres_native_tls::MakeTlsConnector::new(connector);
    let conn_str = config.connection_string();

    match postgres::Client::connect(&conn_str, connector) {
        Ok(mut client) => match client.simple_query("SELECT 1") {
            Ok(_) => HealthResult {
                name: config.name.clone(),
                host: config.host.clone(),
                port: config.port,
                dbname: config.dbname.clone(),
                status: HealthStatus::Connected,
                response_time: Some(start.elapsed()),
                error: None,
            },
            Err(e) => HealthResult {
                name: config.name.clone(),
                host: config.host.clone(),
                port: config.port,
                dbname: config.dbname.clone(),
                status: HealthStatus::Failed,
                response_time: Some(start.elapsed()),
                error: Some(e.to_string()),
            },
        },
        Err(e) => HealthResult {
            name: config.name.clone(),
            host: config.host.clone(),
            port: config.port,
            dbname: config.dbname.clone(),
            status: HealthStatus::Failed,
            response_time: Some(start.elapsed()),
            error: Some(e.to_string()),
        },
    }
}

pub fn check_all_health(configs: &[DatabaseConfig]) -> Vec<HealthResult> {
    configs.iter().map(check_health).collect()
}

pub fn format_health_table(results: &[HealthResult]) -> String {
    if results.is_empty() {
        return "No databases configured.".to_string();
    }

    let name_w = results
        .iter()
        .map(|r| r.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let host_w = results
        .iter()
        .map(|r| format!("{}:{}", r.host, r.port).len())
        .max()
        .unwrap_or(4)
        .max(4);
    let db_w = results
        .iter()
        .map(|r| r.dbname.len())
        .max()
        .unwrap_or(8)
        .max(8);

    let mut out = String::new();
    out.push_str(&format!(
        "{:<name_w$}  {:<host_w$}  {:<db_w$}  {:>9}  {}\n",
        "Name", "Host", "Database", "Time", "Status",
    ));
    let dash_line = format!(
        "{:-<name_w$}  {:-<host_w$}  {:-<db_w$}  {:->9}  {:-<10}\n",
        "", "", "", "", "",
    );
    out.push_str(&dash_line);

    for r in results {
        let host = format!("{}:{}", r.host, r.port);
        let time = match r.response_time {
            Some(d) => format!("{:.0?}", d),
            None => "-".to_string(),
        };
        let status = match r.status {
            HealthStatus::Connected => "Connected",
            HealthStatus::Failed => "Failed",
        };
        out.push_str(&format!(
            "{:<name_w$}  {:<host_w$}  {:<db_w$}  {:>9}  {}\n",
            r.name, host, r.dbname, time, status,
        ));
    }

    out
}
