pub mod analyze;
pub mod compare;
pub mod erd;
pub mod health;
pub mod list_databases;
pub mod query;
pub mod sample;
pub mod schema;
pub mod summary;
pub mod trend;

use crate::error::DatabaseCliError;

pub fn validate_identifier(s: &str) -> Result<(), DatabaseCliError> {
    if s.is_empty() {
        return Err(DatabaseCliError::InvalidIdentifier(s.to_string()));
    }
    let first = s.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(DatabaseCliError::InvalidIdentifier(s.to_string()));
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(DatabaseCliError::InvalidIdentifier(s.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_identifiers() {
        assert!(validate_identifier("users").is_ok());
        assert!(validate_identifier("_private").is_ok());
        assert!(validate_identifier("table_123").is_ok());
        assert!(validate_identifier("A").is_ok());
    }

    #[test]
    fn invalid_identifiers() {
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("123abc").is_err());
        assert!(validate_identifier("table-name").is_err());
        assert!(validate_identifier("table name").is_err());
        assert!(validate_identifier("drop;--").is_err());
    }
}
