use std::fmt;

use crate::commands::validate_identifier;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub enum TrendInterval {
    Day,
    Week,
    Month,
    Year,
}

impl TrendInterval {
    pub fn parse_interval(s: &str) -> Result<Self, DatabaseCliError> {
        match s.to_lowercase().as_str() {
            "day" => Ok(TrendInterval::Day),
            "week" => Ok(TrendInterval::Week),
            "month" => Ok(TrendInterval::Month),
            "year" => Ok(TrendInterval::Year),
            other => Err(DatabaseCliError::InvalidInterval(other.to_string())),
        }
    }

    fn as_pg_str(&self) -> &'static str {
        match self {
            TrendInterval::Day => "day",
            TrendInterval::Week => "week",
            TrendInterval::Month => "month",
            TrendInterval::Year => "year",
        }
    }
}

impl fmt::Display for TrendInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_pg_str())
    }
}

#[derive(Debug, Clone)]
pub struct TrendParams {
    pub table: String,
    pub schema: String,
    pub timestamp_column: String,
    pub interval: TrendInterval,
    pub value_column: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TrendRow {
    pub period: String,
    pub count: i64,
    pub avg_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TrendResult {
    pub database_name: String,
    pub table: String,
    pub interval: TrendInterval,
    pub rows: Vec<TrendRow>,
}

pub fn compute_trend(
    conn: &mut LiveConnection,
    params: &TrendParams,
) -> Result<TrendResult, DatabaseCliError> {
    validate_identifier(&params.table)?;
    validate_identifier(&params.schema)?;
    validate_identifier(&params.timestamp_column)?;
    if let Some(ref col) = params.value_column {
        validate_identifier(col)?;
    }

    let interval_str = params.interval.as_pg_str();
    let schema = &params.schema;
    let table = &params.table;
    let ts_col = &params.timestamp_column;

    let sql = if let Some(ref val_col) = params.value_column {
        let limit_clause = params
            .limit
            .map(|l| format!(" LIMIT {l}"))
            .unwrap_or_default();
        format!(
            "SELECT date_trunc('{interval_str}', {ts_col})::text AS period, \
                    COUNT(*)::bigint AS count, \
                    AVG({val_col})::numeric(20,4)::text AS avg_value \
             FROM {schema}.{table} \
             WHERE {ts_col} IS NOT NULL \
             GROUP BY period ORDER BY period{limit_clause}"
        )
    } else {
        let limit_clause = params
            .limit
            .map(|l| format!(" LIMIT {l}"))
            .unwrap_or_default();
        format!(
            "SELECT date_trunc('{interval_str}', {ts_col})::text AS period, \
                    COUNT(*)::bigint AS count \
             FROM {schema}.{table} \
             WHERE {ts_col} IS NOT NULL \
             GROUP BY period ORDER BY period{limit_clause}"
        )
    };

    let query_rows = conn.client.query(&sql, &[])?;

    let rows: Vec<TrendRow> = if params.value_column.is_some() {
        query_rows
            .iter()
            .map(|r| TrendRow {
                period: r.get::<_, Option<String>>(0).unwrap_or_default(),
                count: r.get(1),
                avg_value: r.get::<_, Option<String>>(2),
            })
            .collect()
    } else {
        query_rows
            .iter()
            .map(|r| TrendRow {
                period: r.get::<_, Option<String>>(0).unwrap_or_default(),
                count: r.get(1),
                avg_value: None,
            })
            .collect()
    };

    Ok(TrendResult {
        database_name: conn.config.name.clone(),
        table: format!("{schema}.{table}"),
        interval: params.interval.clone(),
        rows,
    })
}

pub fn format_trend(result: &TrendResult) -> String {
    if result.rows.is_empty() {
        return format!(
            "[{}] {} — no data for {} intervals.\n",
            result.database_name, result.table, result.interval
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "[{}] {} — by {}\n\n",
        result.database_name, result.table, result.interval
    ));

    let has_avg = result.rows.iter().any(|r| r.avg_value.is_some());

    let period_w = result
        .rows
        .iter()
        .map(|r| r.period.len())
        .max()
        .unwrap_or(6)
        .max(6);

    // Header
    if has_avg {
        out.push_str(&format!(
            "  {:<period_w$}  {:>10}  {:>14}\n",
            "Period", "Count", "Avg"
        ));
        out.push_str(&format!("  {:-<period_w$}  {:->10}  {:->14}\n", "", "", ""));
    } else {
        out.push_str(&format!("  {:<period_w$}  {:>10}\n", "Period", "Count"));
        out.push_str(&format!("  {:-<period_w$}  {:->10}\n", "", ""));
    }

    // Find max count for bar chart
    let max_count = result
        .rows
        .iter()
        .map(|r| r.count)
        .max()
        .unwrap_or(1)
        .max(1);

    for row in &result.rows {
        let bar_len = ((row.count as f64 / max_count as f64) * 20.0) as usize;
        let bar = "█".repeat(bar_len);

        if has_avg {
            let avg = row.avg_value.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "  {:<period_w$}  {:>10}  {:>14}  {bar}\n",
                row.period, row.count, avg
            ));
        } else {
            out.push_str(&format!(
                "  {:<period_w$}  {:>10}  {bar}\n",
                row.period, row.count
            ));
        }
    }

    out.push('\n');
    out
}
