use std::path::PathBuf;

use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
}

impl DatabaseConfig {
    pub fn connection_string(&self) -> String {
        format!(
            "host={} port={} user={} password={} dbname={} connect_timeout=5",
            self.host, self.port, self.user, self.password, self.dbname
        )
    }
}

pub const DEFAULT_QUERY_LIMIT: u32 = 500;

#[derive(Debug, Clone)]
pub struct Settings {
    pub query_limit: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            query_limit: DEFAULT_QUERY_LIMIT,
        }
    }
}

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(dir: &str) -> Result<PathBuf, DatabaseCliError> {
    if let Some(rest) = dir.strip_prefix('~') {
        let home = home::home_dir().ok_or(DatabaseCliError::NoHomeDirectory)?;
        let suffix = rest.strip_prefix('/').unwrap_or(rest);
        Ok(home.join(suffix))
    } else {
        Ok(PathBuf::from(dir))
    }
}

/// Resolve the base directory: expand tilde if present, or fall back to cwd.
pub fn resolve_base_dir(directory: Option<&str>) -> Result<PathBuf, DatabaseCliError> {
    match directory {
        Some(dir) => expand_tilde(dir),
        None => std::env::current_dir().map_err(DatabaseCliError::Io),
    }
}

pub fn resolve_config_path() -> Result<PathBuf, DatabaseCliError> {
    resolve_config_path_with_base(None)
}

pub fn resolve_config_path_with_base(base: Option<&str>) -> Result<PathBuf, DatabaseCliError> {
    if let Ok(path) = std::env::var("DATABASECLI_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }

    if let Some(dir) = base {
        let expanded = expand_tilde(dir)?;
        return Ok(expanded.join(".databasecli").join("databases.ini"));
    }

    let cwd = std::env::current_dir().map_err(DatabaseCliError::Io)?;
    Ok(cwd.join(".databasecli").join("databases.ini"))
}

pub fn config_exists() -> Result<bool, DatabaseCliError> {
    config_exists_with_base(None)
}

pub fn config_exists_with_base(base: Option<&str>) -> Result<bool, DatabaseCliError> {
    let path = resolve_config_path_with_base(base)?;
    Ok(path.exists())
}

pub fn create_default_config(path: &PathBuf) -> Result<(), DatabaseCliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(DatabaseCliError::Io)?;
    }

    let template = "\
; databasecli configuration
;
; Global settings (all optional):
;
; [settings]
; query_limit = 500
;
; Add database connections as INI sections:
;
; [my_database]
; host = localhost
; port = 5432
; user = postgres
; password = secret
; dbname = my_database
";

    std::fs::write(path, template).map_err(DatabaseCliError::Io)?;
    Ok(())
}

pub fn load_settings(path: &std::path::Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }

    let mut ini = configparser::ini::Ini::new();
    if ini.load(path).is_err() {
        return Settings::default();
    }

    let query_limit = ini
        .get("settings", "query_limit")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_QUERY_LIMIT);

    Settings { query_limit }
}

pub fn load_databases(path: &PathBuf) -> Result<Vec<DatabaseConfig>, DatabaseCliError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut ini = configparser::ini::Ini::new();
    ini.load(path)
        .map_err(|e| DatabaseCliError::ConfigParse(e.to_string()))?;

    let sections = ini.sections();
    let mut configs = Vec::with_capacity(sections.len());

    for section in &sections {
        if section == "settings" {
            continue;
        }

        let get_field = |field: &str| -> Result<String, DatabaseCliError> {
            ini.get(section, field)
                .ok_or_else(|| DatabaseCliError::MissingField {
                    section: section.clone(),
                    field: field.to_string(),
                })
        };

        let host = get_field("host")?;
        let port_str = get_field("port")?;
        let port: u16 = port_str.parse().map_err(|e: std::num::ParseIntError| {
            DatabaseCliError::InvalidPort {
                section: section.clone(),
                value: port_str.clone(),
                reason: e.to_string(),
            }
        })?;
        let user = get_field("user")?;
        let password = get_field("password")?;
        let dbname = get_field("dbname")?;

        configs.push(DatabaseConfig {
            name: section.clone(),
            host,
            port,
            user,
            password,
            dbname,
        });
    }

    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_ini(content: &str) -> (tempfile::NamedTempFile, PathBuf) {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn parses_single_section() {
        let (_f, path) = write_ini(
            "[production]\n\
             host = localhost\n\
             port = 5432\n\
             user = admin\n\
             password = secret\n\
             dbname = myapp\n",
        );
        let configs = load_databases(&path).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "production");
        assert_eq!(configs[0].host, "localhost");
        assert_eq!(configs[0].port, 5432);
        assert_eq!(configs[0].user, "admin");
        assert_eq!(configs[0].password, "secret");
        assert_eq!(configs[0].dbname, "myapp");
    }

    #[test]
    fn parses_multiple_sections() {
        let (_f, path) = write_ini(
            "[prod]\n\
             host = db.example.com\n\
             port = 5432\n\
             user = app\n\
             password = p1\n\
             dbname = prod_db\n\
             \n\
             [staging]\n\
             host = staging.example.com\n\
             port = 5433\n\
             user = readonly\n\
             password = p2\n\
             dbname = staging_db\n",
        );
        let configs = load_databases(&path).unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn missing_file_returns_empty_list() {
        let path = PathBuf::from("/tmp/nonexistent-databasecli-test.ini");
        let configs = load_databases(&path).unwrap();
        assert!(configs.is_empty());
    }

    #[test]
    fn missing_field_returns_error() {
        let (_f, path) = write_ini(
            "[broken]\n\
             host = localhost\n\
             port = 5432\n",
        );
        let err = load_databases(&path).unwrap_err();
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn invalid_port_returns_error() {
        let (_f, path) = write_ini(
            "[broken]\n\
             host = localhost\n\
             port = not_a_number\n\
             user = u\n\
             password = p\n\
             dbname = d\n",
        );
        let err = load_databases(&path).unwrap_err();
        assert!(err.to_string().contains("invalid port"));
    }

    #[test]
    fn tilde_expansion_in_base_path() {
        let home = home::home_dir().unwrap();
        let path = resolve_config_path_with_base(Some("~/projects/test")).unwrap();
        assert_eq!(path, home.join("projects/test/.databasecli/databases.ini"));
    }

    #[test]
    fn bare_tilde_expansion() {
        let home = home::home_dir().unwrap();
        let path = resolve_config_path_with_base(Some("~")).unwrap();
        assert_eq!(path, home.join(".databasecli/databases.ini"));
    }

    #[test]
    fn absolute_base_path_unchanged() {
        let path = resolve_config_path_with_base(Some("/tmp/myproject")).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/tmp/myproject/.databasecli/databases.ini")
        );
    }

    #[test]
    fn default_path_uses_cwd() {
        // SAFETY: test binary runs this single-threaded; no other thread reads this var concurrently.
        unsafe { std::env::remove_var("DATABASECLI_CONFIG_PATH") };
        let cwd = std::env::current_dir().unwrap();
        let path = resolve_config_path_with_base(None).unwrap();
        assert_eq!(path, cwd.join(".databasecli").join("databases.ini"));
    }

    #[test]
    fn load_settings_defaults_when_no_file() {
        let path = PathBuf::from("/tmp/nonexistent-databasecli-settings.ini");
        let settings = load_settings(&path);
        assert_eq!(settings.query_limit, 500);
    }

    #[test]
    fn load_settings_defaults_when_no_settings_section() {
        let (_f, path) = write_ini(
            "[production]\n\
             host = localhost\n\
             port = 5432\n\
             user = admin\n\
             password = secret\n\
             dbname = myapp\n",
        );
        let settings = load_settings(&path);
        assert_eq!(settings.query_limit, 500);
    }

    #[test]
    fn load_settings_reads_query_limit() {
        let (_f, path) = write_ini(
            "[settings]\n\
             query_limit = 100\n",
        );
        let settings = load_settings(&path);
        assert_eq!(settings.query_limit, 100);
    }

    #[test]
    fn load_settings_defaults_on_invalid_value() {
        let (_f, path) = write_ini(
            "[settings]\n\
             query_limit = abc\n",
        );
        let settings = load_settings(&path);
        assert_eq!(settings.query_limit, 500);
    }

    #[test]
    fn load_settings_zero_means_no_limit() {
        let (_f, path) = write_ini(
            "[settings]\n\
             query_limit = 0\n",
        );
        let settings = load_settings(&path);
        assert_eq!(settings.query_limit, 0);
    }

    #[test]
    fn load_databases_skips_settings_section() {
        let (_f, path) = write_ini(
            "[settings]\n\
             query_limit = 100\n\
             \n\
             [production]\n\
             host = localhost\n\
             port = 5432\n\
             user = admin\n\
             password = secret\n\
             dbname = myapp\n",
        );
        let configs = load_databases(&path).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "production");
    }

    #[test]
    fn connection_string_format() {
        let cfg = DatabaseConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            user: "admin".to_string(),
            password: "secret".to_string(),
            dbname: "mydb".to_string(),
        };
        let cs = cfg.connection_string();
        assert!(cs.contains("host=localhost"));
        assert!(cs.contains("port=5432"));
        assert!(cs.contains("connect_timeout=5"));
    }
}
