use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;

use databasecli_core::commands::compare::compare_query;
use databasecli_core::commands::query::execute_query;
use databasecli_core::error::DatabaseCliError;

use crate::json_convert::query_result_to_json;
use crate::state::McpSessionState;

pub async fn query(
    state: &McpSessionState,
    sql: String,
    database: Option<String>,
) -> Result<CallToolResult, McpError> {
    let result = state
        .with_manager(move |mgr| {
            let conn = match database {
                Some(ref name) => mgr
                    .get_mut(name)
                    .ok_or_else(|| DatabaseCliError::NotConnected(name.clone())),
                None => mgr
                    .iter_mut()
                    .next()
                    .map(|(_, conn)| conn)
                    .ok_or(DatabaseCliError::NoConnections),
            }?;
            execute_query(conn, &sql)
        })
        .await?;

    match result {
        Ok(qr) => Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&query_result_to_json(&qr)).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "Error: {e}"
        ))])),
    }
}

pub async fn compare(state: &McpSessionState, sql: String) -> Result<CallToolResult, McpError> {
    let result = state
        .with_manager(move |mgr| compare_query(mgr, &sql))
        .await?;

    match result {
        Ok(cr) => {
            let results: Vec<serde_json::Value> =
                cr.results.iter().map(query_result_to_json).collect();
            let errors: Vec<serde_json::Value> = cr
                .errors
                .iter()
                .map(|(name, err)| json!({"database": name, "error": err}))
                .collect();
            let response = json!({
                "query": cr.query,
                "results": results,
                "errors": errors,
            });
            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&response).unwrap_or_default(),
            )]))
        }
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "Error: {e}"
        ))])),
    }
}
