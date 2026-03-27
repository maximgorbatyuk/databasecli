use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::{create_default_config, resolve_base_dir, resolve_config_path_with_base};
use crate::error::DatabaseCliError;

/// What happened to a file during init.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    /// New file was created.
    Created,
    /// Existing file was modified.
    Updated,
    /// No changes were needed.
    Unchanged,
}

/// Result of running init -- reports what was done.
pub struct InitResult {
    pub config_path: PathBuf,
    pub config_action: FileAction,
    pub mcp_path: PathBuf,
    pub mcp_action: FileAction,
}

/// Run the full init: create databases.ini if missing, create/update .mcp.json.
pub fn run_init(directory: Option<&str>) -> Result<InitResult, DatabaseCliError> {
    let config_path = resolve_config_path_with_base(directory)?;
    let config_action = if config_path.exists() {
        FileAction::Unchanged
    } else {
        create_default_config(&config_path)?;
        FileAction::Created
    };

    let base_dir = resolve_base_dir(directory)?;
    let mcp_path = base_dir.join(".mcp.json");
    let mcp_action = upsert_mcp_json(&mcp_path)?;

    Ok(InitResult {
        config_path,
        config_action,
        mcp_path,
        mcp_action,
    })
}

fn upsert_mcp_json(path: &Path) -> Result<FileAction, DatabaseCliError> {
    let mcp_entry = serde_json::json!({
        "command": "databasecli-mcp",
        "args": ["-D", "."]
    });

    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(DatabaseCliError::Io)?;
        let mut doc: Value = serde_json::from_str(&content)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;

        let servers = doc
            .as_object_mut()
            .ok_or_else(|| DatabaseCliError::ConfigParse("expected JSON object".into()))?
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));

        let map = servers
            .as_object_mut()
            .ok_or_else(|| DatabaseCliError::ConfigParse("mcpServers must be an object".into()))?;

        if map.contains_key("databasecli") {
            return Ok(FileAction::Unchanged);
        }

        map.insert("databasecli".to_string(), mcp_entry);

        let output = serde_json::to_string_pretty(&doc)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;
        std::fs::write(path, output + "\n").map_err(DatabaseCliError::Io)?;
        Ok(FileAction::Updated)
    } else {
        let doc = serde_json::json!({
            "mcpServers": {
                "databasecli": mcp_entry
            }
        });
        let output = serde_json::to_string_pretty(&doc)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;
        std::fs::write(path, output + "\n").map_err(DatabaseCliError::Io)?;
        Ok(FileAction::Created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_new_mcp_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");

        let action = upsert_mcp_json(&path).unwrap();
        assert_eq!(action, FileAction::Created);

        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            content["mcpServers"]["databasecli"]["command"],
            "databasecli-mcp"
        );
        assert_eq!(content["mcpServers"]["databasecli"]["args"][0], "-D");
        assert_eq!(content["mcpServers"]["databasecli"]["args"][1], ".");
    }

    #[test]
    fn adds_entry_to_existing_mcp_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        let existing = r#"{"mcpServers":{"other":{"command":"other-cmd"}}}"#;
        std::fs::write(&path, existing).unwrap();

        let action = upsert_mcp_json(&path).unwrap();
        assert_eq!(action, FileAction::Updated);

        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["mcpServers"]["other"]["command"], "other-cmd");
        assert_eq!(
            content["mcpServers"]["databasecli"]["command"],
            "databasecli-mcp"
        );
    }

    #[test]
    fn skips_if_already_configured() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        let existing =
            r#"{"mcpServers":{"databasecli":{"command":"databasecli-mcp","args":["-D","."]}}}"#;
        std::fs::write(&path, existing).unwrap();

        let action = upsert_mcp_json(&path).unwrap();
        assert_eq!(action, FileAction::Unchanged);
    }

    #[test]
    fn adds_mcp_servers_key_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        std::fs::write(&path, "{}").unwrap();

        let action = upsert_mcp_json(&path).unwrap();
        assert_eq!(action, FileAction::Updated);

        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(content["mcpServers"]["databasecli"].is_object());
    }

    #[test]
    fn errors_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        std::fs::write(&path, "not json").unwrap();

        let err = upsert_mcp_json(&path).unwrap_err();
        assert!(err.to_string().contains("config parse error"));
    }

    #[test]
    fn run_init_creates_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let result = run_init(Some(dir_str)).unwrap();
        assert_eq!(result.config_action, FileAction::Created);
        assert_eq!(result.mcp_action, FileAction::Created);
        assert!(result.config_path.exists());
        assert!(result.mcp_path.exists());
    }

    #[test]
    fn run_init_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let first = run_init(Some(dir_str)).unwrap();
        assert_eq!(first.config_action, FileAction::Created);
        assert_eq!(first.mcp_action, FileAction::Created);

        let second = run_init(Some(dir_str)).unwrap();
        assert_eq!(second.config_action, FileAction::Unchanged);
        assert_eq!(second.mcp_action, FileAction::Unchanged);
    }

    #[test]
    fn run_init_with_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        // Use absolute path from tempdir to avoid cwd dependency
        let dir_str = dir.path().to_str().unwrap();
        let result = run_init(Some(dir_str)).unwrap();
        assert!(result.mcp_path.is_absolute());
        assert!(result.config_path.is_absolute());
    }
}
