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

pub fn resolve_config_path() -> Result<PathBuf, DatabaseCliError> {
    if let Ok(path) = std::env::var("DATABASECLI_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }

    if cfg!(debug_assertions) {
        let exe = std::env::current_exe().map_err(DatabaseCliError::Io)?;
        let dir = exe
            .parent()
            .ok_or_else(|| std::io::Error::other("no parent for exe"))
            .map_err(DatabaseCliError::Io)?;
        Ok(dir.join("databases-dev.ini"))
    } else {
        let home = home::home_dir().ok_or(DatabaseCliError::NoHomeDirectory)?;
        Ok(home.join(".databasecli").join("databases.ini"))
    }
}

pub fn config_exists() -> Result<bool, DatabaseCliError> {
    let path = resolve_config_path()?;
    Ok(path.exists())
}

pub fn create_default_config(path: &PathBuf) -> Result<(), DatabaseCliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(DatabaseCliError::Io)?;
    }

    let template = "\
; databasecli configuration
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
