use std::time::{Duration, Instant};

use crate::commands::query::cell_to_string;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

/// SQL classification used by the local-only `exec` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    /// Read-only SQL — `exec` rejects these and points to `query`.
    Read,
    /// Non-destructive write (INSERT, CREATE, GRANT, VACUUM, etc.). Runs without confirmation.
    Write,
    /// Destructive write (UPDATE, DELETE, DROP, TRUNCATE, ALTER). Requires confirmation.
    Destructive,
    /// Disallowed by `exec` v1 (e.g. `WITH`, unknown verb, procedural body).
    Unsupported,
}

impl StatementKind {
    pub fn is_destructive(self) -> bool {
        matches!(self, StatementKind::Destructive)
    }

    pub fn requires_confirmation(self) -> bool {
        self.is_destructive()
    }
}

/// Output of validating a single `exec` statement.
///
/// **Invariant:** `sql` is the operator's *original* input with only two
/// transformations applied — surrounding whitespace trimmed and at most one
/// trailing semicolon removed. Inline comments and token adjacency are
/// preserved so PostgreSQL parses exactly what the operator typed, not a
/// rewritten copy. `first_keyword` and `has_returning` are derived from a
/// separate comment-stripped analysis copy and used for classification only.
#[derive(Debug, Clone)]
pub struct NormalizedStatement {
    /// Executable SQL — derived from the operator's original input.
    pub sql: String,
    /// First SQL keyword in upper case (e.g. "INSERT", "DROP"), extracted
    /// from the comment-stripped analysis copy.
    pub first_keyword: String,
    /// True when the statement contains a top-level `RETURNING` clause
    /// (computed on the analysis copy so an inline comment like
    /// `INSERT /* RETURNING */ INTO t VALUES (1)` is correctly seen as a
    /// non-RETURNING statement).
    pub has_returning: bool,
}

impl NormalizedStatement {
    /// Classify this statement's leading verb. Pure function over `first_keyword`.
    pub fn kind(&self) -> StatementKind {
        classify_keyword(&self.first_keyword)
    }
}

/// Result of executing one statement via `execute_statement`.
#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub database_name: String,
    /// Synthesized command tag, e.g. "INSERT 5" or "CREATE".
    pub command_tag: String,
    /// Rows affected when known. None for DDL where the count is meaningless.
    pub affected_rows: Option<u64>,
    /// Column names when the statement returned rows (e.g. INSERT ... RETURNING).
    pub columns: Vec<String>,
    /// Returned rows as stringified cells (matches read-only query formatting).
    pub rows: Vec<Vec<String>>,
    pub execution_time: Duration,
}

/// Validate that `sql` is a single, simple SQL statement acceptable to `exec` v1.
///
/// Rejects:
/// - empty input,
/// - multiple top-level statements (any unquoted `;` other than a single trailing one),
/// - dollar-quoted strings (`$$ ... $$`, `$tag$ ... $tag$`) and `DO` blocks,
/// - unterminated block comments (`/*` without a matching `*/`),
/// - input whose first keyword cannot be extracted.
///
/// # Safety invariant — no rewriting of the executable text
///
/// Validation runs against a comment-stripped analysis copy (so `INSERT/*x*/INTO`
/// correctly tokenises to `INSERT INTO`). The `sql` field on the returned
/// [`NormalizedStatement`] is the operator's *original* input with only
/// surrounding whitespace trimmed and at most one trailing semicolon removed.
/// This ensures malformed input like an unterminated block comment cannot be
/// silently transformed into a different, broader destructive statement —
/// either the validator rejects it explicitly or PostgreSQL sees the original
/// text and rejects it with a syntax error.
///
/// # Limitations
///
/// The string-literal scanner only understands the SQL-standard `''` doubling
/// for embedded apostrophes. PostgreSQL's `E'...'` *escape strings* (where
/// `\'` is an escape sequence) are not modelled, so an input that relies on
/// `\'` may be over-rejected as multi-statement. This errs on the safe side —
/// no write is ever silently smuggled through. Rewrite affected statements
/// using `''` doubling.
pub fn validate_single_statement(sql: &str) -> Result<NormalizedStatement, DatabaseCliError> {
    // Analysis copy: comments removed, block comments replaced with a single
    // space so token boundaries (e.g. `INSERT/*x*/INTO`) are preserved.
    // `?` propagates the error for unterminated block comments.
    let stripped = strip_sql_comments(sql)?;
    let analysis = stripped.trim();

    if analysis.is_empty() {
        return Err(DatabaseCliError::EmptyQuery);
    }

    if contains_dollar_quote(analysis) {
        return Err(DatabaseCliError::UnsupportedExecStatement(
            "dollar-quoted bodies (e.g. DO $$ ... $$, function definitions) are not allowed"
                .to_string(),
        ));
    }

    // Multi-statement check runs on the analysis copy. Allowed: zero
    // semicolons, or exactly one that sits at the very end.
    let semis = count_unquoted_semicolons(analysis);
    if semis > 1 || (semis == 1 && !analysis.trim_end().ends_with(';')) {
        return Err(DatabaseCliError::UnsupportedExecStatement(
            "multi-statement input is not allowed; submit one statement at a time".to_string(),
        ));
    }

    let first_keyword = analysis
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_uppercase();

    if first_keyword.is_empty() {
        return Err(DatabaseCliError::EmptyQuery);
    }

    if first_keyword == "DO" {
        return Err(DatabaseCliError::UnsupportedExecStatement(
            "procedural DO blocks are not allowed".to_string(),
        ));
    }

    let has_returning = has_returning_clause(analysis);

    // Build executable SQL from the ORIGINAL input. Permitted transformations:
    // trim surrounding whitespace, remove a single trailing semicolon. We never
    // rewrite the body — inline comments and token adjacency are preserved so
    // PostgreSQL parses what the operator typed.
    let original_trimmed = sql.trim();
    let executable = if let Some(without_semi) = original_trimmed.strip_suffix(';') {
        without_semi.trim_end().to_string()
    } else {
        original_trimmed.to_string()
    };

    if executable.is_empty() {
        return Err(DatabaseCliError::EmptyQuery);
    }

    Ok(NormalizedStatement {
        sql: executable,
        first_keyword,
        has_returning,
    })
}

/// Classify the leading verb of `sql` for the `exec` path.
///
/// Returns `Unsupported` for anything `validate_single_statement` rejects, for `WITH`
/// (because writable CTEs cannot be classified safely), and for any unrecognized verb.
pub fn classify_statement(sql: &str) -> StatementKind {
    match validate_single_statement(sql) {
        Ok(n) => n.kind(),
        Err(_) => StatementKind::Unsupported,
    }
}

/// Pure verb-to-kind classifier. Public so callers that already hold a
/// `NormalizedStatement` don't need to re-validate.
///
/// `COPY` is intentionally absent from the Write list. PostgreSQL's `COPY`
/// requires the `copy_in` / `copy_out` streaming APIs, which the `exec` v1
/// path does not implement (it only uses `Client::execute` and
/// `Client::query`). Treating `COPY` as a regular write would either silently
/// drop the data stream or produce a confusing protocol error, so v1 rejects
/// it as `Unsupported`.
pub fn classify_keyword(kw: &str) -> StatementKind {
    match kw {
        "SELECT" | "SHOW" | "EXPLAIN" | "TABLE" => StatementKind::Read,
        "INSERT" | "CREATE" | "GRANT" | "REVOKE" | "VACUUM" | "ANALYZE" | "REINDEX" | "COMMENT"
        | "REFRESH" | "CLUSTER" => StatementKind::Write,
        "UPDATE" | "DELETE" | "DROP" | "TRUNCATE" | "ALTER" => StatementKind::Destructive,
        _ => StatementKind::Unsupported,
    }
}

/// Local CLI/TUI execution only. Do NOT expose through databasecli-mcp.
///
/// Validates and classifies `sql`, then runs it against a writable connection.
/// Convenience wrapper over [`execute_normalized`] for callers that haven't
/// validated yet. Read and Unsupported statements are rejected so a
/// misclassified call cannot silently run as a write.
pub fn execute_statement(
    conn: &mut LiveConnection,
    sql: &str,
) -> Result<ExecuteResult, DatabaseCliError> {
    let normalized = validate_single_statement(sql)?;
    execute_normalized(conn, &normalized)
}

/// Local CLI/TUI execution only. Do NOT expose through databasecli-mcp.
///
/// Run a pre-validated statement against a writable connection. Lets callers
/// validate once and reuse the normalized form (e.g. for confirmation prompts)
/// without re-parsing.
pub fn execute_normalized(
    conn: &mut LiveConnection,
    stmt: &NormalizedStatement,
) -> Result<ExecuteResult, DatabaseCliError> {
    let kind = stmt.kind();

    match kind {
        StatementKind::Read => {
            return Err(DatabaseCliError::UnsupportedExecStatement(format!(
                "{} is read-only; use the `query` command instead",
                stmt.first_keyword
            )));
        }
        StatementKind::Unsupported => {
            return Err(DatabaseCliError::UnsupportedExecStatement(format!(
                "leading keyword `{}` is not supported by `exec` v1",
                stmt.first_keyword
            )));
        }
        StatementKind::Write | StatementKind::Destructive => {}
    }

    let start = Instant::now();
    // Use the cached has_returning computed from the analysis copy. Reading
    // it off the original `stmt.sql` would mis-detect inputs like
    // `INSERT /* RETURNING */ INTO t VALUES (1)`.
    let mut result = if stmt.has_returning {
        // Prepare first so we can capture column metadata even when the
        // statement returns zero rows (e.g. ON CONFLICT DO NOTHING RETURNING).
        let prepared = conn.client.prepare(stmt.sql.as_str())?;
        let columns: Vec<String> = prepared
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let rows = conn.client.query(&prepared, &[])?;
        let data: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                (0..row.columns().len())
                    .map(|i| cell_to_string(row, i))
                    .collect()
            })
            .collect();
        let affected = data.len() as u64;
        let tag = format!("{} {affected}", stmt.first_keyword);
        ExecuteResult {
            database_name: conn.config.name.clone(),
            command_tag: tag,
            affected_rows: Some(affected),
            columns,
            rows: data,
            execution_time: Duration::default(),
        }
    } else {
        let affected = conn.client.execute(stmt.sql.as_str(), &[])?;
        let row_count_meaningful = is_row_count_meaningful(&stmt.first_keyword);
        let tag = if row_count_meaningful {
            format!("{} {affected}", stmt.first_keyword)
        } else {
            stmt.first_keyword.clone()
        };
        ExecuteResult {
            database_name: conn.config.name.clone(),
            command_tag: tag,
            affected_rows: row_count_meaningful.then_some(affected),
            columns: Vec::new(),
            rows: Vec::new(),
            execution_time: Duration::default(),
        }
    };

    result.execution_time = start.elapsed();
    Ok(result)
}

/// True when the postgres command-tag row count for `first_keyword` carries
/// useful information. DDL and maintenance verbs always report 0 rows, so we
/// suppress the count to avoid the misleading "TRUNCATE 0", "VACUUM 0" tags.
/// `COPY` is excluded because `exec` v1 rejects it before reaching this
/// helper — listing it here would only confuse future readers.
fn is_row_count_meaningful(first_keyword: &str) -> bool {
    matches!(first_keyword, "INSERT" | "UPDATE" | "DELETE")
}

pub fn format_execute_result(result: &ExecuteResult) -> String {
    let elapsed = format!("{:.3}s", result.execution_time.as_secs_f64());
    let summary = match result.affected_rows {
        Some(1) => format!("1 row affected ({elapsed})"),
        Some(n) => format!("{n} rows affected ({elapsed})"),
        None => format!("{} ({elapsed})", result.command_tag),
    };

    if !result.columns.is_empty() {
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

        for (i, name) in result.columns.iter().enumerate() {
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

        for row in &result.rows {
            for (i, val) in row.iter().enumerate() {
                if i > 0 {
                    out.push_str("  ");
                }
                out.push_str(&format!("{:<width$}", val, width = col_widths[i]));
            }
            out.push('\n');
        }

        out.push('\n');
        out.push_str(&summary);
        out.push('\n');
        out
    } else {
        format!("{summary}\n")
    }
}

fn has_returning_clause(sql: &str) -> bool {
    let mut in_string = false;
    let bytes = sql.as_bytes();
    let needle = b"RETURNING";
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' {
            if in_string && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string
            && i + needle.len() <= bytes.len()
            && bytes[i..i + needle.len()].eq_ignore_ascii_case(needle)
        {
            let before_ok = i == 0
                || matches!(
                    bytes[i - 1],
                    b' ' | b'\t' | b'\n' | b'\r' | b')' | b'*' | b'"'
                );
            let after_idx = i + needle.len();
            let after_ok = after_idx == bytes.len()
                || matches!(
                    bytes[after_idx],
                    b' ' | b'\t' | b'\n' | b'\r' | b'(' | b'"' | b';'
                );
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn count_unquoted_semicolons(sql: &str) -> usize {
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut count = 0usize;
    while i < len {
        if chars[i] == '\'' {
            i += 1;
            while i < len {
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if chars[i] == ';' {
            count += 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    count
}

fn contains_dollar_quote(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut in_string = false;
    let mut i = 0;
    while i < len {
        let b = bytes[i];
        if b == b'\'' {
            if in_string && i + 1 < len && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string && b == b'$' {
            // Look for a matching closing $tag$ pattern. Tag is [A-Za-z_][A-Za-z0-9_]*.
            let mut j = i + 1;
            while j < len && is_dollar_tag_char(bytes[j], j == i + 1) {
                j += 1;
            }
            if j < len && bytes[j] == b'$' {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_dollar_tag_char(b: u8, first: bool) -> bool {
    if first {
        b.is_ascii_alphabetic() || b == b'_'
    } else {
        b.is_ascii_alphanumeric() || b == b'_'
    }
}

/// Return a copy of `sql` with comments removed for analysis. Block comments
/// are replaced with a single space so adjacent tokens (e.g.
/// `INSERT/*x*/INTO`) stay separable. Line comments end at the next newline,
/// which is preserved naturally as a token separator.
///
/// **Used for analysis only.** The returned string is never executed against
/// PostgreSQL — `validate_single_statement` builds the executable SQL from the
/// operator's *original* input. See `NormalizedStatement` for the invariant.
///
/// Returns `UnsupportedExecStatement` if a `/*` is opened without a matching
/// `*/`. Without this check, an unterminated comment would silently swallow
/// the rest of the input, potentially turning `DELETE FROM t /* WHERE id = 1`
/// into a valid-looking `DELETE FROM t` in the analysis copy.
fn strip_sql_comments(sql: &str) -> Result<String, DatabaseCliError> {
    let mut result = String::with_capacity(sql.len());
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '-' && chars[i + 1] == '-' {
            // Line comment — skip to newline (or EOF). The newline (if any)
            // is left in place to act as a token separator.
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            // Block comment — emit a single space so adjacent tokens stay
            // separate in the analysis copy.
            result.push(' ');
            i += 2;
            let mut depth: usize = 1;
            while i < len && depth > 0 {
                if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth > 0 {
                return Err(DatabaseCliError::UnsupportedExecStatement(
                    "unterminated block comment (`/*` without matching `*/`)".to_string(),
                ));
            }
        } else if chars[i] == '\'' {
            // String literal — preserved verbatim so dollar-quote and
            // semicolon scans see the exact same characters PostgreSQL will.
            result.push(chars[i]);
            i += 1;
            while i < len {
                result.push(chars[i]);
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
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

    Ok(result)
}

#[cfg(test)]
mod tests;
