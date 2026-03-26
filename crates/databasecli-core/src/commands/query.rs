use std::time::{Duration, Instant};

use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct QueryResultSet {
    pub database_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time: Duration,
}

pub fn validate_readonly(sql: &str) -> Result<(), DatabaseCliError> {
    let stripped = strip_sql_comments(sql);

    // Reject multi-statement queries (semicolons outside string literals)
    if contains_unquoted_semicolon(&stripped) {
        return Err(DatabaseCliError::ReadOnlyViolation(
            "multi-statement queries (containing ';') are not allowed".to_string(),
        ));
    }

    let first_keyword = stripped
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_uppercase();

    match first_keyword.as_str() {
        "SELECT" | "WITH" | "EXPLAIN" | "SHOW" | "TABLE" => Ok(()),
        "" => Err(DatabaseCliError::EmptyQuery),
        other => Err(DatabaseCliError::ReadOnlyViolation(other.to_string())),
    }
}

fn contains_unquoted_semicolon(sql: &str) -> bool {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '\'' {
            // Skip string literal
            i += 1;
            while i < len {
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        i += 2; // escaped quote
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if chars[i] == ';' {
            return true;
        } else {
            i += 1;
        }
    }
    false
}

fn strip_sql_comments(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '-' && chars[i + 1] == '-' {
            // Line comment: skip to end of line
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            // Block comment: skip to */
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
        } else if chars[i] == '\'' {
            // String literal: preserve as-is
            result.push(chars[i]);
            i += 1;
            while i < len {
                result.push(chars[i]);
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        // Escaped quote
                        result.push(chars[i + 1]);
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

pub fn cell_to_string(row: &postgres::Row, idx: usize) -> String {
    use postgres::types::Type;

    let col_type = row.columns()[idx].type_();

    match *col_type {
        Type::BOOL => row
            .get::<_, Option<bool>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        Type::INT2 => row
            .get::<_, Option<i16>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        Type::INT4 => row
            .get::<_, Option<i32>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        Type::INT8 => row
            .get::<_, Option<i64>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        Type::FLOAT4 => row
            .get::<_, Option<f32>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        Type::FLOAT8 => row
            .get::<_, Option<f64>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        Type::JSON | Type::JSONB => row
            .get::<_, Option<serde_json::Value>>(idx)
            .map_or("NULL".to_string(), |v| v.to_string()),
        _ => {
            // Fallback: try as text
            match row.try_get::<_, Option<String>>(idx) {
                Ok(Some(v)) => v,
                Ok(None) => "NULL".to_string(),
                Err(_) => "(unsupported type)".to_string(),
            }
        }
    }
}

pub fn execute_query(
    conn: &mut LiveConnection,
    sql: &str,
) -> Result<QueryResultSet, DatabaseCliError> {
    validate_readonly(sql)?;

    let start = Instant::now();
    let rows = conn.client.query(sql, &[])?;
    let execution_time = start.elapsed();

    let columns: Vec<String> = if let Some(first) = rows.first() {
        first
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect()
    } else {
        Vec::new()
    };

    let row_count = rows.len();
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| cell_to_string(row, i))
                .collect()
        })
        .collect();

    Ok(QueryResultSet {
        database_name: conn.config.name.clone(),
        columns,
        rows: data,
        row_count,
        execution_time,
    })
}

pub fn format_query_result(result: &QueryResultSet) -> String {
    if result.columns.is_empty() {
        return format!("Query returned 0 rows ({:.0?})\n", result.execution_time);
    }

    let col_widths: Vec<usize> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let max_data = result
                .rows
                .iter()
                .map(|row| row.get(i).map_or(0, |v| v.len()))
                .max()
                .unwrap_or(0);
            name.len().max(max_data).max(4)
        })
        .collect();

    let mut out = String::new();

    // Header
    for (i, name) in result.columns.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        out.push_str(&format!("{:<width$}", name, width = col_widths[i]));
    }
    out.push('\n');

    // Separator
    for (i, &w) in col_widths.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        out.push_str(&"-".repeat(w));
    }
    out.push('\n');

    // Rows
    for row in &result.rows {
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push_str("  ");
            }
            out.push_str(&format!("{:<width$}", val, width = col_widths[i]));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "\n{} row(s) ({:.0?})\n",
        result.row_count, result.execution_time
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_select() {
        assert!(validate_readonly("SELECT * FROM users").is_ok());
    }

    #[test]
    fn allows_with() {
        assert!(validate_readonly("WITH cte AS (SELECT 1) SELECT * FROM cte").is_ok());
    }

    #[test]
    fn allows_explain() {
        assert!(validate_readonly("EXPLAIN SELECT * FROM users").is_ok());
    }

    #[test]
    fn allows_show() {
        assert!(validate_readonly("SHOW server_version").is_ok());
    }

    #[test]
    fn allows_table() {
        assert!(validate_readonly("TABLE users").is_ok());
    }

    #[test]
    fn rejects_insert() {
        let err = validate_readonly("INSERT INTO users VALUES (1)").unwrap_err();
        assert!(err.to_string().contains("INSERT"));
    }

    #[test]
    fn rejects_update() {
        assert!(validate_readonly("UPDATE users SET name = 'x'").is_err());
    }

    #[test]
    fn rejects_delete() {
        assert!(validate_readonly("DELETE FROM users").is_err());
    }

    #[test]
    fn rejects_drop() {
        assert!(validate_readonly("DROP TABLE users").is_err());
    }

    #[test]
    fn rejects_create() {
        assert!(validate_readonly("CREATE TABLE t (id int)").is_err());
    }

    #[test]
    fn rejects_alter() {
        assert!(validate_readonly("ALTER TABLE users ADD COLUMN x int").is_err());
    }

    #[test]
    fn rejects_truncate() {
        assert!(validate_readonly("TRUNCATE users").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_readonly("").is_err());
        assert!(validate_readonly("   ").is_err());
    }

    #[test]
    fn strips_line_comments() {
        assert!(validate_readonly("-- this is a comment\nSELECT 1").is_ok());
    }

    #[test]
    fn strips_block_comments() {
        assert!(validate_readonly("/* comment */ SELECT 1").is_ok());
    }

    #[test]
    fn rejects_comment_hiding_mutation() {
        // Comment hides the SELECT, first real keyword is DELETE
        assert!(validate_readonly("/* SELECT */ DELETE FROM users").is_err());
    }

    #[test]
    fn case_insensitive() {
        assert!(validate_readonly("select 1").is_ok());
        assert!(validate_readonly("Select 1").is_ok());
    }

    #[test]
    fn rejects_multi_statement_with_semicolon() {
        assert!(validate_readonly("SELECT 1; DROP TABLE users").is_err());
        assert!(validate_readonly("SELECT 1;DELETE FROM users").is_err());
    }

    #[test]
    fn allows_semicolon_inside_string_literal() {
        assert!(validate_readonly("SELECT 'hello;world'").is_ok());
        assert!(validate_readonly("SELECT 'a;b' FROM users").is_ok());
    }

    #[test]
    fn rejects_nested_comment_hiding_mutation() {
        assert!(validate_readonly("/* /* */ DELETE */ SELECT 1").is_err());
    }
}
