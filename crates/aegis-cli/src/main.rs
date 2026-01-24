//! Aegis CLI - Command Line Interface
//!
//! Command-line tool for managing and interacting with Aegis databases.
//! Provides database operations, query execution, and administration commands.
//!
//! @version 0.1.1
//! @author AutomataNexus Development Team

use clap::{Parser, Subcommand};
use comfy_table::{Table, Row, Cell, ContentArrangement};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// =============================================================================
// CLI Arguments
// =============================================================================

#[derive(Parser)]
#[command(name = "aegis")]
#[command(author = "AutomataNexus Development Team")]
#[command(version = "0.1.1")]
#[command(about = "Aegis Database CLI - Multi-paradigm database platform", long_about = None)]
struct Cli {
    /// Server URL (default: http://localhost:9090)
    #[arg(short, long, global = true, default_value = "http://localhost:9090")]
    server: String,

    /// Database connection URL (e.g., aegis://localhost:9090/dbname)
    #[arg(short, long, global = true)]
    database: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a SQL query
    Query {
        /// SQL statement to execute
        sql: String,

        /// Output format (table, json, csv)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Check server health and status
    Status,

    /// Get server metrics
    Metrics,

    /// List all tables
    Tables,

    /// Key-value store operations
    Kv {
        #[command(subcommand)]
        action: KvCommands,
    },

    /// Cluster information
    Cluster,

    /// Interactive SQL shell
    Shell,
}

#[derive(Subcommand)]
enum KvCommands {
    /// Get a key's value
    Get { key: String },
    /// Set a key's value
    Set { key: String, value: String },
    /// Delete a key
    Delete { key: String },
    /// List all keys
    List {
        /// Filter by prefix
        #[arg(short, long)]
        prefix: Option<String>,
    },
}

// =============================================================================
// API Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct MetricsResponse {
    total_requests: u64,
    failed_requests: u64,
    avg_duration_ms: f64,
    success_rate: f64,
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    sql: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    success: bool,
    data: Option<QueryData>,
    error: Option<String>,
    execution_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryData {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    rows_affected: u64,
}

#[derive(Debug, Deserialize)]
struct TablesResponse {
    tables: Vec<TableInfo>,
}

#[derive(Debug, Deserialize)]
struct TableInfo {
    name: String,
    columns: Vec<ColumnInfo>,
    row_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ColumnInfo {
    name: String,
    data_type: String,
    nullable: bool,
}

#[derive(Debug, Deserialize)]
struct ClusterResponse {
    name: String,
    version: String,
    total_nodes: u32,
    healthy_nodes: u32,
    leader_id: String,
}

#[derive(Debug, Deserialize)]
struct KvEntry {
    key: String,
    value: serde_json::Value,
    ttl: Option<u64>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct KvSetRequest {
    value: serde_json::Value,
    ttl: Option<u64>,
}

// =============================================================================
// Main Entry Point
// =============================================================================

/// Parse an aegis:// URL into (server_url, database_name)
fn parse_database_url(url: &str) -> Result<(String, Option<String>), String> {
    if url.starts_with("aegis://") {
        let rest = &url[8..]; // Remove "aegis://"
        if let Some(slash_pos) = rest.find('/') {
            let host_port = &rest[..slash_pos];
            let db_name = &rest[slash_pos + 1..];
            let server = format!("http://{}", host_port);
            let db = if db_name.is_empty() { None } else { Some(db_name.to_string()) };
            Ok((server, db))
        } else {
            Ok((format!("http://{}", rest), None))
        }
    } else if url.starts_with("http://") || url.starts_with("https://") {
        Ok((url.to_string(), None))
    } else {
        Err(format!("Invalid database URL: {}. Expected aegis://host:port/dbname or http(s)://...", url))
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    // Determine server URL - database flag takes precedence
    let (server, _database) = if let Some(ref db_url) = cli.database {
        match parse_database_url(db_url) {
            Ok((s, d)) => (s, d),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        (cli.server.clone(), None)
    };

    let result = match cli.command {
        Commands::Query { sql, format } => execute_query(&client, &server, &sql, &format).await,
        Commands::Status => check_status(&client, &server).await,
        Commands::Metrics => get_metrics(&client, &server).await,
        Commands::Tables => list_tables(&client, &server).await,
        Commands::Kv { action } => handle_kv(&client, &server, action).await,
        Commands::Cluster => get_cluster_info(&client, &server).await,
        Commands::Shell => run_shell(&client, &server).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

// =============================================================================
// Command Implementations
// =============================================================================

async fn execute_query(client: &Client, server: &str, sql: &str, format: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/query", server);
    let request = QueryRequest {
        sql: sql.to_string(),
        params: vec![],
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;

    let query_response: QueryResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if !query_response.success {
        return Err(query_response.error.unwrap_or_else(|| "Unknown error".to_string()));
    }

    if let Some(data) = query_response.data {
        match format {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&data).unwrap());
            }
            "csv" => {
                println!("{}", data.columns.join(","));
                for row in &data.rows {
                    let values: Vec<String> = row.iter().map(|v| format_value(v)).collect();
                    println!("{}", values.join(","));
                }
            }
            _ => {
                // Table format
                if data.columns.is_empty() {
                    println!("Query executed successfully. Rows affected: {}", data.rows_affected);
                } else {
                    let mut table = Table::new();
                    table.set_content_arrangement(ContentArrangement::Dynamic);
                    table.set_header(data.columns.iter().map(|c| Cell::new(c)));

                    for row in &data.rows {
                        let cells: Vec<Cell> = row.iter().map(|v| Cell::new(format_value(v))).collect();
                        table.add_row(Row::from(cells));
                    }

                    println!("{}", table);
                    println!("\n{} row(s) returned in {} ms", data.rows.len(), query_response.execution_time_ms);
                }
            }
        }
    } else {
        println!("Query executed successfully in {} ms", query_response.execution_time_ms);
    }

    Ok(())
}

async fn check_status(client: &Client, server: &str) -> Result<(), String> {
    let url = format!("{}/health", server);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;

    let health: HealthResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    println!("Aegis Database Server");
    println!("---------------------");
    println!("Status:  {}", health.status);
    println!("Version: {}", health.version);
    println!("Server:  {}", server);

    Ok(())
}

async fn get_metrics(client: &Client, server: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/metrics", server);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;

    let metrics: MetricsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    println!("Server Metrics");
    println!("--------------");
    println!("Total Requests:   {}", metrics.total_requests);
    println!("Failed Requests:  {}", metrics.failed_requests);
    println!("Avg Duration:     {:.2} ms", metrics.avg_duration_ms);
    println!("Success Rate:     {:.2}%", metrics.success_rate * 100.0);

    Ok(())
}

async fn list_tables(client: &Client, server: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/tables", server);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;

    let tables_response: TablesResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if tables_response.tables.is_empty() {
        println!("No tables found.");
    } else {
        let mut table = Table::new();
        table.set_content_arrangement(ContentArrangement::Dynamic);
        table.set_header(vec!["Table Name", "Columns", "Row Count"]);

        for t in &tables_response.tables {
            let columns = t.columns.iter()
                .map(|c| {
                    let nullable = if c.nullable { "NULL" } else { "NOT NULL" };
                    format!("{} {} {}", c.name, c.data_type, nullable)
                })
                .collect::<Vec<_>>()
                .join(", ");
            let row_count = t.row_count.map(|r| r.to_string()).unwrap_or_else(|| "N/A".to_string());
            table.add_row(vec![&t.name, &columns, &row_count]);
        }

        println!("{}", table);
    }

    Ok(())
}

async fn get_cluster_info(client: &Client, server: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/admin/cluster", server);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;

    let cluster: ClusterResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    println!("Cluster Information");
    println!("-------------------");
    println!("Name:          {}", cluster.name);
    println!("Version:       {}", cluster.version);
    println!("Total Nodes:   {}", cluster.total_nodes);
    println!("Healthy Nodes: {}", cluster.healthy_nodes);
    println!("Leader ID:     {}", cluster.leader_id);

    Ok(())
}

async fn handle_kv(client: &Client, server: &str, action: KvCommands) -> Result<(), String> {
    match action {
        KvCommands::Get { key } => {
            let url = format!("{}/api/v1/kv/keys/{}", server, key);
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to connect: {}", e))?;

            if response.status().is_success() {
                let entry: KvEntry = response.json().await
                    .map_err(|e| format!("Failed to parse: {}", e))?;
                println!("{}", serde_json::to_string_pretty(&entry.value).unwrap());
            } else if response.status().as_u16() == 404 {
                return Err(format!("Key '{}' not found", key));
            } else {
                return Err(format!("Server error: {}", response.status()));
            }
        }
        KvCommands::Set { key, value } => {
            let url = format!("{}/api/v1/kv/keys/{}", server, key);
            let json_value: serde_json::Value = serde_json::from_str(&value)
                .unwrap_or_else(|_| serde_json::Value::String(value.clone()));

            let request = KvSetRequest {
                value: json_value,
                ttl: None,
            };

            let response = client
                .post(&url)
                .json(&request)
                .send()
                .await
                .map_err(|e| format!("Failed to connect: {}", e))?;

            if response.status().is_success() {
                println!("OK");
            } else {
                return Err(format!("Failed to set key: {}", response.status()));
            }
        }
        KvCommands::Delete { key } => {
            let url = format!("{}/api/v1/kv/keys/{}", server, key);
            let response = client
                .delete(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to connect: {}", e))?;

            if response.status().is_success() {
                println!("Deleted '{}'", key);
            } else if response.status().as_u16() == 404 {
                return Err(format!("Key '{}' not found", key));
            } else {
                return Err(format!("Failed to delete: {}", response.status()));
            }
        }
        KvCommands::List { prefix } => {
            let url = match prefix {
                Some(p) => format!("{}/api/v1/kv/keys?prefix={}", server, p),
                None => format!("{}/api/v1/kv/keys", server),
            };

            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to connect: {}", e))?;

            let entries: Vec<KvEntry> = response.json().await
                .map_err(|e| format!("Failed to parse: {}", e))?;

            if entries.is_empty() {
                println!("No keys found.");
            } else {
                let mut table = Table::new();
                table.set_content_arrangement(ContentArrangement::Dynamic);
                table.set_header(vec!["Key", "Value", "TTL", "Created", "Updated"]);

                for entry in &entries {
                    let value_str = format_value(&entry.value);
                    let truncated = if value_str.len() > 40 {
                        format!("{}...", &value_str[..37])
                    } else {
                        value_str
                    };
                    let ttl_str = entry.ttl.map(|t| format!("{}s", t)).unwrap_or_else(|| "-".to_string());
                    table.add_row(vec![&entry.key, &truncated, &ttl_str, &entry.created_at, &entry.updated_at]);
                }

                println!("{}", table);
            }
        }
    }

    Ok(())
}

async fn run_shell(client: &Client, server: &str) -> Result<(), String> {
    use std::io::{self, Write};

    println!("Aegis SQL Shell");
    println!("Connected to: {}", server);
    println!("Type 'exit' or 'quit' to exit, 'help' for commands.\n");

    let mut input = String::new();
    let stdin = io::stdin();

    loop {
        print!("aegis> ");
        io::stdout().flush().unwrap();

        input.clear();
        if stdin.read_line(&mut input).is_err() {
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        match trimmed.to_lowercase().as_str() {
            "exit" | "quit" | "\\q" => {
                println!("Bye!");
                break;
            }
            "help" | "\\h" | "\\?" => {
                println!("Commands:");
                println!("  \\q, exit, quit  - Exit the shell");
                println!("  \\d              - List tables");
                println!("  \\h, help        - Show this help");
                println!("  Any SQL         - Execute SQL query");
                continue;
            }
            "\\d" => {
                if let Err(e) = list_tables(client, server).await {
                    eprintln!("Error: {}", e);
                }
                continue;
            }
            _ => {}
        }

        // Execute as SQL
        if let Err(e) = execute_query(client, server, trimmed, "table").await {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(_) => serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
    }
}
