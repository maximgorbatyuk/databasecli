use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct TableSummaryRow {
    pub table_name: String,
    pub row_count: i64,
    pub total_size: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseSummary {
    pub database_name: String,
    pub table_count: i64,
    pub total_rows: i64,
    pub total_size: String,
    pub index_count: i64,
    pub largest_tables: Vec<TableSummaryRow>,
}

pub fn summarize(conn: &mut LiveConnection) -> Result<DatabaseSummary, DatabaseCliError> {
    // Table count
    let table_count: i64 = conn
        .client
        .query(
            "SELECT COUNT(*)::bigint FROM information_schema.tables \
             WHERE table_schema NOT IN ('pg_catalog', 'information_schema')",
            &[],
        )?
        .first()
        .map(|r| r.get(0))
        .unwrap_or(0);

    // Tables with row counts and sizes, ordered by size desc
    let table_rows = conn.client.query(
        "SELECT schemaname || '.' || relname AS table_name, \
                n_live_tup AS row_count, \
                pg_total_relation_size(schemaname || '.' || relname) AS total_bytes, \
                pg_size_pretty(pg_total_relation_size(schemaname || '.' || relname)) AS total_size \
         FROM pg_stat_user_tables \
         ORDER BY pg_total_relation_size(schemaname || '.' || relname) DESC",
        &[],
    )?;

    let mut total_rows: i64 = 0;
    let mut largest_tables = Vec::new();

    for (i, row) in table_rows.iter().enumerate() {
        let table_name: String = row.get(0);
        let row_count: i64 = row.get(1);
        let total_size: String = row.get(3);

        total_rows += row_count;

        if i < 10 {
            largest_tables.push(TableSummaryRow {
                table_name,
                row_count,
                total_size,
            });
        }
    }

    // Database size
    let total_size: String = conn
        .client
        .query(
            "SELECT pg_size_pretty(pg_database_size(current_database()))",
            &[],
        )?
        .first()
        .and_then(|r| r.get::<_, Option<String>>(0))
        .unwrap_or_else(|| "-".to_string());

    // Index count
    let index_count: i64 = conn
        .client
        .query(
            "SELECT COUNT(*)::bigint FROM pg_indexes \
             WHERE schemaname NOT IN ('pg_catalog', 'information_schema')",
            &[],
        )?
        .first()
        .map(|r| r.get(0))
        .unwrap_or(0);

    Ok(DatabaseSummary {
        database_name: conn.config.name.clone(),
        table_count,
        total_rows,
        total_size,
        index_count,
        largest_tables,
    })
}

pub fn format_summary(summary: &DatabaseSummary) -> String {
    let mut out = String::new();
    out.push_str(&format!("=== {} ===\n\n", summary.database_name));
    out.push_str(&format!("  Database size:  {}\n", summary.total_size));
    out.push_str(&format!("  Tables:         {}\n", summary.table_count));
    out.push_str(&format!("  Total rows:     {}\n", summary.total_rows));
    out.push_str(&format!("  Indexes:        {}\n", summary.index_count));

    if !summary.largest_tables.is_empty() {
        out.push_str("\n  Largest tables:\n");

        let name_w = summary
            .largest_tables
            .iter()
            .map(|t| t.table_name.len())
            .max()
            .unwrap_or(10)
            .max(10);

        out.push_str(&format!(
            "  {:<name_w$}  {:>12}  {:>10}\n",
            "Table", "Rows", "Size"
        ));
        out.push_str(&format!("  {:-<name_w$}  {:->12}  {:->10}\n", "", "", ""));

        for t in &summary.largest_tables {
            out.push_str(&format!(
                "  {:<name_w$}  {:>12}  {:>10}\n",
                t.table_name, t.row_count, t.total_size
            ));
        }
    }

    out.push('\n');
    out
}
