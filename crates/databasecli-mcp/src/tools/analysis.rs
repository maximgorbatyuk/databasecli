use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;

use databasecli_core::commands::analyze::analyze_table;
use databasecli_core::commands::health::check_all_enhanced_health;
use databasecli_core::commands::summary::summarize;
use databasecli_core::commands::trend::{TrendInterval, TrendParams, compute_trend};
use databasecli_core::error::DatabaseCliError;

use crate::json_convert::{
    enhanced_health_to_json, summary_to_json, table_profile_to_json, trend_result_to_json,
};
use crate::state::McpSessionState;

pub async fn analyze(
    state: &McpSessionState,
    table: String,
    database: String,
    schema_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let schema_filter = schema_name.unwrap_or_else(|| "public".to_string());

    let result = state
        .with_manager(move |mgr| {
            let conn = mgr
                .get_mut(&database)
                .ok_or_else(|| DatabaseCliError::NotConnected(database.clone()))?;
            analyze_table(conn, &table, Some(&schema_filter))
        })
        .await?;

    match result {
        Ok(profile) => Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&table_profile_to_json(&profile)).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "Error: {e}"
        ))])),
    }
}

pub async fn summary(
    state: &McpSessionState,
    database: Option<String>,
) -> Result<CallToolResult, McpError> {
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
                    Ok(c) => match summarize(c) {
                        Ok(r) => results.push(summary_to_json(&r)),
                        Err(e) => errors.push(json!({"database": db_name, "error": e.to_string()})),
                    },
                    Err(e) => errors.push(json!({"database": db_name, "error": e.to_string()})),
                }
            } else {
                for (name, conn) in mgr.iter_mut() {
                    match summarize(conn) {
                        Ok(r) => results.push(summary_to_json(&r)),
                        Err(e) => errors.push(json!({"database": name, "error": e.to_string()})),
                    }
                }
            }

            (results, errors)
        })
        .await?;

    let (results, errors) = result;
    let response = json!({ "summaries": results, "errors": errors });
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

pub struct TrendToolParams {
    pub table: String,
    pub database: String,
    pub timestamp_column: String,
    pub interval: Option<String>,
    pub value_column: Option<String>,
    pub schema_name: Option<String>,
    pub limit: Option<i64>,
}

pub async fn trend(
    state: &McpSessionState,
    params: TrendToolParams,
) -> Result<CallToolResult, McpError> {
    let TrendToolParams {
        table,
        database,
        timestamp_column,
        interval,
        value_column,
        schema_name,
        limit,
    } = params;
    let interval = TrendInterval::parse_interval(interval.as_deref().unwrap_or("day"))
        .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

    let trend_params = TrendParams {
        table,
        schema: schema_name.unwrap_or_else(|| "public".to_string()),
        timestamp_column,
        interval,
        value_column,
        limit,
    };

    let result = state
        .with_manager(move |mgr| {
            let conn = mgr
                .get_mut(&database)
                .ok_or_else(|| DatabaseCliError::NotConnected(database.clone()))?;
            compute_trend(conn, &trend_params)
        })
        .await?;

    match result {
        Ok(tr) => Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&trend_result_to_json(&tr)).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
            "Error: {e}"
        ))])),
    }
}

pub async fn enhanced_health(state: &McpSessionState) -> Result<CallToolResult, McpError> {
    let result = state.with_manager(check_all_enhanced_health).await?;

    let json: Vec<serde_json::Value> = result.iter().map(enhanced_health_to_json).collect();
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )]))
}
