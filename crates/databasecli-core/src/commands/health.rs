use std::time::{Duration, Instant};

use crate::connection::{ConnectionManager, LiveConnection};
use crate::health::HealthStatus;

#[derive(Debug, Clone)]
pub struct EnhancedHealthResult {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub status: HealthStatus,
    pub response_time: Option<Duration>,
    pub pg_version: Option<String>,
    pub db_size: Option<String>,
    pub uptime: Option<String>,
    pub error: Option<String>,
}

pub fn check_enhanced_health(conn: &mut LiveConnection) -> EnhancedHealthResult {
    let start = Instant::now();

    let mut result = EnhancedHealthResult {
        name: conn.config.name.clone(),
        host: conn.config.host.clone(),
        port: conn.config.port,
        dbname: conn.config.dbname.clone(),
        status: HealthStatus::Connected,
        response_time: None,
        pg_version: None,
        db_size: None,
        uptime: None,
        error: None,
    };

    // Test basic connectivity
    match conn.client.simple_query("SELECT 1") {
        Ok(_) => {}
        Err(e) => {
            result.status = HealthStatus::Failed;
            result.response_time = Some(start.elapsed());
            result.error = Some(e.to_string());
            return result;
        }
    }

    // PostgreSQL version
    if let Ok(rows) = conn.client.query("SELECT version()", &[])
        && let Some(row) = rows.first()
    {
        result.pg_version = row.get::<_, Option<String>>(0);
    }

    // Database size
    if let Ok(rows) = conn.client.query(
        "SELECT pg_size_pretty(pg_database_size(current_database()))",
        &[],
    ) && let Some(row) = rows.first()
    {
        result.db_size = row.get::<_, Option<String>>(0);
    }

    // Uptime
    if let Ok(rows) = conn
        .client
        .query("SELECT (now() - pg_postmaster_start_time())::text", &[])
        && let Some(row) = rows.first()
    {
        result.uptime = row.get::<_, Option<String>>(0);
    }

    result.response_time = Some(start.elapsed());
    result
}

pub fn check_all_enhanced_health(manager: &mut ConnectionManager) -> Vec<EnhancedHealthResult> {
    let mut results = Vec::new();
    for (_, conn) in manager.iter_mut() {
        results.push(check_enhanced_health(conn));
    }
    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

pub fn format_enhanced_health_table(results: &[EnhancedHealthResult]) -> String {
    if results.is_empty() {
        return "No active connections.\n".to_string();
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
    let size_w = results
        .iter()
        .map(|r| r.db_size.as_ref().map_or(1, |s| s.len()))
        .max()
        .unwrap_or(4)
        .max(4);

    let mut out = String::new();
    out.push_str(&format!(
        "{:<name_w$}  {:<host_w$}  {:>9}  {:<size_w$}  {}\n",
        "Name", "Host", "Time", "Size", "Status",
    ));
    out.push_str(&format!(
        "{:-<name_w$}  {:-<host_w$}  {:->9}  {:-<size_w$}  {:-<10}\n",
        "", "", "", "", "",
    ));

    for r in results {
        let time = match r.response_time {
            Some(d) => format!("{:.0?}", d),
            None => "-".to_string(),
        };
        let size = r.db_size.as_deref().unwrap_or("-");
        let status = match r.status {
            HealthStatus::Connected => "Connected",
            HealthStatus::Failed => "Failed",
        };
        out.push_str(&format!(
            "{:<name_w$}  {:<host_w$}  {:>9}  {:<size_w$}  {}\n",
            r.name,
            format!("{}:{}", r.host, r.port),
            time,
            size,
            status,
        ));
    }

    out
}
