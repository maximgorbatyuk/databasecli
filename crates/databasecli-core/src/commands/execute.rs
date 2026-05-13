use std::time::{Duration, Instant};

use crate::commands::query::cell_to_string;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

/// SQL classification used by the local-only `exec` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    /// Read-only SQL — `exec` rejects these and points to `query`.
    Read,
    /// Non-destructive write (INSERT, CREATE, GRANT, VACUUM, BEGIN, SET, ...). Runs without confirmation.
    Write,
    /// Destructive write (UPDATE, DELETE, DROP, TRUNCATE, ALTER, or a WITH chain that contains any of those at top level). Requires confirmation.
    Destructive,
    /// Disallowed by `exec` v1 (unknown verb, procedural body, WITH chain with no resolvable DML).
    Unsupported,
}

impl StatementKind {
    pub fn is_destructive(self) -> bool {
        matches!(self, StatementKind::Destructive)
    }

    pub fn requires_confirmation(self) -> bool {
        self.is_destructive()
    }

    /// Order kinds by severity for "most severe wins" merging across CTE bodies.
    fn severity(self) -> u8 {
        match self {
            StatementKind::Unsupported => 0,
            StatementKind::Read => 1,
            StatementKind::Write => 2,
            StatementKind::Destructive => 3,
        }
    }

    fn merge(self, other: StatementKind) -> StatementKind {
        if other.severity() > self.severity() {
            other
        } else {
            self
        }
    }
}

/// Output of validating a single `exec` statement.
///
/// **Invariant:** `sql` is the operator's *original* input with only two
/// transformations applied — surrounding whitespace trimmed and at most one
/// trailing semicolon removed. Inline comments and token adjacency are
/// preserved so PostgreSQL parses exactly what the operator typed, not a
/// rewritten copy. `first_keyword`, `effective_verb`, `kind`, and
/// `has_returning` are derived from a separate comment-stripped analysis copy
/// and used for classification only.
#[derive(Debug, Clone)]
pub struct NormalizedStatement {
    /// Executable SQL — derived from the operator's original input.
    pub sql: String,
    /// Literal first SQL keyword in upper case (e.g. "INSERT", "WITH", "DROP"),
    /// extracted from the comment-stripped analysis copy. Used for human-facing
    /// messages; do **not** classify on this field directly because a leading
    /// `WITH` hides the real DML verb.
    pub first_keyword: String,
    /// Verb that drives execution semantics (RETURNING handling, command tag).
    /// Equals `first_keyword` for non-WITH statements. For a `WITH ... DML`
    /// chain this is the outer DML verb (the one PostgreSQL uses for the
    /// command tag).
    pub effective_verb: String,
    /// Resolved classification. For `WITH` this is the *most severe* DML kind
    /// found at top level across every CTE body and the final clause.
    pub kind: StatementKind,
    /// True when the statement contains a top-level `RETURNING` clause
    /// (computed on the analysis copy so an inline comment like
    /// `INSERT /* RETURNING */ INTO t VALUES (1)` is correctly seen as a
    /// non-RETURNING statement).
    pub has_returning: bool,
}

impl NormalizedStatement {
    /// Return the resolved classification. Pure accessor; classification is
    /// computed once during validation.
    pub fn kind(&self) -> StatementKind {
        self.kind
    }
}

/// One statement extracted from a multi-statement script, with its 1-based
/// starting line number for error context.
#[derive(Debug, Clone)]
pub struct ScriptStatement {
    pub statement: NormalizedStatement,
    pub start_line: usize,
}

/// Result of executing one statement via `execute_statement` or
/// `execute_script`.
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
/// - input whose first keyword cannot be extracted,
/// - `WITH ... <SELECT only>` chains (use `query` for those),
/// - `WITH` chains where no top-level DML verb can be resolved.
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

    let semis = count_unquoted_semicolons(analysis);
    if semis > 1 || (semis == 1 && !analysis.trim_end().ends_with(';')) {
        return Err(DatabaseCliError::UnsupportedExecStatement(
            "multi-statement input is not allowed; submit one statement at a time or use `--file` for scripts".to_string(),
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

    // Resolve effective_verb and kind. For non-WITH the verb is the leading
    // keyword. For WITH we scan the analysis copy for top-level DML verbs
    // across every CTE body and pick the most severe for `kind`; the outer
    // verb (the one after the last CTE) drives `effective_verb` so the
    // command tag matches PostgreSQL's.
    let (effective_verb, kind) = if first_keyword == "WITH" {
        resolve_with_kind(analysis)?
    } else {
        (first_keyword.clone(), classify_keyword(&first_keyword))
    };

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
        effective_verb,
        kind,
        has_returning,
    })
}

/// Classify the leading verb of `sql` for the `exec` path.
///
/// Convenience wrapper that maps any validation failure to `Unsupported`.
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
///
/// Transaction-control verbs (`BEGIN`, `COMMIT`, `ROLLBACK`, `START`, `END`,
/// `SAVEPOINT`, `RELEASE`, `SET`) are classified as `Write` (non-destructive,
/// no confirmation prompt) so multi-statement scripts can group operations in
/// an explicit transaction.
pub fn classify_keyword(kw: &str) -> StatementKind {
    match kw {
        "SELECT" | "SHOW" | "EXPLAIN" | "TABLE" | "VALUES" => StatementKind::Read,
        "INSERT" | "CREATE" | "GRANT" | "REVOKE" | "VACUUM" | "ANALYZE" | "REINDEX" | "COMMENT"
        | "REFRESH" | "CLUSTER" | "BEGIN" | "COMMIT" | "ROLLBACK" | "START" | "END"
        | "SAVEPOINT" | "RELEASE" | "SET" | "RESET" | "LOCK" | "LISTEN" | "UNLISTEN" | "NOTIFY"
        | "DECLARE" | "FETCH" | "CLOSE" | "MOVE" | "CHECKPOINT" => StatementKind::Write,
        "UPDATE" | "DELETE" | "DROP" | "TRUNCATE" | "ALTER" | "MERGE" => StatementKind::Destructive,
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
    let verb = stmt.effective_verb.as_str();
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
        let tag = format!("{verb} {affected}");
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
        let row_count_meaningful = is_row_count_meaningful(verb);
        let tag = if row_count_meaningful {
            format!("{verb} {affected}")
        } else {
            verb.to_string()
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

/// Local CLI/TUI execution only. Do NOT expose through databasecli-mcp.
///
/// Run a pre-validated list of statements against a single writable connection
/// in submission order. Stops at the first failure and returns the error
/// annotated with the source line number of the offending statement so the
/// operator can find it in the script. Statements executed before the
/// failure stay committed unless the script itself wraps them in a
/// transaction (BEGIN/COMMIT).
pub fn execute_script(
    conn: &mut LiveConnection,
    statements: &[ScriptStatement],
) -> Result<Vec<ExecuteResult>, DatabaseCliError> {
    let mut results = Vec::with_capacity(statements.len());
    for entry in statements {
        match execute_normalized(conn, &entry.statement) {
            Ok(result) => results.push(result),
            Err(e) => {
                // Re-wrap with line context. Injected BEGIN/COMMIT (start_line=0)
                // skip the annotation so the user isn't pointed at a synthetic
                // line they didn't write.
                if entry.start_line == 0 {
                    return Err(e);
                }
                let msg = format!("line {}: {e}", entry.start_line);
                return Err(DatabaseCliError::QueryFailed(msg));
            }
        }
    }
    Ok(results)
}

/// Split a multi-statement SQL script into validated single statements.
///
/// The splitter is comment-aware and string-literal-aware. It rejects
/// dollar-quoted bodies up front (consistent with `exec` v1's ban on
/// procedural bodies). Each resulting chunk is run through
/// [`validate_single_statement`]; the error context includes the 1-based line
/// number where the offending chunk starts.
pub fn split_script(script: &str) -> Result<Vec<ScriptStatement>, DatabaseCliError> {
    if contains_dollar_quote(script) {
        return Err(DatabaseCliError::UnsupportedExecStatement(
            "dollar-quoted bodies (e.g. DO $$ ... $$, function definitions) are not allowed"
                .to_string(),
        ));
    }

    let mut out = Vec::new();
    let bytes = script.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut chunk_start = 0;

    while i < len {
        let b = bytes[i];
        match b {
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        if i + 1 < len && bytes[i + 1] == b'\'' {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                let comment_open = i;
                i += 2;
                let mut depth: usize = 1;
                while i < len && depth > 0 {
                    if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                        depth += 1;
                        i += 2;
                    } else if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if depth > 0 {
                    return Err(DatabaseCliError::UnsupportedExecStatement(format!(
                        "line {}: unterminated block comment (`/*` without matching `*/`)",
                        line_of(script, comment_open)
                    )));
                }
            }
            b';' => {
                push_chunk_if_nonempty(&mut out, script, chunk_start, i)?;
                i += 1;
                chunk_start = i;
            }
            _ => i += 1,
        }
    }

    if chunk_start < len {
        push_chunk_if_nonempty(&mut out, script, chunk_start, len)?;
    }

    if out.is_empty() {
        return Err(DatabaseCliError::EmptyQuery);
    }

    Ok(out)
}

/// 1-based line number containing the byte at `byte_offset`. Treats EOF as
/// belonging to the last line.
fn line_of(src: &str, byte_offset: usize) -> usize {
    let upper = byte_offset.min(src.len());
    1 + src.as_bytes()[..upper]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

fn push_chunk_if_nonempty(
    out: &mut Vec<ScriptStatement>,
    script: &str,
    chunk_start: usize,
    chunk_end: usize,
) -> Result<(), DatabaseCliError> {
    let chunk = &script[chunk_start..chunk_end];
    let leading_ws: usize = chunk
        .bytes()
        .take_while(|b| b.is_ascii_whitespace())
        .count();
    if chunk_start + leading_ws >= chunk_end {
        return Ok(());
    }
    let start_line = line_of(script, chunk_start + leading_ws);
    match validate_single_statement(chunk) {
        Ok(statement) => {
            out.push(ScriptStatement {
                statement,
                start_line,
            });
            Ok(())
        }
        // A chunk that becomes empty after comment-stripping (e.g. `;-- end\n`
        // or a stray `;` between two comments) is silently skipped to match
        // `psql -f` behaviour. The whole-script empty case is still caught by
        // the `out.is_empty()` check in `split_script`.
        Err(DatabaseCliError::EmptyQuery) => Ok(()),
        Err(DatabaseCliError::UnsupportedExecStatement(msg)) => Err(
            DatabaseCliError::UnsupportedExecStatement(format!("line {start_line}: {msg}")),
        ),
        Err(other) => Err(other),
    }
}

/// Resolve `(effective_verb, kind)` for a statement whose first keyword is
/// `WITH`.
///
/// Walks the comment-stripped analysis copy and collects:
///   * **CTE body verbs** — classified keywords that appear at paren depth 1
///     immediately after `(`. Each captures the verb that drives one CTE body
///     (e.g. `INSERT`, `DELETE`, `SELECT`).
///   * **Outer verb** — the *first* classified keyword found at depth 0 after
///     we've descended into at least one CTE body. This is the verb after the
///     final closing `)` of the CTE list; any later depth-0 tokens (e.g. the
///     `SELECT` inside `INSERT INTO t (cols) SELECT ...`) are part of the
///     outer statement's body and are ignored.
///
/// `kind` is the most severe classification across the CTE-body verbs and the
/// outer verb. `effective_verb` is the outer verb, which drives the command
/// tag and matches what PostgreSQL itself reports.
fn resolve_with_kind(analysis: &str) -> Result<(String, StatementKind), DatabaseCliError> {
    let bytes = analysis.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut descended = false;
    let mut cte_verbs: Vec<String> = Vec::new();
    let mut outer_verb: Option<String> = None;

    while i < len {
        let b = bytes[i];
        match b {
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        if i + 1 < len && bytes[i + 1] == b'\'' {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            b'(' => {
                depth += 1;
                descended = true;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let upper = analysis[start..i].to_ascii_uppercase();
                if classify_keyword(&upper) == StatementKind::Unsupported {
                    continue;
                }
                if depth >= 1 && prev_non_ws_is(bytes, start, b'(') {
                    // Any classified verb whose token immediately follows `(`
                    // (at any depth >= 1) is treated as a CTE-body verb for
                    // severity purposes. This intentionally over-captures
                    // verbs in nested CTEs and parenthesised subqueries:
                    // `WITH outer AS (WITH inner AS (DELETE ...) SELECT ...) INSERT ...`
                    // must still resolve to `Destructive` even though the
                    // inner DELETE sits at depth 2.
                    cte_verbs.push(upper);
                } else if depth == 0 && descended && outer_verb.is_none() {
                    // The outer verb sits right after the final `)` of the
                    // CTE list. We don't require an exact prev-char match
                    // here — anything at depth 0 after our first descent is
                    // by construction outside the CTE list.
                    outer_verb = Some(upper);
                }
            }
            _ => i += 1,
        }
    }

    let outer = outer_verb.ok_or_else(|| {
        DatabaseCliError::UnsupportedExecStatement(
            "WITH chain has no resolvable outer DML statement".to_string(),
        )
    })?;

    let mut kind = classify_keyword(&outer);
    for verb in &cte_verbs {
        kind = kind.merge(classify_keyword(verb));
    }

    if kind == StatementKind::Unsupported {
        return Err(DatabaseCliError::UnsupportedExecStatement(format!(
            "WITH chain's top-level verb `{outer}` is not supported by `exec` v1"
        )));
    }

    Ok((outer, kind))
}

/// True when the previous non-whitespace byte before `token_start` in `bytes`
/// equals `expect`.
///
/// Walks left over ASCII whitespace one byte at a time. Worst case is O(n)
/// per keyword scan; in practice the analysis copy is short (one statement,
/// comments stripped) and the leading whitespace run before any token is
/// bounded by source-level indentation. Safe to call on every classified
/// token without changing the overall complexity of `resolve_with_kind`.
fn prev_non_ws_is(bytes: &[u8], token_start: usize, expect: u8) -> bool {
    let mut j = token_start;
    while j > 0 && bytes[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j == 0 {
        return false;
    }
    bytes[j - 1] == expect
}

/// True when the postgres command-tag row count for `verb` carries
/// useful information. DDL and maintenance verbs always report 0 rows, so we
/// suppress the count to avoid misleading "TRUNCATE 0", "VACUUM 0" tags.
fn is_row_count_meaningful(verb: &str) -> bool {
    matches!(verb, "INSERT" | "UPDATE" | "DELETE")
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

/// Render a list of script results as one block per statement, separated by
/// a blank line. Each block starts with a `--` header showing the source
/// line and command tag, so a user can match a result to the chunk of the
/// file it came from.
pub fn format_script_results(statements: &[ScriptStatement], results: &[ExecuteResult]) -> String {
    let mut out = String::new();
    for (idx, result) in results.iter().enumerate() {
        let line = statements
            .get(idx)
            .map(|s| s.start_line)
            .unwrap_or_default();
        out.push_str(&format!("-- line {line}: {}\n", result.command_tag));
        out.push_str(&format_execute_result(result));
        if idx + 1 < results.len() {
            out.push('\n');
        }
    }
    out
}

/// True iff the analysis copy contains a `RETURNING` clause at paren depth 0.
///
/// Depth-aware so a RETURNING inside a CTE body (e.g.
/// `WITH d AS (DELETE FROM t RETURNING id) INSERT INTO log SELECT ...`) does
/// not trick the outer non-RETURNING INSERT into the prepare/query path.
fn has_returning_clause(sql: &str) -> bool {
    let mut in_string = false;
    let mut depth: i32 = 0;
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
        if !in_string {
            if b == b'(' {
                depth += 1;
                i += 1;
                continue;
            }
            if b == b')' {
                depth -= 1;
                i += 1;
                continue;
            }
        }
        if !in_string
            && depth == 0
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
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
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
