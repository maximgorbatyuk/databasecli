use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};

use crate::state::McpSessionState;
use crate::tools;

// === Parameter structs ===

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectParams {
    /// Database names from config to connect to
    pub names: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DisconnectParams {
    /// Database names to disconnect. Empty array disconnects all.
    pub names: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct QueryParams {
    /// Read-only SQL query (SELECT, WITH, EXPLAIN, SHOW, TABLE)
    pub sql: String,
    /// Database name to query. Omit to use first connected.
    pub database: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SchemaParams {
    /// PostgreSQL schema name. Default: 'public'
    pub schema_name: Option<String>,
    /// Database name. Omit for all connected databases.
    pub database: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SampleParams {
    /// Table name
    pub table: String,
    /// Database name
    pub database: String,
    /// Schema name. Default: 'public'
    pub schema_name: Option<String>,
    /// Max rows. Default: 20
    pub limit: Option<i64>,
    /// Column to ORDER BY DESC
    pub order_by: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CompareParams {
    /// Read-only SQL query to run on all connected databases
    pub sql: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeParams {
    /// Table name
    pub table: String,
    /// Database name
    pub database: String,
    /// Schema name. Default: 'public'
    pub schema_name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SummaryParams {
    /// Database name. Omit for all connected.
    pub database: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ErdParams {
    /// Database name
    pub database: String,
    /// Schema name. Default: 'public'
    pub schema_name: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TrendParams {
    /// Table name
    pub table: String,
    /// Database name
    pub database: String,
    /// Timestamp/date column name
    pub timestamp_column: String,
    /// Grouping interval: day, week, month, year. Default: day
    pub interval: Option<String>,
    /// Optional numeric column to compute AVG
    pub value_column: Option<String>,
    /// Schema name. Default: 'public'
    pub schema_name: Option<String>,
    /// Max number of periods to return
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct MigrationParams {
    /// Database to analyze
    pub database: String,
    /// Natural language description of desired schema changes
    pub description: String,
    /// Schema name. Default: 'public'
    pub schema_name: Option<String>,
}

// === Server ===

#[derive(Clone)]
pub struct DatabaseCliServer {
    tool_router: ToolRouter<Self>,
    pub state: Arc<McpSessionState>,
}

#[tool_router]
impl DatabaseCliServer {
    pub fn new(state: McpSessionState) -> Self {
        Self {
            tool_router: Self::tool_router(),
            state: Arc::new(state),
        }
    }

    #[tool(
        description = "List all databases from the configuration file. Returns name, host, port, dbname, user. Passwords are never exposed. Use this first to discover available databases."
    )]
    fn list_configured_databases(&self) -> Result<CallToolResult, McpError> {
        tools::connection::list_configured(&self.state)
    }

    #[tool(
        description = "Connect to one or more databases by name. Names must match config entries (use list_configured_databases first). Connections persist across tool calls. All connections are read-only."
    )]
    async fn connect_databases(
        &self,
        Parameters(params): Parameters<ConnectParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::connection::connect(&self.state, params.names).await
    }

    #[tool(description = "Disconnect from databases by name. Empty array disconnects all.")]
    async fn disconnect_databases(
        &self,
        Parameters(params): Parameters<DisconnectParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::connection::disconnect(&self.state, params.names).await
    }

    #[tool(description = "List all currently connected databases with connection details.")]
    async fn list_connected_databases(&self) -> Result<CallToolResult, McpError> {
        tools::connection::list_connected(&self.state).await
    }

    #[tool(
        description = "Execute a read-only SQL query. Only SELECT, WITH, EXPLAIN, SHOW, TABLE allowed. Specify database name or omit for first connected."
    )]
    async fn query(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::query::query(&self.state, params.sql, params.database).await
    }

    #[tool(
        description = "Get full schema: tables, columns, data types, primary keys, row counts, sizes. Optionally filter by schema name."
    )]
    async fn schema(
        &self,
        Parameters(params): Parameters<SchemaParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::schema::schema(&self.state, params.schema_name, params.database).await
    }

    #[tool(
        description = "Preview rows from a table. Returns up to N rows (default 20). Optionally order by a column descending."
    )]
    async fn sample(
        &self,
        Parameters(params): Parameters<SampleParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::schema::sample(
            &self.state,
            params.table,
            params.database,
            params.schema_name,
            params.limit,
            params.order_by,
        )
        .await
    }

    #[tool(
        description = "Run the same read-only SQL query across ALL connected databases and compare results. Requires 2+ connections."
    )]
    async fn compare(
        &self,
        Parameters(params): Parameters<CompareParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::query::compare(&self.state, params.sql).await
    }

    #[tool(
        description = "Profile a table: null counts, cardinality, min/max/avg for numerics, top 10 values per column. Useful for data quality assessment."
    )]
    async fn analyze(
        &self,
        Parameters(params): Parameters<AnalyzeParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::analysis::analyze(
            &self.state,
            params.table,
            params.database,
            params.schema_name,
        )
        .await
    }

    #[tool(
        description = "Database overview: total size, table count, row count, index count, and 10 largest tables."
    )]
    async fn summary(
        &self,
        Parameters(params): Parameters<SummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::analysis::summary(&self.state, params.database).await
    }

    #[tool(
        description = "Entity-relationship diagram: tables, columns, primary keys, foreign keys. Returns Mermaid syntax plus structured data."
    )]
    async fn erd(
        &self,
        Parameters(params): Parameters<ErdParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::schema::erd(&self.state, params.database, params.schema_name).await
    }

    #[tool(
        description = "Time-series trend: group rows by timestamp column at day/week/month/year intervals. Returns counts and optional averages."
    )]
    async fn trend(
        &self,
        Parameters(params): Parameters<TrendParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::analysis::trend(
            &self.state,
            tools::analysis::TrendToolParams {
                table: params.table,
                database: params.database,
                timestamp_column: params.timestamp_column,
                interval: params.interval,
                value_column: params.value_column,
                schema_name: params.schema_name,
                limit: params.limit,
            },
        )
        .await
    }

    #[tool(
        description = "Health check for connected databases: PostgreSQL version, database size, server uptime, response time."
    )]
    async fn enhanced_health(&self) -> Result<CallToolResult, McpError> {
        tools::analysis::enhanced_health(&self.state).await
    }

    #[tool(
        description = "Analyze current schema and return context for migration planning. Returns full schema, foreign keys, and your description. NEVER executes DDL. Use this to gather information, then generate migration SQL yourself."
    )]
    async fn suggest_migration(
        &self,
        Parameters(params): Parameters<MigrationParams>,
    ) -> Result<CallToolResult, McpError> {
        tools::migration::suggest_migration(
            &self.state,
            params.database,
            params.description,
            params.schema_name,
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for DatabaseCliServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "databasecli MCP server: a read-only gateway to PostgreSQL databases. \
                 Use list_configured_databases to discover available databases, \
                 connect_databases to establish connections, then use query/schema/sample/analyze/\
                 compare/summary/erd/trend/enhanced_health to explore data. \
                 suggest_migration provides schema context for DDL planning but never executes DDL. \
                 All operations are strictly read-only."
                    .to_string(),
            )
    }
}
