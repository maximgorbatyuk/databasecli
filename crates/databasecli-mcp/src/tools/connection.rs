use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;

use crate::json_convert::config_to_json;
use crate::state::McpSessionState;

pub fn list_configured(state: &McpSessionState) -> Result<CallToolResult, McpError> {
    let configs: Vec<serde_json::Value> = state.configs.iter().map(config_to_json).collect();
    let text = serde_json::to_string_pretty(&configs).unwrap_or_else(|_| "[]".to_string());
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

pub async fn connect(
    state: &McpSessionState,
    names: Vec<String>,
) -> Result<CallToolResult, McpError> {
    let configs_to_connect: Vec<_> = names
        .iter()
        .filter_map(|n| state.find_config(n).cloned())
        .collect();
    let missing: Vec<String> = names
        .iter()
        .filter(|n| state.find_config(n).is_none())
        .cloned()
        .collect();

    let (connected, mut errors) = state
        .with_manager(move |mgr| {
            let mut connected = Vec::new();
            let mut errors: Vec<serde_json::Value> = Vec::new();
            for config in &configs_to_connect {
                match mgr.connect(config) {
                    Ok(()) => connected.push(config.name.clone()),
                    Err(e) => errors.push(json!({"name": config.name, "error": e.to_string()})),
                }
            }
            (connected, errors)
        })
        .await?;

    for name in &missing {
        errors.push(json!({"name": name, "error": "not found in config"}));
    }

    let response = json!({ "connected": connected, "errors": errors });
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

pub async fn disconnect(
    state: &McpSessionState,
    names: Vec<String>,
) -> Result<CallToolResult, McpError> {
    let names_clone = names.clone();
    let (disconnected, still_connected) = state
        .with_manager(move |mgr| {
            let was_connected = mgr.connected_names();
            if names_clone.is_empty() {
                mgr.disconnect_all();
                (was_connected, Vec::new())
            } else {
                for name in &names_clone {
                    let _ = mgr.disconnect(name);
                }
                let remaining = mgr.connected_names();
                (names_clone, remaining)
            }
        })
        .await?;

    let response = json!({ "disconnected": disconnected, "still_connected": still_connected });
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&response).unwrap_or_default(),
    )]))
}

pub async fn list_connected(state: &McpSessionState) -> Result<CallToolResult, McpError> {
    let result = state
        .with_manager(databasecli_core::commands::list_databases::list_connected)
        .await?;

    let databases: Vec<serde_json::Value> = result
        .iter()
        .map(|d| {
            json!({
                "name": d.name,
                "host": d.host,
                "port": d.port,
                "dbname": d.dbname,
                "user": d.user,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&databases).unwrap_or_default(),
    )]))
}
