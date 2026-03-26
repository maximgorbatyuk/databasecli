use crate::commands::validate_identifier;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct ColumnProfile {
    pub name: String,
    pub data_type: String,
    pub total_rows: i64,
    pub non_null_count: i64,
    pub null_count: i64,
    pub null_pct: f64,
    pub distinct_count: i64,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub avg_value: Option<String>,
    pub top_values: Vec<(String, i64)>,
}

#[derive(Debug, Clone)]
pub struct TableProfile {
    pub database_name: String,
    pub schema: String,
    pub table: String,
    pub total_rows: i64,
    pub columns: Vec<ColumnProfile>,
}

pub fn analyze_table(
    conn: &mut LiveConnection,
    table: &str,
    schema: Option<&str>,
) -> Result<TableProfile, DatabaseCliError> {
    let schema = schema.unwrap_or("public");
    validate_identifier(table)?;
    validate_identifier(schema)?;

    // Get columns and their types
    let col_rows = conn.client.query(
        "SELECT column_name, data_type \
         FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = $2 \
         ORDER BY ordinal_position",
        &[&schema, &table],
    )?;

    if col_rows.is_empty() {
        return Err(DatabaseCliError::TableNotFound {
            schema: schema.to_string(),
            table: table.to_string(),
        });
    }

    // Get total row count
    let count_sql = format!("SELECT COUNT(*) FROM {schema}.{table}");
    let count_row = conn.client.query(&count_sql, &[])?;
    let total_rows: i64 = count_row.first().map(|r| r.get(0)).unwrap_or(0);

    let mut columns = Vec::new();

    for col_row in &col_rows {
        let col_name: String = col_row.get(0);
        let data_type: String = col_row.get(1);
        validate_identifier(&col_name)?;

        // Null/distinct stats
        let stats_sql = format!(
            "SELECT \
                COUNT({col_name})::bigint AS non_null_count, \
                (COUNT(*) - COUNT({col_name}))::bigint AS null_count, \
                COUNT(DISTINCT {col_name})::bigint AS distinct_count \
             FROM {schema}.{table}"
        );
        let stats_rows = conn.client.query(&stats_sql, &[])?;

        let (non_null_count, null_count, distinct_count) = if let Some(row) = stats_rows.first() {
            let non_null: i64 = row.get(0);
            let nulls: i64 = row.get(1);
            let distinct: i64 = row.get(2);
            (non_null, nulls, distinct)
        } else {
            (0, 0, 0)
        };

        let null_pct = if total_rows > 0 {
            (null_count as f64 / total_rows as f64) * 100.0
        } else {
            0.0
        };

        // Min/max/avg for numeric types
        let is_numeric = matches!(
            data_type.as_str(),
            "integer" | "bigint" | "smallint" | "numeric" | "real" | "double precision" | "decimal"
        );

        let (min_value, max_value, avg_value) = if is_numeric {
            let minmax_sql = format!(
                "SELECT MIN({col_name})::text, MAX({col_name})::text, \
                 AVG({col_name}::numeric)::text FROM {schema}.{table}"
            );
            match conn.client.query(&minmax_sql, &[]) {
                Ok(rows) if !rows.is_empty() => {
                    let row = &rows[0];
                    (
                        row.get::<_, Option<String>>(0),
                        row.get::<_, Option<String>>(1),
                        row.get::<_, Option<String>>(2),
                    )
                }
                _ => (None, None, None),
            }
        } else {
            // Min/max as text for non-numeric
            let minmax_sql = format!(
                "SELECT MIN({col_name})::text, MAX({col_name})::text FROM {schema}.{table}"
            );
            match conn.client.query(&minmax_sql, &[]) {
                Ok(rows) if !rows.is_empty() => {
                    let row = &rows[0];
                    (
                        row.get::<_, Option<String>>(0),
                        row.get::<_, Option<String>>(1),
                        None,
                    )
                }
                _ => (None, None, None),
            }
        };

        // Top 10 values
        let top_sql = format!(
            "SELECT {col_name}::text AS value, COUNT(*) AS freq \
             FROM {schema}.{table} WHERE {col_name} IS NOT NULL \
             GROUP BY {col_name} ORDER BY freq DESC LIMIT 10"
        );
        let top_values: Vec<(String, i64)> = match conn.client.query(&top_sql, &[]) {
            Ok(rows) => rows
                .iter()
                .filter_map(|r| {
                    let val: Option<String> = r.get(0);
                    let freq: i64 = r.get(1);
                    val.map(|v| (v, freq))
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        columns.push(ColumnProfile {
            name: col_name,
            data_type,
            total_rows,
            non_null_count,
            null_count,
            null_pct,
            distinct_count,
            min_value,
            max_value,
            avg_value,
            top_values,
        });
    }

    Ok(TableProfile {
        database_name: conn.config.name.clone(),
        schema: schema.to_string(),
        table: table.to_string(),
        total_rows,
        columns,
    })
}

pub fn format_table_profile(profile: &TableProfile) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "[{}] {}.{} — {} rows\n\n",
        profile.database_name, profile.schema, profile.table, profile.total_rows
    ));

    for col in &profile.columns {
        out.push_str(&format!("  {} ({})\n", col.name, col.data_type));
        out.push_str(&format!(
            "    Nulls: {}/{} ({:.1}%)  Distinct: {}\n",
            col.null_count, col.total_rows, col.null_pct, col.distinct_count
        ));
        if let Some(ref min) = col.min_value {
            out.push_str(&format!("    Min: {min}"));
            if let Some(ref max) = col.max_value {
                out.push_str(&format!("  Max: {max}"));
            }
            if let Some(ref avg) = col.avg_value {
                out.push_str(&format!("  Avg: {avg}"));
            }
            out.push('\n');
        }
        if !col.top_values.is_empty() {
            out.push_str("    Top values:\n");
            for (val, freq) in &col.top_values {
                let truncated = if val.len() > 40 {
                    format!("{}...", &val[..37])
                } else {
                    val.clone()
                };
                out.push_str(&format!("      {truncated:<40} {freq}\n"));
            }
        }
        out.push('\n');
    }

    out
}
