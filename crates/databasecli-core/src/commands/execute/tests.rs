use std::time::Duration;

use super::*;

// validate_single_statement

#[test]
fn validate_rejects_empty() {
    assert!(matches!(
        validate_single_statement(""),
        Err(DatabaseCliError::EmptyQuery)
    ));
    assert!(matches!(
        validate_single_statement("   \n  "),
        Err(DatabaseCliError::EmptyQuery)
    ));
}

#[test]
fn validate_rejects_only_comments() {
    assert!(matches!(
        validate_single_statement("-- nothing here\n/* still nothing */"),
        Err(DatabaseCliError::EmptyQuery)
    ));
}

#[test]
fn validate_allows_single_trailing_semicolon() {
    let n = validate_single_statement("INSERT INTO t VALUES (1);").unwrap();
    assert_eq!(n.first_keyword, "INSERT");
    assert_eq!(n.sql, "INSERT INTO t VALUES (1)");
}

#[test]
fn validate_allows_no_semicolon() {
    let n = validate_single_statement("DELETE FROM t").unwrap();
    assert_eq!(n.first_keyword, "DELETE");
    assert_eq!(n.sql, "DELETE FROM t");
}

#[test]
fn validate_rejects_multi_statement() {
    let err = validate_single_statement("INSERT INTO t VALUES (1); DELETE FROM t").unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

#[test]
fn validate_rejects_internal_semicolon_even_without_second_stmt() {
    let err = validate_single_statement("INSERT; INTO t VALUES (1)").unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

#[test]
fn validate_allows_semicolon_inside_string_literal() {
    let n = validate_single_statement("INSERT INTO t VALUES ('a;b')").unwrap();
    assert_eq!(n.first_keyword, "INSERT");
}

#[test]
fn validate_rejects_dollar_quoted_body() {
    let err = validate_single_statement("DO $$ BEGIN RAISE NOTICE 'x'; END $$").unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

#[test]
fn validate_rejects_tagged_dollar_quoted_body() {
    let err = validate_single_statement(
        "CREATE FUNCTION foo() RETURNS void AS $body$ BEGIN END $body$ LANGUAGE plpgsql",
    )
    .unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

#[test]
fn validate_rejects_do_keyword() {
    let err = validate_single_statement("DO LANGUAGE plpgsql 'BEGIN END'").unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

#[test]
fn validate_first_keyword_uppercased() {
    let n = validate_single_statement("update t set x = 1").unwrap();
    assert_eq!(n.first_keyword, "UPDATE");
}

#[test]
fn validate_handles_leading_block_comment() {
    let n = validate_single_statement("/* hint */ INSERT INTO t VALUES (1)").unwrap();
    assert_eq!(n.first_keyword, "INSERT");
}

#[test]
fn validate_handles_leading_line_comment() {
    let n = validate_single_statement("-- doc\nDELETE FROM t").unwrap();
    assert_eq!(n.first_keyword, "DELETE");
}

#[test]
fn validate_strips_only_one_trailing_semicolon() {
    // `;;` means one statement, then an empty trailing statement — reject.
    let err = validate_single_statement("DELETE FROM t;;").unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

// Safety invariant: the executable SQL is the operator's original input,
// never a comment-stripped or otherwise rewritten copy.

#[test]
fn validate_rejects_unterminated_block_comment() {
    // The dangerous case: stripping a `/*` that has no `*/` would have
    // silently widened `DELETE FROM users /* WHERE id = 1` into a clean
    // `DELETE FROM users` in the analysis copy. Reject explicitly so the
    // operator gets a clear error and PostgreSQL is never asked to run a
    // statement broader than what was typed.
    let err = validate_single_statement("DELETE FROM users /* WHERE id = 1").unwrap_err();
    let msg = err.to_string();
    assert!(
        matches!(err, DatabaseCliError::UnsupportedExecStatement(_)),
        "expected UnsupportedExecStatement, got: {err:?}"
    );
    assert!(
        msg.contains("unterminated") && msg.contains("block comment"),
        "expected unterminated-block-comment message, got: {msg}"
    );
}

#[test]
fn validate_rejects_unterminated_nested_block_comment() {
    let err = validate_single_statement("DELETE FROM users /* outer /* inner */").unwrap_err();
    assert!(matches!(err, DatabaseCliError::UnsupportedExecStatement(_)));
}

#[test]
fn validate_does_not_silently_widen_destructive_statement() {
    // Belt-and-braces: prove that for the canonical broken-comment input the
    // validator never produces a NormalizedStatement at all (so there is no
    // way for a stripped, broader DELETE to flow into execute_normalized).
    let result = validate_single_statement("DELETE FROM users /* WHERE id = 1");
    assert!(result.is_err());
}

#[test]
fn validate_preserves_inline_comment_in_executable_sql() {
    // Token adjacency: `INSERT/*x*/INTO` is valid PostgreSQL — the comment
    // acts as whitespace. Validation must classify it as INSERT (Write), and
    // the executable SQL must keep the inline comment so PG parses what the
    // operator typed, not a tokenized rewrite like "INSERTINTO ..." or
    // "INSERT INTO ...".
    let n = validate_single_statement("INSERT/*x*/INTO tbl VALUES (1)").unwrap();
    assert_eq!(n.first_keyword, "INSERT");
    assert_eq!(n.kind(), StatementKind::Write);
    assert_eq!(n.sql, "INSERT/*x*/INTO tbl VALUES (1)");
}

#[test]
fn validate_preserves_original_sql_with_block_comment() {
    let n = validate_single_statement("UPDATE t /* hint */ SET x = 1").unwrap();
    assert_eq!(n.first_keyword, "UPDATE");
    // Original text preserved verbatim (modulo trim, which does nothing here).
    assert_eq!(n.sql, "UPDATE t /* hint */ SET x = 1");
}

#[test]
fn validate_preserves_original_sql_with_leading_comment() {
    let n = validate_single_statement("/* hint */ DELETE FROM t").unwrap();
    assert_eq!(n.first_keyword, "DELETE");
    // Leading comment is preserved in the executable text.
    assert_eq!(n.sql, "/* hint */ DELETE FROM t");
}

#[test]
fn validate_strips_only_trim_and_one_trailing_semicolon_from_original() {
    // Surrounding whitespace: trimmed. Single trailing `;`: removed.
    // Everything else preserved verbatim.
    let n = validate_single_statement("  \n DELETE /*x*/ FROM t ;  \n").unwrap();
    assert_eq!(n.sql, "DELETE /*x*/ FROM t");
}

#[test]
fn validate_has_returning_ignores_comment_text() {
    // The literal token `RETURNING` only appears inside a comment, so the
    // statement does NOT have a RETURNING clause.
    let n = validate_single_statement("INSERT /* RETURNING */ INTO t VALUES (1)").unwrap();
    assert!(
        !n.has_returning,
        "RETURNING inside a comment must not be detected as a RETURNING clause"
    );
}

#[test]
fn validate_has_returning_detected_when_real() {
    let n = validate_single_statement("INSERT INTO t VALUES (1) RETURNING id").unwrap();
    assert!(n.has_returning);
}

// classify_statement

#[test]
fn classify_read_keywords() {
    for kw in [
        "SELECT * FROM t",
        "show server_version",
        "EXPLAIN SELECT 1",
        "TABLE t",
    ] {
        assert_eq!(classify_statement(kw), StatementKind::Read, "input: {kw}");
    }
}

#[test]
fn classify_write_keywords() {
    for kw in [
        "INSERT INTO t VALUES (1)",
        "CREATE TABLE t (id int)",
        "GRANT SELECT ON t TO u",
        "REVOKE SELECT ON t FROM u",
        "VACUUM t",
        "ANALYZE t",
        "REINDEX TABLE t",
        "COMMENT ON TABLE t IS 'x'",
        "REFRESH MATERIALIZED VIEW v",
        "CLUSTER t",
        // NOTE: COPY intentionally omitted — exec v1 does not implement
        // copy_in/copy_out and rejects it as Unsupported. See
        // `copy_is_unsupported_in_v1` below.
    ] {
        assert_eq!(classify_statement(kw), StatementKind::Write, "input: {kw}");
    }
}

#[test]
fn classify_destructive_keywords() {
    for kw in [
        "UPDATE t SET x = 1",
        "DELETE FROM t",
        "DROP TABLE t",
        "TRUNCATE t",
        "ALTER TABLE t ADD COLUMN y int",
    ] {
        assert_eq!(
            classify_statement(kw),
            StatementKind::Destructive,
            "input: {kw}"
        );
    }
}

#[test]
fn classify_with_is_unsupported() {
    assert_eq!(
        classify_statement("WITH x AS (SELECT 1) SELECT * FROM x"),
        StatementKind::Unsupported
    );
    assert_eq!(
        classify_statement("WITH x AS (DELETE FROM t RETURNING id) SELECT * FROM x"),
        StatementKind::Unsupported
    );
}

#[test]
fn classify_unknown_verb_unsupported() {
    assert_eq!(classify_statement("BLARGH foo"), StatementKind::Unsupported);
}

#[test]
fn copy_is_unsupported_in_v1() {
    // exec v1 does not implement copy_in/copy_out — running COPY through
    // Client::query/execute would either drop the data stream or produce a
    // confusing protocol error. Reject it up front.
    assert_eq!(
        classify_statement("COPY t TO STDOUT"),
        StatementKind::Unsupported
    );
    assert_eq!(
        classify_statement("COPY t FROM STDIN"),
        StatementKind::Unsupported
    );
    assert_eq!(
        classify_statement("copy t from '/tmp/x'"),
        StatementKind::Unsupported
    );
    assert_eq!(classify_keyword("COPY"), StatementKind::Unsupported);
}

#[test]
fn execute_normalized_rejects_copy_before_running() {
    // Proves the rejection happens at the validation/classification layer,
    // before any connection is touched. We can construct a NormalizedStatement
    // by hand to feed execute_normalized — but since we have no real
    // connection in unit tests, we exercise the same gate via classify.
    let n = validate_single_statement("COPY t FROM STDIN").unwrap();
    assert_eq!(n.kind(), StatementKind::Unsupported);
}

#[test]
fn classify_invalid_input_unsupported() {
    assert_eq!(classify_statement(""), StatementKind::Unsupported);
    assert_eq!(
        classify_statement("DO $$ BEGIN END $$"),
        StatementKind::Unsupported
    );
    assert_eq!(
        classify_statement("DELETE FROM t; DROP TABLE t"),
        StatementKind::Unsupported
    );
}

#[test]
fn classify_comment_hidden_first_keyword() {
    // The comment is stripped, so the real first keyword wins.
    assert_eq!(
        classify_statement("/* SELECT */ DELETE FROM users"),
        StatementKind::Destructive
    );
}

#[test]
fn classify_case_insensitive() {
    assert_eq!(
        classify_statement("delete from t"),
        StatementKind::Destructive
    );
    assert_eq!(
        classify_statement("Insert INTO t VALUES (1)"),
        StatementKind::Write
    );
}

#[test]
fn destructive_helpers() {
    assert!(StatementKind::Destructive.is_destructive());
    assert!(StatementKind::Destructive.requires_confirmation());
    assert!(!StatementKind::Write.is_destructive());
    assert!(!StatementKind::Read.requires_confirmation());
}

#[test]
fn normalized_kind_matches_classify_statement() {
    let n = validate_single_statement("DELETE FROM t").unwrap();
    assert_eq!(n.kind(), StatementKind::Destructive);
    assert_eq!(n.kind(), classify_statement("DELETE FROM t"));
}

#[test]
fn classify_keyword_is_pure_function_over_verb() {
    assert_eq!(classify_keyword("INSERT"), StatementKind::Write);
    assert_eq!(classify_keyword("DROP"), StatementKind::Destructive);
    assert_eq!(classify_keyword("SELECT"), StatementKind::Read);
    assert_eq!(classify_keyword("WITH"), StatementKind::Unsupported);
    assert_eq!(classify_keyword("BLARGH"), StatementKind::Unsupported);
}

#[test]
fn row_count_meaningful_excludes_truncate_ddl_and_copy() {
    // DML keeps the count.
    assert!(is_row_count_meaningful("INSERT"));
    assert!(is_row_count_meaningful("UPDATE"));
    assert!(is_row_count_meaningful("DELETE"));
    // TRUNCATE always reports 0 — suppress the count to avoid the misleading
    // "TRUNCATE 0" tag.
    assert!(!is_row_count_meaningful("TRUNCATE"));
    // COPY is rejected upstream by classify_keyword in v1; its row count is
    // never reported via this path.
    assert!(!is_row_count_meaningful("COPY"));
    // DDL.
    assert!(!is_row_count_meaningful("CREATE"));
    assert!(!is_row_count_meaningful("DROP"));
    assert!(!is_row_count_meaningful("ALTER"));
    assert!(!is_row_count_meaningful("VACUUM"));
    assert!(!is_row_count_meaningful("GRANT"));
}

// has_returning_clause

#[test]
fn returning_clause_detected() {
    assert!(has_returning_clause(
        "INSERT INTO t VALUES (1) RETURNING id"
    ));
    assert!(has_returning_clause("UPDATE t SET x = 1 returning *"));
    assert!(has_returning_clause(
        "DELETE FROM t WHERE id = 1 RETURNING id, name"
    ));
}

#[test]
fn returning_clause_not_in_string_literal() {
    assert!(!has_returning_clause("INSERT INTO t VALUES ('RETURNING')"));
}

#[test]
fn returning_must_be_a_word_boundary() {
    assert!(!has_returning_clause(
        "INSERT INTO returning_table VALUES (1)"
    ));
    assert!(!has_returning_clause("INSERT INTO t VALUES ('xreturning')"));
}

// format_execute_result

fn make_result(columns: Vec<&str>, rows: Vec<Vec<&str>>, command_tag: &str) -> ExecuteResult {
    ExecuteResult {
        database_name: "testdb".to_string(),
        command_tag: command_tag.to_string(),
        affected_rows: Some(rows.len() as u64),
        columns: columns.iter().map(|s| s.to_string()).collect(),
        rows: rows
            .into_iter()
            .map(|r| r.into_iter().map(|s| s.to_string()).collect())
            .collect(),
        execution_time: Duration::from_millis(12),
    }
}

#[test]
fn format_command_tag_only_for_no_rows() {
    let r = ExecuteResult {
        database_name: "testdb".to_string(),
        command_tag: "DELETE 7".to_string(),
        affected_rows: Some(7),
        columns: Vec::new(),
        rows: Vec::new(),
        execution_time: Duration::from_millis(523),
    };
    let out = format_execute_result(&r);
    assert!(
        out.contains("7 rows affected"),
        "expected affected-row count, got: {out}"
    );
    assert!(out.contains("(0.523s)"), "expected seconds, got: {out}");
    assert!(!out.contains("---"));
}

#[test]
fn format_table_when_rows_returned() {
    let r = make_result(vec!["id", "name"], vec![vec!["1", "alice"]], "INSERT 1");
    let out = format_execute_result(&r);
    assert!(out.contains("id"));
    assert!(out.contains("name"));
    assert!(out.contains("alice"));
    assert!(
        out.contains("1 row affected"),
        "expected singular form, got: {out}"
    );
    assert!(out.contains("(0.012s)"), "expected seconds, got: {out}");
    let table_pos = out.find("alice").expect("table row should appear");
    let summary_pos = out
        .find("1 row affected")
        .expect("summary should appear");
    assert!(
        table_pos < summary_pos,
        "summary must follow the table, got: {out}"
    );
}

#[test]
fn format_zero_affected_rows_uses_plural() {
    // Common operator scenario: UPDATE ... WHERE that matches no rows. The
    // contract is "0 rows affected" (plural), not "0 row affected".
    let r = ExecuteResult {
        database_name: "testdb".to_string(),
        command_tag: "UPDATE 0".to_string(),
        affected_rows: Some(0),
        columns: Vec::new(),
        rows: Vec::new(),
        execution_time: Duration::from_millis(7),
    };
    let out = format_execute_result(&r);
    assert_eq!(out, "0 rows affected (0.007s)\n");
}

#[test]
fn format_ddl_command_tag_no_count() {
    // DDL like CREATE has no meaningful row count — affected_rows is None,
    // so the summary falls back to the command tag plus elapsed time.
    let r = ExecuteResult {
        database_name: "testdb".to_string(),
        command_tag: "CREATE".to_string(),
        affected_rows: None,
        columns: Vec::new(),
        rows: Vec::new(),
        execution_time: Duration::from_millis(45),
    };
    let out = format_execute_result(&r);
    assert_eq!(out, "CREATE (0.045s)\n");
}

#[test]
fn format_elapsed_always_in_seconds_with_three_decimals() {
    let cases = [
        (Duration::from_millis(0), "0.000s"),
        (Duration::from_millis(1), "0.001s"),
        (Duration::from_millis(523), "0.523s"),
        (Duration::from_millis(1234), "1.234s"),
    ];
    for (dur, expected) in cases {
        let r = ExecuteResult {
            database_name: "testdb".to_string(),
            command_tag: "UPDATE 3".to_string(),
            affected_rows: Some(3),
            columns: Vec::new(),
            rows: Vec::new(),
            execution_time: dur,
        };
        let out = format_execute_result(&r);
        assert!(
            out.contains(expected),
            "duration {dur:?} should render as {expected}, got: {out}"
        );
    }
}
