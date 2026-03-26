use crate::commands::query::{
    QueryResultSet, execute_query, format_query_result, validate_readonly,
};
use crate::connection::ConnectionManager;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct CompareResult {
    pub query: String,
    pub results: Vec<QueryResultSet>,
    pub errors: Vec<(String, String)>,
}

pub fn compare_query(
    manager: &mut ConnectionManager,
    sql: &str,
) -> Result<CompareResult, DatabaseCliError> {
    validate_readonly(sql)?;

    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (name, conn) in manager.iter_mut() {
        match execute_query(conn, sql) {
            Ok(r) => results.push(r),
            Err(e) => errors.push((name.clone(), e.to_string())),
        }
    }

    Ok(CompareResult {
        query: sql.to_string(),
        results,
        errors,
    })
}

pub fn format_compare_result(result: &CompareResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("Query: {}\n\n", result.query));

    for qr in &result.results {
        out.push_str(&format!("--- {} ---\n", qr.database_name));
        out.push_str(&format_query_result(qr));
        out.push('\n');
    }

    for (name, err) in &result.errors {
        out.push_str(&format!("--- {} ---\n", name));
        out.push_str(&format!("  Error: {err}\n\n"));
    }

    out
}
