use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub max_length: Option<i32>,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub row_count: i64,
    pub total_size: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key_columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SchemaResult {
    pub database_name: String,
    pub tables: Vec<TableInfo>,
}

pub fn dump_schema(
    conn: &mut LiveConnection,
    schema_filter: Option<&str>,
) -> Result<SchemaResult, DatabaseCliError> {
    let schema = schema_filter.unwrap_or("public");

    // Tables with row counts and sizes
    let table_rows = conn.client.query(
        "SELECT schemaname, relname, n_live_tup, \
         pg_size_pretty(pg_total_relation_size(schemaname || '.' || relname)) \
         FROM pg_stat_user_tables \
         WHERE schemaname = $1 \
         ORDER BY relname",
        &[&schema],
    )?;

    let mut tables: Vec<TableInfo> = table_rows
        .iter()
        .map(|row| {
            let schema_name: String = row.get(0);
            let table_name: String = row.get(1);
            let row_count: i64 = row.get(2);
            let total_size: String = row.get(3);
            TableInfo {
                schema: schema_name,
                name: table_name,
                row_count,
                total_size,
                columns: Vec::new(),
                primary_key_columns: Vec::new(),
            }
        })
        .collect();

    // Columns
    let col_rows = conn.client.query(
        "SELECT table_name, column_name, data_type, \
         character_maximum_length, is_nullable, column_default \
         FROM information_schema.columns \
         WHERE table_schema = $1 \
         ORDER BY table_name, ordinal_position",
        &[&schema],
    )?;

    for row in &col_rows {
        let table_name: String = row.get(0);
        let column_name: String = row.get(1);
        let data_type: String = row.get(2);
        let max_length: Option<i32> = row.get(3);
        let is_nullable_str: String = row.get(4);
        let default_value: Option<String> = row.get(5);

        if let Some(table) = tables.iter_mut().find(|t| t.name == table_name) {
            table.columns.push(ColumnInfo {
                name: column_name,
                data_type,
                max_length,
                is_nullable: is_nullable_str == "YES",
                default_value,
            });
        }
    }

    // Primary keys
    let pk_rows = conn.client.query(
        "SELECT tc.table_name, kcu.column_name \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
             ON tc.constraint_name = kcu.constraint_name \
             AND tc.table_schema = kcu.table_schema \
         WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 \
         ORDER BY tc.table_name, kcu.ordinal_position",
        &[&schema],
    )?;

    for row in &pk_rows {
        let table_name: String = row.get(0);
        let column_name: String = row.get(1);
        if let Some(table) = tables.iter_mut().find(|t| t.name == table_name) {
            table.primary_key_columns.push(column_name);
        }
    }

    Ok(SchemaResult {
        database_name: conn.config.name.clone(),
        tables,
    })
}

pub fn format_schema(result: &SchemaResult) -> String {
    if result.tables.is_empty() {
        return format!("[{}] No tables found.\n", result.database_name);
    }

    let mut out = String::new();
    out.push_str(&format!("=== {} ===\n\n", result.database_name));

    for table in &result.tables {
        out.push_str(&format!(
            "{}.{}  ({} rows, {})\n",
            table.schema, table.name, table.row_count, table.total_size
        ));

        let pk_set: std::collections::HashSet<&str> = table
            .primary_key_columns
            .iter()
            .map(|s| s.as_str())
            .collect();

        for col in &table.columns {
            let pk_marker = if pk_set.contains(col.name.as_str()) {
                " PK"
            } else {
                ""
            };
            let nullable = if col.is_nullable {
                " NULL"
            } else {
                " NOT NULL"
            };
            let type_str = match col.max_length {
                Some(len) => format!("{}({})", col.data_type, len),
                None => col.data_type.clone(),
            };
            out.push_str(&format!(
                "  {:<30} {:<20}{}{}\n",
                col.name, type_str, nullable, pk_marker
            ));
        }
        out.push('\n');
    }

    out
}
