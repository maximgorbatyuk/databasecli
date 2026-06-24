use std::time::{Duration, Instant};

use crate::commands::render;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct QueryResultSet {
    pub database_name: String,
    pub columns: Vec<String>,
    /// Row cells, `None` for SQL `NULL`. Keeping NULL distinct from data lets the
    /// machine formats emit it faithfully (empty CSV field, JSON `null`) instead
    /// of the ambiguous literal text `NULL`.
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub execution_time: Duration,
    pub truncated: bool,
}

/// Display text for a cell in human-facing table output: SQL `NULL` renders as
/// the literal `NULL`.
pub fn cell_display(cell: &Option<String>) -> &str {
    cell.as_deref().unwrap_or("NULL")
}

/// Strip a single optional trailing semicolon (after trailing whitespace) from
/// raw SQL. `SELECT … ;` is a near-universal habit; tolerating exactly one
/// trailing semicolon can never turn a single statement into a multi-statement
/// one, so it is safe on the read-only path. An internal semicolon is left in
/// place for the multi-statement check to reject.
pub fn strip_trailing_semicolon(sql: &str) -> &str {
    let trimmed = sql.trim_end();
    trimmed.strip_suffix(';').unwrap_or(trimmed)
}

pub fn validate_readonly(sql: &str) -> Result<(), DatabaseCliError> {
    let body = strip_trailing_semicolon(sql);
    let stripped = strip_sql_comments(body);

    // Reject multi-statement queries (semicolons outside string literals).
    if contains_unquoted_semicolon(&stripped) {
        return Err(DatabaseCliError::MultiStatement);
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

/// Stringify a cell, or `None` for SQL `NULL`. Callers that render NULL as the
/// literal text `NULL` (table/JSON inspection paths) use [`cell_to_string`];
/// callers that need to distinguish NULL from data (`export`) use this.
pub fn cell_to_string_opt(row: &postgres::Row, idx: usize) -> Option<String> {
    use postgres::types::Type;

    let col_type = row.columns()[idx].type_();

    match *col_type {
        Type::BOOL => row.get::<_, Option<bool>>(idx).map(|v| v.to_string()),
        Type::INT2 => row.get::<_, Option<i16>>(idx).map(|v| v.to_string()),
        Type::INT4 => row.get::<_, Option<i32>>(idx).map(|v| v.to_string()),
        Type::INT8 => row.get::<_, Option<i64>>(idx).map(|v| v.to_string()),
        Type::FLOAT4 => row.get::<_, Option<f32>>(idx).map(|v| v.to_string()),
        Type::FLOAT8 => row.get::<_, Option<f64>>(idx).map(|v| v.to_string()),
        Type::JSON | Type::JSONB => row
            .get::<_, Option<serde_json::Value>>(idx)
            .map(|v| v.to_string()),
        Type::UUID => row.get::<_, Option<uuid::Uuid>>(idx).map(|v| v.to_string()),
        Type::TIMESTAMPTZ => row
            .get::<_, Option<chrono::DateTime<chrono::Utc>>>(idx)
            .map(|v| v.to_rfc3339()),
        Type::TIMESTAMP => row
            .get::<_, Option<chrono::NaiveDateTime>>(idx)
            .map(|v| v.to_string()),
        Type::DATE => row
            .get::<_, Option<chrono::NaiveDate>>(idx)
            .map(|v| v.to_string()),
        Type::TIME => row
            .get::<_, Option<chrono::NaiveTime>>(idx)
            .map(|v| v.to_string()),
        _ => {
            // Fallback: try as text
            match row.try_get::<_, Option<String>>(idx) {
                Ok(Some(v)) => Some(v),
                Ok(None) => None,
                Err(_) => Some("(unsupported type)".to_string()),
            }
        }
    }
}

pub fn cell_to_string(row: &postgres::Row, idx: usize) -> String {
    cell_to_string_opt(row, idx).unwrap_or_else(|| "NULL".to_string())
}

/// True for column types rendered as bare (unquoted) SQL literals in `export
/// --format sql`. Limited to the types [`cell_to_string_opt`] decodes as real
/// numbers/booleans; everything else (including NUMERIC, which is not decoded
/// numerically here) is single-quoted so the emitted INSERT stays valid SQL.
pub fn is_unquoted_sql_type(col_type: &postgres::types::Type) -> bool {
    use postgres::types::Type;
    matches!(
        *col_type,
        Type::BOOL | Type::INT2 | Type::INT4 | Type::INT8 | Type::FLOAT4 | Type::FLOAT8
    )
}

fn should_wrap_with_limit(sql: &str) -> bool {
    let stripped = strip_sql_comments(sql);
    let first_keyword = stripped
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_uppercase();
    matches!(first_keyword.as_str(), "SELECT" | "WITH" | "TABLE")
}

pub fn execute_query(
    conn: &mut LiveConnection,
    sql: &str,
    query_limit: Option<u32>,
) -> Result<QueryResultSet, DatabaseCliError> {
    validate_readonly(sql)?;

    let effective_limit = query_limit.filter(|&l| l > 0);

    // Strip a tolerated trailing `;` so the query is safe to wrap in a subquery.
    let body = strip_trailing_semicolon(sql);

    let effective_sql = match effective_limit {
        Some(limit) if should_wrap_with_limit(body) => {
            format!(
                "SELECT * FROM ({body}) AS _limited_query LIMIT {}",
                limit as i64 + 1
            )
        }
        _ => body.to_string(),
    };

    let start = Instant::now();
    let rows = conn.client.query(&effective_sql, &[])?;
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

    let mut data: Vec<Vec<Option<String>>> = rows
        .iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| cell_to_string_opt(row, i))
                .collect()
        })
        .collect();

    let mut truncated = false;
    if let Some(limit) = effective_limit {
        let limit = limit as usize;
        if data.len() > limit {
            data.truncate(limit);
            truncated = true;
        }
    }

    let row_count = data.len();

    Ok(QueryResultSet {
        database_name: conn.config.name.clone(),
        columns,
        rows: data,
        row_count,
        execution_time,
        truncated,
    })
}

/// Machine- and human-readable shapes for `databasecli query` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Csv,
    Tsv,
    Json,
    Ndjson,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, DatabaseCliError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            "json" => Ok(Self::Json),
            "ndjson" | "jsonl" => Ok(Self::Ndjson),
            other => Err(DatabaseCliError::InvalidOutputFormat(other.to_string())),
        }
    }
}

/// Render only the row data, in the requested format. Goes to stdout so piped
/// output stays free of headers/timing noise (those go to stderr via
/// [`format_query_summary`]).
pub fn format_query_data(result: &QueryResultSet, format: OutputFormat, header: bool) -> String {
    match format {
        OutputFormat::Table => format_table(result, header),
        OutputFormat::Csv => format_delimited(result, ',', header),
        OutputFormat::Tsv => format_delimited(result, '\t', header),
        OutputFormat::Json => format_json(result),
        OutputFormat::Ndjson => format_ndjson(result),
    }
}

/// Human-readable run summary (row count, timing, truncation) intended for
/// stderr so it never corrupts machine-readable stdout.
pub fn format_query_summary(result: &QueryResultSet) -> String {
    let mut out = format!(
        "{} row(s) ({:.0?})\n",
        result.row_count, result.execution_time
    );
    if result.truncated {
        out.push_str(&format!(
            "⚠ output truncated to {} row(s) (more rows exist) — \
             raise the cap with --limit N (0 = unlimited), add SQL LIMIT/OFFSET, \
             or increase [settings] query_limit\n",
            result.row_count
        ));
    }
    out
}

/// Combined table + summary string. Kept for `compare` and any human-facing
/// caller that wants one blob; the CLI `query` path uses the split
/// data/summary functions instead so it can route them to different streams.
pub fn format_query_result(result: &QueryResultSet) -> String {
    if result.columns.is_empty() {
        return format!("Query returned 0 rows ({:.0?})\n", result.execution_time);
    }

    let mut out = format_table(result, true);
    out.push('\n');
    out.push_str(&format!(
        "{} row(s) ({:.0?})\n",
        result.row_count, result.execution_time
    ));
    if result.truncated {
        out.push_str(&format!(
            "(results truncated to {} rows by query_limit)\n",
            result.row_count
        ));
    }
    out
}

fn format_table(result: &QueryResultSet, header: bool) -> String {
    if result.columns.is_empty() {
        return String::new();
    }

    let names: Vec<String> = result
        .columns
        .iter()
        .map(|n| render::table_cell(n))
        .collect();
    let disp: Vec<Vec<String>> = result
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| render::table_cell(cell_display(v)))
                .collect()
        })
        .collect();

    let col_widths: Vec<usize> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let max_data = disp
                .iter()
                .map(|row| row.get(i).map_or(0, |v| v.chars().count()))
                .max()
                .unwrap_or(0);
            name.chars().count().max(max_data).max(4)
        })
        .collect();

    let mut out = String::new();

    if header {
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                out.push_str("  ");
            }
            out.push_str(&format!("{:<width$}", name, width = col_widths[i]));
        }
        out.push('\n');

        for (i, &w) in col_widths.iter().enumerate() {
            if i > 0 {
                out.push_str("  ");
            }
            out.push_str(&"-".repeat(w));
        }
        out.push('\n');
    }

    for row in &disp {
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push_str("  ");
            }
            out.push_str(&format!("{:<width$}", val, width = col_widths[i]));
        }
        out.push('\n');
    }

    out
}

fn format_delimited(result: &QueryResultSet, delim: char, header: bool) -> String {
    let sep = delim.to_string();
    let mut out = String::new();
    if header {
        let line: Vec<String> = result
            .columns
            .iter()
            .map(|c| render::delimited_field(c, delim))
            .collect();
        out.push_str(&line.join(&sep));
        out.push('\n');
    }
    // SQL NULL becomes an empty field, matching the `export` CSV convention.
    for row in &result.rows {
        let line: Vec<String> = row
            .iter()
            .map(|v| {
                v.as_deref()
                    .map_or(String::new(), |s| render::delimited_field(s, delim))
            })
            .collect();
        out.push_str(&line.join(&sep));
        out.push('\n');
    }
    out
}

fn row_to_object(result: &QueryResultSet, row: &[Option<String>]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (i, col) in result.columns.iter().enumerate() {
        let v = match row.get(i) {
            Some(Some(s)) => serde_json::Value::String(s.clone()),
            _ => serde_json::Value::Null,
        };
        map.insert(col.clone(), v);
    }
    serde_json::Value::Object(map)
}

fn format_json(result: &QueryResultSet) -> String {
    let arr: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| row_to_object(result, row))
        .collect();
    let mut out = serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_default();
    out.push('\n');
    out
}

fn format_ndjson(result: &QueryResultSet) -> String {
    let mut out = String::new();
    for row in &result.rows {
        let obj = row_to_object(result, row);
        out.push_str(&serde_json::to_string(&obj).unwrap_or_default());
        out.push('\n');
    }
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

    #[test]
    fn wraps_select_queries() {
        assert!(should_wrap_with_limit("SELECT * FROM users"));
        assert!(should_wrap_with_limit("select 1"));
    }

    #[test]
    fn wraps_with_queries() {
        assert!(should_wrap_with_limit(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
    }

    #[test]
    fn wraps_table_queries() {
        assert!(should_wrap_with_limit("TABLE users"));
    }

    #[test]
    fn does_not_wrap_explain() {
        assert!(!should_wrap_with_limit("EXPLAIN SELECT * FROM users"));
    }

    #[test]
    fn does_not_wrap_show() {
        assert!(!should_wrap_with_limit("SHOW server_version"));
    }

    #[test]
    fn wraps_select_with_leading_comment() {
        assert!(should_wrap_with_limit("/* comment */ SELECT 1"));
    }

    #[test]
    fn does_not_wrap_empty_or_whitespace() {
        assert!(!should_wrap_with_limit(""));
        assert!(!should_wrap_with_limit("   "));
    }

    fn make_result(truncated: bool, row_count: usize) -> QueryResultSet {
        QueryResultSet {
            database_name: "testdb".to_string(),
            columns: vec!["id".to_string()],
            rows: (0..row_count).map(|i| vec![Some(i.to_string())]).collect(),
            row_count,
            execution_time: Duration::from_millis(10),
            truncated,
        }
    }

    #[test]
    fn format_query_result_shows_truncation_notice() {
        let result = make_result(true, 500);
        let output = format_query_result(&result);
        assert!(output.contains("500 row(s)"));
        assert!(output.contains("results truncated to 500 rows by query_limit"));
    }

    #[test]
    fn format_query_result_no_notice_when_not_truncated() {
        let result = make_result(false, 10);
        let output = format_query_result(&result);
        assert!(output.contains("10 row(s)"));
        assert!(!output.contains("truncated"));
    }

    #[test]
    fn format_query_result_empty_result_no_notice() {
        let result = QueryResultSet {
            database_name: "testdb".to_string(),
            columns: vec![],
            rows: vec![],
            row_count: 0,
            execution_time: Duration::from_millis(1),
            truncated: false,
        };
        let output = format_query_result(&result);
        assert!(output.contains("0 rows"));
        assert!(!output.contains("truncated"));
    }

    #[test]
    fn allows_trailing_semicolon() {
        assert!(validate_readonly("SELECT 1;").is_ok());
        assert!(validate_readonly("SELECT 1 ;  ").is_ok());
        assert!(validate_readonly("SELECT 'a;b';").is_ok());
    }

    #[test]
    fn multi_statement_error_message_is_clean() {
        let err = validate_readonly("SELECT 1; SELECT 2").unwrap_err();
        let msg = err.to_string();
        assert!(msg.to_lowercase().contains("multi-statement"));
        // The old bug spliced the sentence into the read-only template.
        assert!(!msg.contains("begins with"));
    }

    #[test]
    fn double_trailing_semicolon_still_rejected() {
        assert!(validate_readonly("SELECT 1;;").is_err());
    }

    fn wide_cell_result() -> QueryResultSet {
        QueryResultSet {
            database_name: "t".to_string(),
            columns: vec!["big".to_string()],
            rows: vec![vec![Some("A".repeat(300_000))]],
            row_count: 1,
            execution_time: Duration::from_millis(1),
            truncated: false,
        }
    }

    #[test]
    fn wide_cell_renders_without_panic() {
        let result = wide_cell_result();
        let out = format_query_result(&result);
        assert!(out.contains('…'));
        // The crash was here: a 300k-wide format width. Exercise all paths.
        let _ = format_query_data(&result, OutputFormat::Table, true);
        let _ = format_query_data(&result, OutputFormat::Csv, true);
    }

    #[test]
    fn csv_quotes_special_chars() {
        let result = QueryResultSet {
            database_name: "t".to_string(),
            columns: vec!["a".to_string(), "b".to_string()],
            rows: vec![
                vec![Some("x,y".to_string()), Some("line1\nline2".to_string())],
                vec![Some("he\"llo".to_string()), Some("plain".to_string())],
            ],
            row_count: 2,
            execution_time: Duration::from_millis(1),
            truncated: false,
        };
        let csv = format_query_data(&result, OutputFormat::Csv, true);
        assert!(csv.starts_with("a,b\n"));
        assert!(csv.contains("\"x,y\""));
        assert!(csv.contains("\"line1\nline2\""));
        assert!(csv.contains("\"he\"\"llo\""));
    }

    #[test]
    fn no_header_omits_header() {
        let result = make_result(false, 2);
        let csv = format_query_data(&result, OutputFormat::Csv, false);
        assert!(!csv.starts_with("id"));
        assert_eq!(csv.lines().count(), 2);
    }

    #[test]
    fn ndjson_one_object_per_row() {
        let result = make_result(false, 3);
        let nd = format_query_data(&result, OutputFormat::Ndjson, true);
        assert_eq!(nd.lines().count(), 3);
        assert!(nd.lines().all(|l| l.starts_with('{') && l.ends_with('}')));
    }

    #[test]
    fn json_is_array_of_objects() {
        let result = make_result(false, 2);
        let j = format_query_data(&result, OutputFormat::Json, true);
        let parsed: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert_eq!(parsed[0]["id"], "0");
    }

    #[test]
    fn summary_routes_truncation_with_actionable_hint() {
        let result = make_result(true, 500);
        let s = format_query_summary(&result);
        assert!(s.contains("500 row(s)"));
        assert!(s.contains("truncated to 500 row(s)"));
        assert!(s.contains("--limit"));
    }

    #[test]
    fn null_is_distinct_across_formats() {
        let result = QueryResultSet {
            database_name: "t".to_string(),
            columns: vec!["a".to_string()],
            rows: vec![vec![None], vec![Some("NULL".to_string())]],
            row_count: 2,
            execution_time: Duration::from_millis(1),
            truncated: false,
        };
        // CSV: real NULL is an empty field; the literal string "NULL" is not.
        let csv = format_query_data(&result, OutputFormat::Csv, false);
        assert_eq!(csv, "\nNULL\n");
        // JSON: real NULL serializes as json null, the string stays a string.
        let json = format_query_data(&result, OutputFormat::Json, true);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["a"], serde_json::Value::Null);
        assert_eq!(parsed[1]["a"], "NULL");
        // Table: both render as the text NULL for humans.
        let table = format_query_data(&result, OutputFormat::Table, false);
        assert_eq!(table.matches("NULL").count(), 2);
    }

    #[test]
    fn output_format_parse() {
        assert_eq!(OutputFormat::parse("CSV").unwrap(), OutputFormat::Csv);
        assert_eq!(OutputFormat::parse("jsonl").unwrap(), OutputFormat::Ndjson);
        assert_eq!(OutputFormat::parse("ndjson").unwrap(), OutputFormat::Ndjson);
        assert!(OutputFormat::parse("xml").is_err());
    }
}
