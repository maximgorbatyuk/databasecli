use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;

use databasecli_core::commands::erd::build_erd;
use databasecli_core::commands::schema::dump_schema;
use databasecli_core::error::DatabaseCliError;

use crate::json_convert::{erd_result_to_json, schema_result_to_json};
use crate::state::McpSessionState;

pub async fn suggest_migration(
    state: &McpSessionState,
    database: String,
    description: String,
    schema_name: Option<String>,
) -> Result<CallToolResult, McpError> {
    let schema_filter = schema_name.unwrap_or_else(|| "public".to_string());

    let result = state
        .with_manager(move |mgr| {
            let conn = mgr
                .get_mut(&database)
                .ok_or_else(|| DatabaseCliError::NotConnected(database.clone()))?;
            let schema = dump_schema(conn, Some(&schema_filter))?;
            let erd = build_erd(conn, Some(&schema_filter))?;
            Ok::<_, DatabaseCliError>((schema, erd))
        })
        .await?;

    match result {
        Ok((schema, erd)) => {
            let response = json!({
                "current_schema": schema_result_to_json(&schema),
                "foreign_keys": erd_result_to_json(&erd)["foreign_keys"],
                "description": description,
                "note": "This is an analysis tool. Use the schema and foreign key information above to generate migration SQL. The SQL has NOT been executed and CANNOT be executed through this tool. Present the migration plan to the user for manual review."
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
