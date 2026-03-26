use crate::commands::validate_identifier;
use crate::connection::LiveConnection;
use crate::error::DatabaseCliError;

#[derive(Debug, Clone)]
pub struct ErdColumn {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
}

#[derive(Debug, Clone)]
pub struct ErdTable {
    pub name: String,
    pub columns: Vec<ErdColumn>,
    pub primary_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub constraint_name: String,
}

#[derive(Debug, Clone)]
pub struct ErdResult {
    pub database_name: String,
    pub schema: String,
    pub tables: Vec<ErdTable>,
    pub foreign_keys: Vec<ForeignKey>,
}

pub fn build_erd(
    conn: &mut LiveConnection,
    schema: Option<&str>,
) -> Result<ErdResult, DatabaseCliError> {
    let schema = schema.unwrap_or("public");
    validate_identifier(schema)?;

    // Columns per table
    let col_rows = conn.client.query(
        "SELECT table_name, column_name, data_type, is_nullable \
         FROM information_schema.columns \
         WHERE table_schema = $1 \
         ORDER BY table_name, ordinal_position",
        &[&schema],
    )?;

    let mut tables: Vec<ErdTable> = Vec::new();
    for row in &col_rows {
        let table_name: String = row.get(0);
        let col_name: String = row.get(1);
        let data_type: String = row.get(2);
        let nullable_str: String = row.get(3);

        let table = if let Some(t) = tables.iter_mut().find(|t| t.name == table_name) {
            t
        } else {
            tables.push(ErdTable {
                name: table_name.clone(),
                columns: Vec::new(),
                primary_keys: Vec::new(),
            });
            tables.last_mut().unwrap()
        };

        table.columns.push(ErdColumn {
            name: col_name,
            data_type,
            is_nullable: nullable_str == "YES",
        });
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
        let col_name: String = row.get(1);
        if let Some(table) = tables.iter_mut().find(|t| t.name == table_name) {
            table.primary_keys.push(col_name);
        }
    }

    // Foreign keys
    let fk_rows = conn.client.query(
        "SELECT tc.table_name, kcu.column_name, \
                ccu.table_name AS to_table, ccu.column_name AS to_column, \
                tc.constraint_name \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
             ON tc.constraint_name = kcu.constraint_name \
             AND tc.table_schema = kcu.table_schema \
         JOIN information_schema.constraint_column_usage ccu \
             ON tc.constraint_name = ccu.constraint_name \
             AND tc.table_schema = ccu.table_schema \
         WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = $1 \
         ORDER BY tc.table_name",
        &[&schema],
    )?;

    let foreign_keys: Vec<ForeignKey> = fk_rows
        .iter()
        .map(|row| ForeignKey {
            from_table: row.get(0),
            from_column: row.get(1),
            to_table: row.get(2),
            to_column: row.get(3),
            constraint_name: row.get(4),
        })
        .collect();

    Ok(ErdResult {
        database_name: conn.config.name.clone(),
        schema: schema.to_string(),
        tables,
        foreign_keys,
    })
}

pub fn format_erd_ascii(result: &ErdResult) -> String {
    if result.tables.is_empty() {
        return format!(
            "[{}] No tables found in schema '{}'.\n",
            result.database_name, result.schema
        );
    }

    let mut out = String::new();
    out.push_str(&format!(
        "[{}] ERD for schema '{}'\n\n",
        result.database_name, result.schema
    ));

    // Draw each table as a box
    for table in &result.tables {
        let pk_set: std::collections::HashSet<&str> =
            table.primary_keys.iter().map(|s| s.as_str()).collect();

        // Find FK columns for this table
        let fk_cols: std::collections::HashSet<&str> = result
            .foreign_keys
            .iter()
            .filter(|fk| fk.from_table == table.name)
            .map(|fk| fk.from_column.as_str())
            .collect();

        // Calculate box width
        let max_col_len = table
            .columns
            .iter()
            .map(|c| {
                let marker_len = if pk_set.contains(c.name.as_str()) {
                    4
                } else {
                    0
                } + if fk_cols.contains(c.name.as_str()) {
                    4
                } else {
                    0
                };
                c.name.len() + c.data_type.len() + 3 + marker_len
            })
            .max()
            .unwrap_or(0);
        let box_width = table.name.len().max(max_col_len).max(10) + 4;

        // Top border
        out.push_str(&format!("  ┌{}┐\n", "─".repeat(box_width)));
        // Table name
        out.push_str(&format!(
            "  │ {:<width$} │\n",
            table.name,
            width = box_width - 2
        ));
        out.push_str(&format!("  ├{}┤\n", "─".repeat(box_width)));

        // Columns
        for col in &table.columns {
            let pk = if pk_set.contains(col.name.as_str()) {
                " PK"
            } else {
                ""
            };
            let fk = if fk_cols.contains(col.name.as_str()) {
                " FK"
            } else {
                ""
            };
            let line = format!("{} : {}{}{}", col.name, col.data_type, pk, fk);
            out.push_str(&format!("  │ {:<width$} │\n", line, width = box_width - 2));
        }

        // Bottom border
        out.push_str(&format!("  └{}┘\n\n", "─".repeat(box_width)));
    }

    // Relationships
    if !result.foreign_keys.is_empty() {
        out.push_str("  Relationships:\n");
        for fk in &result.foreign_keys {
            out.push_str(&format!(
                "    {}.{} ──> {}.{}\n",
                fk.from_table, fk.from_column, fk.to_table, fk.to_column
            ));
        }
        out.push('\n');
    }

    out
}

pub fn format_erd_mermaid(result: &ErdResult) -> String {
    let mut out = String::from("erDiagram\n");

    for table in &result.tables {
        let pk_set: std::collections::HashSet<&str> =
            table.primary_keys.iter().map(|s| s.as_str()).collect();
        let fk_cols: std::collections::HashSet<&str> = result
            .foreign_keys
            .iter()
            .filter(|fk| fk.from_table == table.name)
            .map(|fk| fk.from_column.as_str())
            .collect();

        out.push_str(&format!("    {} {{\n", table.name));
        for col in &table.columns {
            let constraint = if pk_set.contains(col.name.as_str()) {
                " PK"
            } else if fk_cols.contains(col.name.as_str()) {
                " FK"
            } else {
                ""
            };
            // Mermaid uses simplified type names
            let mermaid_type = col.data_type.replace(' ', "_");
            out.push_str(&format!(
                "        {} {}{}\n",
                mermaid_type, col.name, constraint
            ));
        }
        out.push_str("    }\n");
    }

    for fk in &result.foreign_keys {
        out.push_str(&format!(
            "    {} ||--o{{ {} : \"{}\"\n",
            fk.to_table, fk.from_table, fk.from_column
        ));
    }

    out
}

pub fn format_erd_dot(result: &ErdResult) -> String {
    let mut out = String::from("digraph erd {\n    rankdir=LR;\n    node [shape=record];\n\n");

    for table in &result.tables {
        let pk_set: std::collections::HashSet<&str> =
            table.primary_keys.iter().map(|s| s.as_str()).collect();

        let mut fields: Vec<String> = Vec::new();
        for col in &table.columns {
            let pk_marker = if pk_set.contains(col.name.as_str()) {
                "*"
            } else {
                ""
            };
            fields.push(format!("{}{} : {}\\l", pk_marker, col.name, col.data_type));
        }

        out.push_str(&format!(
            "    {} [label=\"{{{} | {}}}\"]\n",
            table.name,
            table.name,
            fields.join("")
        ));
    }

    out.push('\n');

    for fk in &result.foreign_keys {
        out.push_str(&format!(
            "    {} -> {} [label=\"{}\"]\n",
            fk.from_table, fk.to_table, fk.from_column
        ));
    }

    out.push_str("}\n");
    out
}
