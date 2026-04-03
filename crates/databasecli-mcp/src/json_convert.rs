use databasecli_core::commands::analyze::TableProfile;
use databasecli_core::commands::erd::ErdResult;
use databasecli_core::commands::health::EnhancedHealthResult;
use databasecli_core::commands::query::QueryResultSet;
use databasecli_core::commands::sample::SampleResult;
use databasecli_core::commands::schema::SchemaResult;
use databasecli_core::commands::summary::DatabaseSummary;
use databasecli_core::commands::trend::TrendResult;
use databasecli_core::config::DatabaseConfig;
use databasecli_core::health::HealthStatus;
use serde_json::{Value, json};

pub fn config_to_json(config: &DatabaseConfig) -> Value {
    json!({
        "name": config.name,
        "host": config.host,
        "port": config.port,
        "dbname": config.dbname,
        "user": config.user,
    })
}

pub fn query_result_to_json(result: &QueryResultSet) -> Value {
    let mut obj = json!({
        "database": result.database_name,
        "columns": result.columns,
        "rows": result.rows,
        "row_count": result.row_count,
        "execution_time_ms": result.execution_time.as_millis(),
        "truncated": result.truncated,
    });

    if result.truncated {
        obj["truncation_notice"] = json!(format!(
            "Result set exceeded the configured query_limit of {} rows. \
             Only the first {} rows are returned. \
             To retrieve additional rows, use SQL pagination with LIMIT and OFFSET \
             (e.g., SELECT ... LIMIT {} OFFSET {}).",
            result.row_count, result.row_count, result.row_count, result.row_count
        ));
    }

    obj
}

pub fn sample_result_to_json(result: &SampleResult) -> Value {
    json!({
        "database": result.database_name,
        "table": result.table,
        "columns": result.columns,
        "rows": result.rows,
        "total_rows_in_table": result.total_rows_in_table,
        "rows_returned": result.rows_returned,
    })
}

pub fn schema_result_to_json(result: &SchemaResult) -> Value {
    let tables: Vec<Value> = result
        .tables
        .iter()
        .map(|t| {
            let columns: Vec<Value> = t
                .columns
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "data_type": c.data_type,
                        "max_length": c.max_length,
                        "is_nullable": c.is_nullable,
                        "default_value": c.default_value,
                    })
                })
                .collect();

            json!({
                "schema": t.schema,
                "name": t.name,
                "row_count": t.row_count,
                "total_size": t.total_size,
                "columns": columns,
                "primary_key_columns": t.primary_key_columns,
            })
        })
        .collect();

    json!({
        "database": result.database_name,
        "tables": tables,
    })
}

pub fn table_profile_to_json(profile: &TableProfile) -> Value {
    let columns: Vec<Value> = profile
        .columns
        .iter()
        .map(|c| {
            let top_values: Vec<Value> = c
                .top_values
                .iter()
                .map(|(val, freq)| json!({"value": val, "frequency": freq}))
                .collect();

            json!({
                "name": c.name,
                "data_type": c.data_type,
                "total_rows": c.total_rows,
                "non_null_count": c.non_null_count,
                "null_count": c.null_count,
                "null_pct": c.null_pct,
                "distinct_count": c.distinct_count,
                "min_value": c.min_value,
                "max_value": c.max_value,
                "avg_value": c.avg_value,
                "top_values": top_values,
            })
        })
        .collect();

    json!({
        "database": profile.database_name,
        "schema": profile.schema,
        "table": profile.table,
        "total_rows": profile.total_rows,
        "columns": columns,
    })
}

pub fn summary_to_json(summary: &DatabaseSummary) -> Value {
    let largest: Vec<Value> = summary
        .largest_tables
        .iter()
        .map(|t| {
            json!({
                "table_name": t.table_name,
                "row_count": t.row_count,
                "total_size": t.total_size,
            })
        })
        .collect();

    json!({
        "database": summary.database_name,
        "table_count": summary.table_count,
        "total_rows": summary.total_rows,
        "total_size": summary.total_size,
        "index_count": summary.index_count,
        "largest_tables": largest,
    })
}

pub fn trend_result_to_json(result: &TrendResult) -> Value {
    let rows: Vec<Value> = result
        .rows
        .iter()
        .map(|r| {
            json!({
                "period": r.period,
                "count": r.count,
                "avg_value": r.avg_value,
            })
        })
        .collect();

    json!({
        "database": result.database_name,
        "table": result.table,
        "interval": result.interval.to_string(),
        "rows": rows,
    })
}

pub fn enhanced_health_to_json(result: &EnhancedHealthResult) -> Value {
    let status = match result.status {
        HealthStatus::Connected => "connected",
        HealthStatus::Failed => "failed",
    };

    json!({
        "name": result.name,
        "host": result.host,
        "port": result.port,
        "dbname": result.dbname,
        "status": status,
        "response_time_ms": result.response_time.map(|d| d.as_millis()),
        "pg_version": result.pg_version,
        "db_size": result.db_size,
        "uptime": result.uptime,
        "error": result.error,
    })
}

pub fn erd_result_to_json(result: &ErdResult) -> Value {
    let mermaid = databasecli_core::commands::erd::format_erd_mermaid(result);

    let tables: Vec<Value> = result
        .tables
        .iter()
        .map(|t| {
            let columns: Vec<Value> = t
                .columns
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "data_type": c.data_type,
                        "is_nullable": c.is_nullable,
                    })
                })
                .collect();

            json!({
                "name": t.name,
                "columns": columns,
                "primary_keys": t.primary_keys,
            })
        })
        .collect();

    let foreign_keys: Vec<Value> = result
        .foreign_keys
        .iter()
        .map(|fk| {
            json!({
                "from_table": fk.from_table,
                "from_column": fk.from_column,
                "to_table": fk.to_table,
                "to_column": fk.to_column,
                "constraint_name": fk.constraint_name,
            })
        })
        .collect();

    json!({
        "database": result.database_name,
        "schema": result.schema,
        "mermaid": mermaid,
        "tables": tables,
        "foreign_keys": foreign_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_query_result(truncated: bool, row_count: usize) -> QueryResultSet {
        QueryResultSet {
            database_name: "testdb".to_string(),
            columns: vec!["id".to_string(), "name".to_string()],
            rows: (0..row_count)
                .map(|i| vec![i.to_string(), format!("row_{i}")])
                .collect(),
            row_count,
            execution_time: Duration::from_millis(42),
            truncated,
        }
    }

    #[test]
    fn json_includes_truncated_false_without_notice() {
        let result = make_query_result(false, 3);
        let json = query_result_to_json(&result);

        assert_eq!(json["truncated"], false);
        assert!(json.get("truncation_notice").is_none());
        assert_eq!(json["row_count"], 3);
        assert_eq!(json["database"], "testdb");
    }

    #[test]
    fn json_includes_truncation_notice_when_truncated() {
        let result = make_query_result(true, 500);
        let json = query_result_to_json(&result);

        assert_eq!(json["truncated"], true);
        let notice = json["truncation_notice"].as_str().unwrap();
        assert!(notice.contains("query_limit of 500 rows"));
        assert!(notice.contains("LIMIT and OFFSET"));
        assert!(notice.contains("LIMIT 500 OFFSET 500"));
    }

    #[test]
    fn json_truncation_notice_reflects_actual_row_count() {
        let result = make_query_result(true, 100);
        let json = query_result_to_json(&result);

        let notice = json["truncation_notice"].as_str().unwrap();
        assert!(notice.contains("query_limit of 100 rows"));
        assert!(notice.contains("LIMIT 100 OFFSET 100"));
    }

    #[test]
    fn json_empty_result_not_truncated() {
        let result = QueryResultSet {
            database_name: "testdb".to_string(),
            columns: vec![],
            rows: vec![],
            row_count: 0,
            execution_time: Duration::from_millis(1),
            truncated: false,
        };
        let json = query_result_to_json(&result);

        assert_eq!(json["truncated"], false);
        assert_eq!(json["row_count"], 0);
        assert!(json.get("truncation_notice").is_none());
    }
}
