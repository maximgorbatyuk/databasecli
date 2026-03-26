use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;

use databasecli_core::commands::erd::build_erd;
use databasecli_core::commands::sample::sample_table;
use databasecli_core::commands::schema::dump_schema;
use databasecli_core::error::DatabaseCliError;

use crate::json_convert::{erd_result_to_json, sample_result_to_json, schema_result_to_json};
use crate::state::McpSessionState;

pub async fn schema(
    state: &McpSessionState,
    schema_name: Option<String>,
    database: Option<String>,
) -> Result<CallToolResult, McpError> {
    let schema_filter = schema_name.unwrap_or_else(|| "public".to_string());

    // Collect per-database results and errors instead of aborting on first failure
    let result = state
        .with_manager(move |mgr| {
            let mut results = Vec::new();
            let mut errors: Vec<serde_json::Value> = Vec::new();

            if let Some(ref db_name) = database {
                let conn = mgr
                    .get_mut(db_name)
                    .ok_or_else(|| DatabaseCliError::NotConnected(db_name.clone()));
                match conn {
                    Ok(c) => match dump_schema(c, Some(&schema_filter)) {
                        Ok(r) => results.push(schema_result_to_json(&r)),
                        Err(e) => errors.push(json!({"database": db_name, "error": e.to_string()})),
                    },
                    Err(e) => errors.push(json!({"database": db_name, "error": e.to_string()})),
                }
            } else {
                for (name, conn) in mgr.iter_mut() {
                    match dump_schema(conn, Some(&schema_filter)) {
                        Ok(r) => results.push(schema_result_to_json(&r)),
                        Err(e) => errors.push(json!({"database": name, "error": e.to_string()})),
                    }
                }
            }

            (results, errors)
        })
        .await?;

    let (results, errors) = result;
    let response = json!({ "schemas": results, "errors": errors });
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

pub async fn erd(
    state: &McpSessionState,
    database: String,
    schema_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let schema_filter = schema_name.unwrap_or_else(|| "public".to_string());

    let result = state
        .with_manager(move |mgr| {
            let conn = mgr
                .get_mut(&database)
                .ok_or_else(|| DatabaseCliError::NotConnected(database.clone()))?;
            build_erd(conn, Some(&schema_filter))
        })
        .await?;

    match result {
        Ok(erd_result) => Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&erd_result_to_json(&erd_result)).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "Error: {e}"
        ))])),
    }
}

pub async fn sample(
    state: &McpSessionState,
    table: String,
    database: String,
    schema_name: Option<String>,
    limit: Option<i64>,
    order_by: Option<String>,
) -> Result<CallToolResult, McpError> {
    let schema_filter = schema_name.unwrap_or_else(|| "public".to_string());

    let result = state
        .with_manager(move |mgr| {
            let conn = mgr
                .get_mut(&database)
                .ok_or_else(|| DatabaseCliError::NotConnected(database.clone()))?;
            sample_table(
                conn,
                &table,
                Some(&schema_filter),
                limit.or(Some(20)),
                order_by.as_deref(),
            )
        })
        .await?;

    match result {
        Ok(sr) => Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&sample_result_to_json(&sr)).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "Error: {e}"
        ))])),
    }
}
