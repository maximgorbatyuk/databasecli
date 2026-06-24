# databasecli — Issue Report & Fix Plan

**Tool:** `databasecli` (CLI) + `databasecli-mcp` (MCP server)
**Version observed:** `databasecli 0.1.8`
**Date:** 2026-06-12
**Reporter context:** Used the tool (both MCP and CLI surfaces) to take a data-only snapshot of a sandbox PostgreSQL DB. Hit four blocking/serious issues and two minor ones. All are reproducible; exact commands are in the Appendix.

---

## Background — what I was doing

Goal: dump all tables of an Organizations sandbox DB (`localhost:5443`, PostgreSQL 15.12, ~28k rows across 8 tables) to a file, excluding two tables, **data-only**.

The databasecli MCP is read-only (SELECT only) and can't emit a dump directly, so the plan was: drive the **`databasecli` CLI** from a script, generating `INSERT` statements server-side via a `SELECT 'INSERT ...' || quote_nullable(col::text) ...` query, and redirect stdout to a `.sql` file (so data goes disk-to-disk, not through the agent context).

This worked for 7 of 8 tables. It then failed hard on the `roles` table, and along the way exposed several issues. I ultimately abandoned databasecli and used `pg_dump` instead (worked first try, see Appendix).

---

## Summary of issues

| # | Severity | Surface | Issue |
|---|----------|---------|-------|
| 1 | **Critical** | CLI | Panic (`Formatting argument out of range`) when a result cell is very wide |
| 2 | **High** | CLI | Result truncation at `query_limit` is **silent** (no notice) — MCP handles this correctly, CLI does not |
| 3 | **High** | CLI | No machine-readable output format (`--format csv/json`, `--no-header`, metadata-to-stderr) — only a padded ASCII table |
| 4 | Medium | CLI + MCP | `statement_timeout` is fixed at 30s with no override; large/expensive read queries get canceled |
| 5 | Medium (feature) | CLI | No native `export`/`dump` command — users must hand-roll generator SQL |
| 6 | Low | MCP | Malformed error message for multi-statement / trailing-semicolon queries |

Suggested fix order: **1 → 3 → 2 → 4 → 6 → 5** (1 is a crash; 3 unblocks the whole use case; 2 prevents silent data loss).

---

## Issue 1 — Panic on wide result cell (CRITICAL)

**Context.** The `roles` table has a `Permissions` (jsonb) column averaging ~35 KB/row, max ~280 KB. Selecting it (or any single wide value) crashes the CLI instead of printing or erroring gracefully.

**Reproduction (minimal, no real data needed):**
```
databasecli --db <any> query "SELECT repeat('A',300000) AS big"
```

**Observed:**
```
thread 'main' (…) panicked at crates/databasecli-core/src/commands/query.rs:270:23:
Formatting argument out of range
stack backtrace:
   1: core::panicking::panic_fmt
   2: databasecli_core::commands::query::format_query_result
   3: databasecli::run::run_query
   4: databasecli::main
```
Process exits with code `101` (Rust panic). This is independent of row count and timeout — a single wide cell triggers it.

**Root-cause hypothesis.** In `format_query_result` (`query.rs:270`) a column/cell display width is computed from the data and fed to a formatting facility that rejects it — e.g. a dynamic width/precision (`{:width$}` / `{:.*}`-style) or a width cast to a narrower integer type that goes out of range for a ~300k-wide value. The panic string `Formatting argument out of range` points at the formatting layer, and the backtrace shows the panic originates directly in `format_query_result`, not a table-rendering dependency.

**Expected fix.**
- Never crash on wide content. Clamp the computed display width to a sane bound before formatting.
- Truncate over-wide cells in the table view with an ellipsis (e.g. default cap 200–500 chars) and indicate truncation, configurable via `--max-col-width N` (`0` = unlimited).
- Add a regression test asserting `SELECT repeat('A',300000)` renders without panic.
- The proper path for genuinely large values is a non-table output mode — see Issue 3.

---

## Issue 2 — Silent truncation in the CLI (HIGH)

**Context.** `query_limit` defaults to 500 (`[settings] query_limit = 500`, `0 = unlimited`). When a result exceeds it, the **CLI gives no indication** — it just prints the footer `500 row(s) (…ms)`, which reads exactly like a complete 500-row result. During the dump, every large table silently came back as exactly 500 rows; I only caught it because the counts were suspiciously uniform. This is a real correctness/data-loss trap.

**Reproduction:**
```
databasecli --db <any> query "SELECT generate_series(1,1200) AS n"
# → footer says "500 row(s) (…ms)", no warning that 700 rows were dropped
```

**Important contrast — the MCP already does this right.** The same query over MCP returns:
```json
{ "row_count": 500, "truncated": true,
  "truncation_notice": "Result set exceeded the configured query_limit of 500 rows. Only the first 500 rows are returned. To retrieve additional rows, use SQL pagination with LIMIT and OFFSET (…)." }
```

**Expected fix.**
- Bring the CLI to parity with the MCP: when a result is capped, print a clear warning **to stderr** (so it doesn't corrupt piped stdout), e.g.
  `⚠ Output truncated to query_limit=500 (more rows exist). Use LIMIT/OFFSET, --limit, or raise [settings] query_limit.`
- Ideally detect "more rows exist" by fetching `limit + 1` and reporting `500+`.
- Improve discoverability of `query_limit` (it's currently only in `databasecli reference`); consider a `--limit N` flag on `query` for ad-hoc overrides (note: `sample`/`trend` already have `--limit`, `query` does not).

---

## Issue 3 — No machine-readable output format (HIGH)

**Context.** `databasecli query` only emits a human-oriented, space-padded ASCII table: a header row, a dashes separator, right-padded data rows, then a `N row(s) (Xms)` footer — all on stdout. There is no `--format`, `--no-header`, or `--tuples-only`. (For comparison, `erd` supports `--format ascii|mermaid|dot`, but `query` has nothing.)

This makes the CLI hostile to scripting:
- Header (2 lines) + footer must be stripped by the caller.
- Values are right-padded with spaces (trailing whitespace).
- **Embedded newlines/tabs in a value break line/column parsing** — a value containing `\n` is rendered across multiple physical lines with no way to tell it apart from a row boundary.
- Timing/metadata (`N row(s) (Xms)`) is mixed into stdout.

To dump data I had to encode newlines server-side (`replace(quote_nullable(col::text), chr(10), '''||chr(10)||''')`) just to keep each logical row on one physical line — a workaround that should be unnecessary.

**Expected fix.**
- Add `--format table|csv|tsv|json|ndjson` to `query` (and ideally `sample`). CSV with RFC 4180 quoting solves the newline/quote/tab problems cleanly; `ndjson`/`json` are ideal for programmatic consumers.
- Add `--no-header` / `--tuples-only`.
- Emit timing and the `N row(s)` summary to **stderr**, keeping stdout pure data.
- This one feature alone would have made the entire dump trivial (`databasecli query "SELECT * FROM t" --format csv > t.csv`).

---

## Issue 4 — Fixed 30s statement_timeout, no override (MEDIUM)

**Context.** Every connection runs `SET statement_timeout = '30s'` (per `databasecli reference`). Building the full `roles` result in one query (8805 rows × large jsonb) exceeded 30s and was canceled:
```
Error: database error: db error
Caused by: ERROR: canceling statement due to statement timeout
```
I had to paginate with small `LIMIT`/`OFFSET` pages to stay under the timeout. There appears to be no flag or `[settings]` key to change it.

**Expected fix.**
- Make it configurable: `[settings] statement_timeout = "30s"` and/or a per-invocation `--timeout <duration>` (CLI) / `timeout` param (MCP).
- Allow disabling for read-only sessions (`0`/`disable`), since the tool is read-only by design and long analytical scans are legitimate.
- Document the default and the override in `reference`.

---

## Issue 5 — No native export/dump command (MEDIUM, feature)

**Context.** The tool is a read-only DB explorer, but there's no first-class way to export query/table data. Users must hand-build `SELECT 'INSERT…'` generators or scrape the table view — which is exactly where Issues 1–3 bite.

**Expected fix.**
- Add `databasecli export <table | --query SQL> --format csv|jsonl|sql [--output FILE]`.
- **Stream via a server-side cursor** (`DECLARE … FETCH` in batches). Streaming neatly sidesteps three other issues at once: no `query_limit` cap to fight, no 30s timeout (work is chunked), and no need to render a giant table (no Issue-1 panic).
- This fits the read-only safety model and would be the natural, supported answer to "dump this DB".

---

## Issue 6 — Malformed multi-statement error message (LOW)

**Context.** Sending a single statement with a trailing `;` to the MCP `query` returns a garbled message that splices the rule description into a "statement begins with '…'" template:
```
Error: read-only violation: statement begins with 'multi-statement queries (containing ';') are not allowed' which is not allowed
```

**Expected fix.**
- Fix the message, e.g. `Multi-statement queries (containing ';') are not allowed — submit one statement at a time.`
- Consider tolerating a single trailing semicolon (strip it) so that `SELECT … ;` — a near-universal habit — just works.

---

## What worked instead (for reference)

`pg_dump` handled the same job with no issues:
```
pg_dump -h localhost -p 5443 -U devops -d organizations-sandbox \
  --data-only --no-owner --no-privileges -Fc \
  --exclude-table=public.backgroundjobs --exclude-table=public.consumedmessages \
  -f out.dump
```
It dumped all 8 tables including the ~300 MB (uncompressed) `roles` table, compressed to 18.3 MB, in seconds. The gap this report is really about: databasecli has no comparable streaming/export path, and its interactive table renderer is being asked to do a job it isn't built for (and crashes on).

---

## Appendix — reproduction transcript

```
$ databasecli --version
databasecli 0.1.8

# Issue 1 — panic on wide cell
$ RUST_BACKTRACE=1 databasecli --db <db> query "SELECT repeat('A',300000) AS big"
thread 'main' panicked at crates/databasecli-core/src/commands/query.rs:270:23:
Formatting argument out of range
  … databasecli_core::commands::query::format_query_result …
# exit code 101

# Issue 2 — silent CLI truncation (footer claims 500, no warning)
$ databasecli --db <db> query "SELECT generate_series(1,1200) AS n"
…
500 row(s) (224ms)

# Issue 2 contrast — MCP reports it correctly
MCP query "SELECT generate_series(1,1200) AS n"
→ { "row_count":500, "truncated":true, "truncation_notice":"Result set exceeded the configured query_limit of 500 rows. …" }

# Issue 4 — fixed 30s statement_timeout
$ databasecli --db <db> query "<build 8805 large rows in one query>"
Error: … ERROR: canceling statement due to statement timeout

# relevant settings (from `databasecli reference`)
[settings] query_limit = 500   (0 = unlimited)
Statement timeout = SET statement_timeout = '30s' on every connection
```
