use std::fmt;
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

/// Coding agent targets for MCP configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodingAgent {
    ClaudeCode,
    Opencode,
    Codex,
    Cursor,
}

impl CodingAgent {
    /// All available coding agents.
    pub const ALL: [CodingAgent; 4] = [
        CodingAgent::ClaudeCode,
        CodingAgent::Opencode,
        CodingAgent::Codex,
        CodingAgent::Cursor,
    ];

    /// Config file path relative to the project directory.
    pub fn config_filename(&self) -> &'static str {
        match self {
            CodingAgent::ClaudeCode => ".mcp.json",
            CodingAgent::Opencode => "opencode.jsonc",
            CodingAgent::Codex => ".codex/config.toml",
            CodingAgent::Cursor => ".cursor/mcp.json",
        }
    }
}

impl fmt::Display for CodingAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodingAgent::ClaudeCode => write!(f, "Claude Code"),
            CodingAgent::Opencode => write!(f, "Opencode"),
            CodingAgent::Codex => write!(f, "Codex"),
            CodingAgent::Cursor => write!(f, "Cursor"),
        }
    }
}

/// Result of configuring one coding agent.
#[derive(Debug)]
pub struct AgentInitResult {
    pub agent: CodingAgent,
    pub path: PathBuf,
    pub action: FileAction,
}

/// Result of running init -- reports what was done.
pub struct InitResult {
    pub config_path: PathBuf,
    pub config_action: FileAction,
    pub agent_results: Vec<AgentInitResult>,
}

/// Run the full init: create databases.ini if missing, configure selected agents.
pub fn run_init(
    directory: Option<&str>,
    agents: &[CodingAgent],
) -> Result<InitResult, DatabaseCliError> {
    let config_path = resolve_config_path_with_base(directory)?;
    let config_action = if config_path.exists() {
        FileAction::Unchanged
    } else {
        create_default_config(&config_path)?;
        FileAction::Created
    };

    let base_dir = resolve_base_dir(directory)?;
    let mut agent_results = Vec::new();

    for &agent in agents {
        let path = base_dir.join(agent.config_filename());
        let action = match agent {
            CodingAgent::ClaudeCode => upsert_claude_code(&path)?,
            CodingAgent::Opencode => upsert_opencode(&path)?,
            CodingAgent::Codex | CodingAgent::Cursor => {
                if let Some(parent) = path.parent()
                    && !parent.exists()
                {
                    std::fs::create_dir_all(parent).map_err(DatabaseCliError::Io)?;
                }
                match agent {
                    CodingAgent::Codex => upsert_codex(&path)?,
                    CodingAgent::Cursor => upsert_claude_code(&path)?,
                    _ => unreachable!(),
                }
            }
        };
        agent_results.push(AgentInitResult {
            agent,
            path,
            action,
        });
    }

    Ok(InitResult {
        config_path,
        config_action,
        agent_results,
    })
}

// ---------------------------------------------------------------------------
// Claude Code: .mcp.json
// ---------------------------------------------------------------------------

fn upsert_claude_code(path: &Path) -> Result<FileAction, DatabaseCliError> {
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

// ---------------------------------------------------------------------------
// Opencode: opencode.jsonc
// ---------------------------------------------------------------------------

fn upsert_opencode(path: &Path) -> Result<FileAction, DatabaseCliError> {
    let mcp_entry = serde_json::json!({
        "type": "local",
        "command": ["databasecli-mcp", "-D", "."],
        "enabled": true
    });

    if path.exists() {
        let raw = std::fs::read_to_string(path).map_err(DatabaseCliError::Io)?;
        let content = strip_jsonc_comments(&raw);
        let mut doc: Value = serde_json::from_str(&content)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;

        let mcp = doc
            .as_object_mut()
            .ok_or_else(|| DatabaseCliError::ConfigParse("expected JSON object".into()))?
            .entry("mcp")
            .or_insert_with(|| serde_json::json!({}));

        let map = mcp
            .as_object_mut()
            .ok_or_else(|| DatabaseCliError::ConfigParse("mcp must be an object".into()))?;

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
            "$schema": "https://opencode.ai/config.json",
            "mcp": {
                "databasecli": mcp_entry
            }
        });
        let output = serde_json::to_string_pretty(&doc)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;
        std::fs::write(path, output + "\n").map_err(DatabaseCliError::Io)?;
        Ok(FileAction::Created)
    }
}

// ---------------------------------------------------------------------------
// Codex: .codex/config.toml
// ---------------------------------------------------------------------------

fn codex_mcp_entry() -> toml::Table {
    let mut entry = toml::Table::new();
    entry.insert(
        "command".to_string(),
        toml::Value::String("databasecli-mcp".to_string()),
    );
    entry.insert(
        "args".to_string(),
        toml::Value::Array(vec![
            toml::Value::String("-D".to_string()),
            toml::Value::String(".".to_string()),
        ]),
    );
    entry
}

fn upsert_codex(path: &Path) -> Result<FileAction, DatabaseCliError> {
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(DatabaseCliError::Io)?;
        let mut doc: toml::Table = content
            .parse()
            .map_err(|e: toml::de::Error| DatabaseCliError::ConfigParse(e.to_string()))?;

        let servers = doc
            .entry("mcp_servers")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));

        let map = servers
            .as_table_mut()
            .ok_or_else(|| DatabaseCliError::ConfigParse("mcp_servers must be a table".into()))?;

        if map.contains_key("databasecli") {
            return Ok(FileAction::Unchanged);
        }

        map.insert(
            "databasecli".to_string(),
            toml::Value::Table(codex_mcp_entry()),
        );

        let output = toml::to_string_pretty(&doc)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;
        std::fs::write(path, output).map_err(DatabaseCliError::Io)?;
        Ok(FileAction::Updated)
    } else {
        let mut servers = toml::Table::new();
        servers.insert(
            "databasecli".to_string(),
            toml::Value::Table(codex_mcp_entry()),
        );

        let mut doc = toml::Table::new();
        doc.insert("mcp_servers".to_string(), toml::Value::Table(servers));

        let output = toml::to_string_pretty(&doc)
            .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;
        std::fs::write(path, output).map_err(DatabaseCliError::Io)?;
        Ok(FileAction::Created)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Strip JSONC-style comments (`//` line comments and `/* */` block comments)
/// so the result can be parsed by a standard JSON parser.
fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(&c) = chars.peek() {
        if in_string {
            out.push(c);
            chars.next();
            if c == '\\' {
                // skip escaped character
                if let Some(&esc) = chars.peek() {
                    out.push(esc);
                    chars.next();
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
            chars.next();
        } else if c == '/' {
            chars.next();
            match chars.peek() {
                Some(&'/') => {
                    // line comment — consume until newline
                    for ch in chars.by_ref() {
                        if ch == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some(&'*') => {
                    // block comment — consume until */
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {
                    out.push('/');
                }
            }
        } else {
            out.push(c);
            chars.next();
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Claude Code (.mcp.json)
    // -----------------------------------------------------------------------

    #[test]
    fn creates_new_mcp_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");

        let action = upsert_claude_code(&path).unwrap();
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

        let action = upsert_claude_code(&path).unwrap();
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

        let action = upsert_claude_code(&path).unwrap();
        assert_eq!(action, FileAction::Unchanged);
    }

    #[test]
    fn adds_mcp_servers_key_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".mcp.json");
        std::fs::write(&path, "{}").unwrap();

        let action = upsert_claude_code(&path).unwrap();
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

        let err = upsert_claude_code(&path).unwrap_err();
        assert!(err.to_string().contains("config parse error"));
    }

    // -----------------------------------------------------------------------
    // Opencode (opencode.jsonc)
    // -----------------------------------------------------------------------

    #[test]
    fn creates_new_opencode_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.jsonc");

        let action = upsert_opencode(&path).unwrap();
        assert_eq!(action, FileAction::Created);

        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["$schema"], "https://opencode.ai/config.json");
        assert_eq!(content["mcp"]["databasecli"]["type"], "local");
        assert_eq!(
            content["mcp"]["databasecli"]["command"][0],
            "databasecli-mcp"
        );
        assert_eq!(content["mcp"]["databasecli"]["command"][1], "-D");
        assert_eq!(content["mcp"]["databasecli"]["command"][2], ".");
        assert_eq!(content["mcp"]["databasecli"]["enabled"], true);
    }

    #[test]
    fn adds_entry_to_existing_opencode_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.jsonc");
        let existing = r#"{"$schema":"https://opencode.ai/config.json","mcp":{"other":{"type":"local","command":["other"],"enabled":true}}}"#;
        std::fs::write(&path, existing).unwrap();

        let action = upsert_opencode(&path).unwrap();
        assert_eq!(action, FileAction::Updated);

        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(content["mcp"]["other"].is_object());
        assert_eq!(content["mcp"]["databasecli"]["type"], "local");
    }

    #[test]
    fn skips_opencode_if_already_configured() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.jsonc");
        let existing = r#"{"mcp":{"databasecli":{"type":"local","command":["databasecli-mcp"],"enabled":true}}}"#;
        std::fs::write(&path, existing).unwrap();

        let action = upsert_opencode(&path).unwrap();
        assert_eq!(action, FileAction::Unchanged);
    }

    #[test]
    fn parses_opencode_jsonc_with_line_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.jsonc");
        let existing = "{\n  // this is a comment\n  \"mcp\": {}\n}\n";
        std::fs::write(&path, existing).unwrap();

        let action = upsert_opencode(&path).unwrap();
        assert_eq!(action, FileAction::Updated);

        let raw = std::fs::read_to_string(&path).unwrap();
        let content: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(content["mcp"]["databasecli"]["type"], "local");
    }

    #[test]
    fn parses_opencode_jsonc_with_block_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.jsonc");
        let existing = "{\n  /* block comment */\n  \"mcp\": {}\n}\n";
        std::fs::write(&path, existing).unwrap();

        let action = upsert_opencode(&path).unwrap();
        assert_eq!(action, FileAction::Updated);

        let raw = std::fs::read_to_string(&path).unwrap();
        let content: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(content["mcp"]["databasecli"]["type"], "local");
    }

    #[test]
    fn strip_jsonc_preserves_urls_in_strings() {
        let input = r#"{"$schema": "https://opencode.ai/config.json"}"#;
        let output = strip_jsonc_comments(input);
        assert_eq!(input, output);
    }

    #[test]
    fn strip_jsonc_removes_line_and_block_comments() {
        let input = "{\n  // line\n  /* block */\n  \"key\": \"val\"\n}";
        let output = strip_jsonc_comments(input);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["key"], "val");
    }

    // -----------------------------------------------------------------------
    // Codex (.codex/config.toml)
    // -----------------------------------------------------------------------

    #[test]
    fn creates_new_codex_toml() {
        let dir = tempfile::tempdir().unwrap();
        let codex_dir = dir.path().join(".codex");
        std::fs::create_dir(&codex_dir).unwrap();
        let path = codex_dir.join("config.toml");

        let action = upsert_codex(&path).unwrap();
        assert_eq!(action, FileAction::Created);

        let content: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        let entry = content["mcp_servers"]["databasecli"].as_table().unwrap();
        assert_eq!(entry["command"].as_str().unwrap(), "databasecli-mcp");
        let args: Vec<&str> = entry["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(args, vec!["-D", "."]);
    }

    #[test]
    fn adds_entry_to_existing_codex_toml() {
        let dir = tempfile::tempdir().unwrap();
        let codex_dir = dir.path().join(".codex");
        std::fs::create_dir(&codex_dir).unwrap();
        let path = codex_dir.join("config.toml");
        let existing = "[mcp_servers.other]\ncommand = \"other-cmd\"\n";
        std::fs::write(&path, existing).unwrap();

        let action = upsert_codex(&path).unwrap();
        assert_eq!(action, FileAction::Updated);

        let content: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert!(content["mcp_servers"]["other"].is_table());
        assert!(content["mcp_servers"]["databasecli"].is_table());
    }

    #[test]
    fn skips_codex_if_already_configured() {
        let dir = tempfile::tempdir().unwrap();
        let codex_dir = dir.path().join(".codex");
        std::fs::create_dir(&codex_dir).unwrap();
        let path = codex_dir.join("config.toml");
        let existing =
            "[mcp_servers.databasecli]\ncommand = \"databasecli-mcp\"\nargs = [\"-D\", \".\"]\n";
        std::fs::write(&path, existing).unwrap();

        let action = upsert_codex(&path).unwrap();
        assert_eq!(action, FileAction::Unchanged);
    }

    // -----------------------------------------------------------------------
    // Cursor (.cursor/mcp.json) — reuses mcpServers format
    // -----------------------------------------------------------------------

    #[test]
    fn creates_new_cursor_mcp_json() {
        let dir = tempfile::tempdir().unwrap();
        let cursor_dir = dir.path().join(".cursor");
        std::fs::create_dir(&cursor_dir).unwrap();
        let path = cursor_dir.join("mcp.json");

        let action = upsert_claude_code(&path).unwrap();
        assert_eq!(action, FileAction::Created);

        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            content["mcpServers"]["databasecli"]["command"],
            "databasecli-mcp"
        );
    }

    #[test]
    fn run_init_creates_cursor_dir_and_config() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let result = run_init(Some(dir_str), &[CodingAgent::Cursor]).unwrap();

        assert_eq!(result.agent_results.len(), 1);
        assert_eq!(result.agent_results[0].agent, CodingAgent::Cursor);
        assert_eq!(result.agent_results[0].action, FileAction::Created);
        assert!(result.agent_results[0].path.exists());
        assert!(dir.path().join(".cursor/mcp.json").exists());
    }

    // -----------------------------------------------------------------------
    // run_init integration
    // -----------------------------------------------------------------------

    #[test]
    fn run_init_creates_config_and_selected_agents() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let result = run_init(
            Some(dir_str),
            &[CodingAgent::ClaudeCode, CodingAgent::Codex],
        )
        .unwrap();

        assert_eq!(result.config_action, FileAction::Created);
        assert!(result.config_path.exists());
        assert_eq!(result.agent_results.len(), 2);
        assert_eq!(result.agent_results[0].agent, CodingAgent::ClaudeCode);
        assert_eq!(result.agent_results[0].action, FileAction::Created);
        assert!(result.agent_results[0].path.exists());
        assert_eq!(result.agent_results[1].agent, CodingAgent::Codex);
        assert_eq!(result.agent_results[1].action, FileAction::Created);
        assert!(result.agent_results[1].path.exists());
    }

    #[test]
    fn run_init_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let first = run_init(Some(dir_str), &CodingAgent::ALL).unwrap();
        assert_eq!(first.config_action, FileAction::Created);
        assert_eq!(first.agent_results.len(), 4);
        for r in &first.agent_results {
            assert_eq!(r.action, FileAction::Created);
        }

        let second = run_init(Some(dir_str), &CodingAgent::ALL).unwrap();
        assert_eq!(second.config_action, FileAction::Unchanged);
        for r in &second.agent_results {
            assert_eq!(r.action, FileAction::Unchanged);
        }
    }

    #[test]
    fn run_init_with_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();
        let result = run_init(Some(dir_str), &[CodingAgent::ClaudeCode]).unwrap();
        assert!(result.config_path.is_absolute());
        assert!(result.agent_results[0].path.is_absolute());
    }

    #[test]
    fn run_init_empty_agents_creates_config_only() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let result = run_init(Some(dir_str), &[]).unwrap();
        assert_eq!(result.config_action, FileAction::Created);
        assert!(result.agent_results.is_empty());
    }
}
