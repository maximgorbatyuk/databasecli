use crate::commands::query::cell_to_string;
use crate::commands::validate_identifier;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct SampleResult {
    pub database_name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows_in_table: i64,
    pub rows_returned: usize,
}

pub fn sample_table(
    conn: &mut LiveConnection,
    table: &str,
    schema: Option<&str>,
    limit: Option<i64>,
    order_by: Option<&str>,
) -> Result<SampleResult, DatabaseCliError> {
    let schema = schema.unwrap_or("public");
    validate_identifier(table)?;
    validate_identifier(schema)?;
    if let Some(col) = order_by {
        validate_identifier(col)?;
    }

    let limit = limit.unwrap_or(20);

    // Get approximate row count
    let count_rows = conn.client.query(
        "SELECT n_live_tup FROM pg_stat_user_tables \
         WHERE schemaname = $1 AND relname = $2",
        &[&schema, &table],
    )?;

    let total_rows: i64 = count_rows.first().map(|r| r.get(0)).unwrap_or(0);

    // Build query
    let sql = match order_by {
        Some(col) => format!("SELECT * FROM {schema}.{table} ORDER BY {col} DESC LIMIT {limit}"),
        None => format!("SELECT * FROM {schema}.{table} LIMIT {limit}"),
    };

    let rows = conn.client.query(&sql, &[])?;

    let columns: Vec<String> = if let Some(first) = rows.first() {
        first
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect()
    } else {
        Vec::new()
    };

    let rows_returned = rows.len();
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| cell_to_string(row, i))
                .collect()
        })
        .collect();

    Ok(SampleResult {
        database_name: conn.config.name.clone(),
        table: format!("{schema}.{table}"),
        columns,
        rows: data,
        total_rows_in_table: total_rows,
        rows_returned,
    })
}

pub fn format_sample(result: &SampleResult) -> String {
    if result.columns.is_empty() {
        return format!(
            "[{}] {} — table is empty.\n",
            result.database_name, result.table
        );
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
    out.push_str(&format!(
        "[{}] {} — {} of ~{} rows\n\n",
        result.database_name, result.table, result.rows_returned, result.total_rows_in_table
    ));

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

    out
}
