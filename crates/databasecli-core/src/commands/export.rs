//! Streaming, read-only data export. Rows are pulled through a server-side
//! cursor (`DECLARE … FETCH`) in batches, so neither the client nor the
//! renderer ever materializes the whole result — this sidesteps the
//! `query_limit` cap, the per-statement timeout (each FETCH is its own
//! statement), and the ASCII-table renderer entirely. The export path is
//! deliberately CLI/TUI-only and never wired into the MCP surface.

use std::io::Write;

use crate::commands::query::{
    cell_to_string_opt, is_unquoted_sql_type, strip_trailing_semicolon, validate_readonly,
};
use crate::commands::render;
use crate::commands::validate_identifier;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

const FETCH_BATCH: i64 = 1000;
const CURSOR_NAME: &str = "_databasecli_export";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Jsonl,
    Sql,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Result<Self, DatabaseCliError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "jsonl" | "ndjson" => Ok(Self::Jsonl),
            "sql" => Ok(Self::Sql),
            other => Err(DatabaseCliError::InvalidOutputFormat(other.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    /// SELECT-shaped SQL producing the rows to export.
    pub sql: String,
    pub format: ExportFormat,
    /// Fully-qualified `schema.table` target for `--format sql` INSERTs.
    /// Required for [`ExportFormat::Sql`]; unused for other formats.
    pub target_table: Option<String>,
}

/// Build a request that exports an entire table.
pub fn table_request(
    schema: &str,
    table: &str,
    format: ExportFormat,
) -> Result<ExportRequest, DatabaseCliError> {
    validate_identifier(schema)?;
    validate_identifier(table)?;
    Ok(ExportRequest {
        sql: format!("SELECT * FROM {schema}.{table}"),
        format,
        target_table: Some(format!("{schema}.{table}")),
    })
}

/// Build a request that exports the result of an arbitrary read-only query.
/// `--format sql` is rejected here because INSERTs need a single target table.
pub fn query_request(sql: &str, format: ExportFormat) -> Result<ExportRequest, DatabaseCliError> {
    validate_readonly(sql)?;
    if format == ExportFormat::Sql {
        return Err(DatabaseCliError::InvalidOutputFormat(
            "sql requires a table target — use `export <table> --format sql`".to_string(),
        ));
    }
    Ok(ExportRequest {
        sql: strip_trailing_semicolon(sql).to_string(),
        format,
        target_table: None,
    })
}

/// Stream the request's rows to `out` through a server-side cursor. Returns the
/// number of data rows written.
pub fn export<W: Write>(
    conn: &mut LiveConnection,
    req: &ExportRequest,
    out: &mut W,
) -> Result<u64, DatabaseCliError> {
    let mut tx = conn.client.transaction()?;
    tx.batch_execute(&format!(
        "DECLARE {CURSOR_NAME} NO SCROLL CURSOR FOR {}",
        req.sql
    ))?;

    let fetch = format!("FETCH FORWARD {FETCH_BATCH} FROM {CURSOR_NAME}");
    let mut rows_written: u64 = 0;
    let mut header_written = false;

    loop {
        let batch = tx.query(fetch.as_str(), &[])?;
        if batch.is_empty() {
            break;
        }
        if !header_written {
            write_header(out, &batch[0], req)?;
            header_written = true;
        }
        for row in &batch {
            write_row(out, row, req)?;
            rows_written += 1;
        }
        if (batch.len() as i64) < FETCH_BATCH {
            break;
        }
    }

    tx.batch_execute(&format!("CLOSE {CURSOR_NAME}"))?;
    tx.commit()?;
    out.flush()?;
    Ok(rows_written)
}

fn write_header<W: Write>(
    out: &mut W,
    row: &postgres::Row,
    req: &ExportRequest,
) -> Result<(), DatabaseCliError> {
    if req.format == ExportFormat::Csv {
        let cols: Vec<String> = row
            .columns()
            .iter()
            .map(|c| render::delimited_field(c.name(), ','))
            .collect();
        writeln!(out, "{}", cols.join(","))?;
    }
    Ok(())
}

fn write_row<W: Write>(
    out: &mut W,
    row: &postgres::Row,
    req: &ExportRequest,
) -> Result<(), DatabaseCliError> {
    match req.format {
        ExportFormat::Csv => {
            let fields: Vec<String> = (0..row.columns().len())
                .map(|i| {
                    cell_to_string_opt(row, i)
                        .map_or_else(String::new, |v| render::delimited_field(&v, ','))
                })
                .collect();
            writeln!(out, "{}", fields.join(","))?;
        }
        ExportFormat::Jsonl => {
            let mut map = serde_json::Map::new();
            for (i, col) in row.columns().iter().enumerate() {
                let v = cell_to_string_opt(row, i)
                    .map_or(serde_json::Value::Null, serde_json::Value::String);
                map.insert(col.name().to_string(), v);
            }
            writeln!(out, "{}", serde_json::Value::Object(map))?;
        }
        ExportFormat::Sql => {
            let table = req.target_table.as_deref().unwrap_or("exported_table");
            let cols: Vec<String> = row
                .columns()
                .iter()
                .map(|c| quote_ident(c.name()))
                .collect();
            let vals: Vec<String> = (0..row.columns().len())
                .map(|i| cell_to_sql_literal(row, i))
                .collect();
            writeln!(
                out,
                "INSERT INTO {table} ({}) VALUES ({});",
                cols.join(", "),
                vals.join(", ")
            )?;
        }
    }
    Ok(())
}

fn cell_to_sql_literal(row: &postgres::Row, idx: usize) -> String {
    match cell_to_string_opt(row, idx) {
        None => "NULL".to_string(),
        Some(s) if is_unquoted_sql_type(row.columns()[idx].type_()) => s,
        Some(s) => format!("'{}'", s.replace('\'', "''")),
    }
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_parse() {
        assert_eq!(ExportFormat::parse("CSV").unwrap(), ExportFormat::Csv);
        assert_eq!(ExportFormat::parse("jsonl").unwrap(), ExportFormat::Jsonl);
        assert_eq!(ExportFormat::parse("ndjson").unwrap(), ExportFormat::Jsonl);
        assert_eq!(ExportFormat::parse("sql").unwrap(), ExportFormat::Sql);
        assert!(ExportFormat::parse("parquet").is_err());
    }

    #[test]
    fn table_request_builds_select_and_target() {
        let req = table_request("public", "users", ExportFormat::Sql).unwrap();
        assert_eq!(req.sql, "SELECT * FROM public.users");
        assert_eq!(req.target_table.as_deref(), Some("public.users"));
    }

    #[test]
    fn table_request_rejects_bad_identifier() {
        assert!(table_request("public", "users; DROP", ExportFormat::Csv).is_err());
    }

    #[test]
    fn query_request_rejects_non_readonly() {
        assert!(query_request("DELETE FROM t", ExportFormat::Csv).is_err());
    }

    #[test]
    fn query_request_rejects_sql_format() {
        let err = query_request("SELECT 1", ExportFormat::Sql).unwrap_err();
        assert!(err.to_string().contains("table target"));
    }

    #[test]
    fn query_request_strips_trailing_semicolon() {
        let req = query_request("SELECT 1;", ExportFormat::Csv).unwrap();
        assert_eq!(req.sql, "SELECT 1");
    }

    #[test]
    fn quote_ident_escapes_quotes() {
        assert_eq!(quote_ident("col"), "\"col\"");
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }
}
