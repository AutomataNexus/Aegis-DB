//! Aegis CLI - Command Line Interface
//!
//! Command-line tool for managing and interacting with Aegis databases.
//! Provides database operations, query execution, and administration commands.
//!
//! @version 0.1.2
//! @author AutomataNexus Development Team

use clap::{Parser, Subcommand};
use comfy_table::{Table, Row, Cell, ContentArrangement};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
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

    /// Database: shorthand (nexusscribe, axonml, dashboard), URL (aegis://host:port/db), or host:port
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

    /// Manage registered nodes/databases
    Nodes {
        #[command(subcommand)]
        action: NodesCommands,
    },
}

#[derive(Subcommand)]
enum NodesCommands {
    /// List all registered nodes
    List,
    /// Add a new node shorthand
    Add {
        /// Shorthand name for the node
        name: String,
        /// Server URL (e.g., http://localhost:9091)
        url: String,
    },
    /// Remove a node shorthand
    Remove {
        /// Shorthand name to remove
        name: String,
    },
    /// Sync nodes from a running cluster
    Sync {
        /// Server to query for cluster nodes (default: http://localhost:9090)
        #[arg(short, long, default_value = "http://localhost:9090")]
        from: String,
    },
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
// Node Registry
// =============================================================================

/// Node registry stored in ~/.aegis/nodes.json
#[derive(Debug, Serialize, Deserialize, Default)]
struct NodeRegistry {
    nodes: HashMap<String, NodeEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NodeEntry {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    added_at: Option<String>,
}

impl NodeRegistry {
    fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".aegis").join("nodes.json")
    }

    fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(registry) = serde_json::from_str(&data) {
                    return registry;
                }
            }
        }
        // Return default with built-in nodes
        let mut registry = NodeRegistry::default();
        registry.add_builtin_nodes();
        registry
    }

    fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        std::fs::write(&path, data)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        Ok(())
    }

    fn add_builtin_nodes(&mut self) {
        // Only add if not already present
        if !self.nodes.contains_key("dashboard") {
            self.nodes.insert("dashboard".to_string(), NodeEntry {
                url: "http://localhost:9090".to_string(),
                node_id: None,
                role: Some("primary".to_string()),
                status: Some("builtin".to_string()),
                added_at: Some("builtin".to_string()),
            });
        }
        if !self.nodes.contains_key("local") {
            self.nodes.insert("local".to_string(), NodeEntry {
                url: "http://localhost:9090".to_string(),
                node_id: None,
                role: Some("primary".to_string()),
                status: Some("builtin".to_string()),
                added_at: Some("builtin".to_string()),
            });
        }
    }

    fn get(&self, name: &str) -> Option<&NodeEntry> {
        self.nodes.get(&name.to_lowercase())
    }

    fn add(&mut self, name: &str, url: &str, node_id: Option<String>, role: Option<String>, status: Option<String>) {
        self.nodes.insert(name.to_lowercase(), NodeEntry {
            url: url.to_string(),
            node_id,
            role,
            status,
            added_at: Some(chrono::Utc::now().to_rfc3339()),
        });
    }

    fn remove(&mut self, name: &str) -> bool {
        self.nodes.remove(&name.to_lowercase()).is_some()
    }
}

// =============================================================================
// Main Entry Point
// =============================================================================

/// Parse an aegis:// URL or shorthand name into (server_url, database_name)
fn parse_database_url(url: &str) -> Result<(String, Option<String>), String> {
    // Load node registry and check for shorthand names
    let registry = NodeRegistry::load();
    let lower = url.to_lowercase();

    if let Some(entry) = registry.get(&lower) {
        return Ok((entry.url.clone(), Some(lower)));
    }

    if let Some(rest) = url.strip_prefix("aegis://") {
        // Remove "aegis://"
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
        // Try to parse as host:port
        if url.contains(':') {
            Ok((format!("http://{}", url), None))
        } else {
            // List available shorthands in error message
            let available: Vec<_> = registry.nodes.keys().collect();
            Err(format!("Unknown shorthand '{}'. Available: {:?}. Or use aegis://host:port/db, http(s)://..., host:port", url, available))
        }
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
        Commands::Nodes { action } => handle_nodes(&client, action).await,
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
                    let values: Vec<String> = row.iter().map(format_value).collect();
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
                    table.set_header(data.columns.iter().map(Cell::new));

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

// =============================================================================
// Node Management
// =============================================================================

/// Node from /api/v1/admin/nodes endpoint
#[derive(Debug, Deserialize)]
struct ClusterNode {
    id: String,
    address: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    status: String,
}

async fn handle_nodes(client: &Client, action: NodesCommands) -> Result<(), String> {
    match action {
        NodesCommands::List => {
            let registry = NodeRegistry::load();

            if registry.nodes.is_empty() {
                println!("No nodes registered. Use 'aegis-client nodes add <name> <url>' or 'aegis-client nodes sync' to add nodes.");
                return Ok(());
            }

            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["Name", "URL", "Role", "Status", "Node ID"]);

            let mut entries: Vec<_> = registry.nodes.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));

            for (name, entry) in entries {
                table.add_row(vec![
                    name.as_str(),
                    &entry.url,
                    entry.role.as_deref().unwrap_or("-"),
                    entry.status.as_deref().unwrap_or("-"),
                    entry.node_id.as_deref().unwrap_or("-"),
                ]);
            }

            println!("{}", table);
            println!("\nConfig file: {}", NodeRegistry::config_path().display());
        }

        NodesCommands::Add { name, url } => {
            let mut registry = NodeRegistry::load();

            // Validate URL format
            let url = if url.starts_with("http://") || url.starts_with("https://") {
                url
            } else if url.contains(':') {
                format!("http://{}", url)
            } else {
                return Err(format!("Invalid URL: {}. Use http://host:port or host:port format", url));
            };

            // Try to get node info from the server
            let node_id = match client.get(format!("{}/health", url)).send().await {
                Ok(resp) if resp.status().is_success() => {
                    // Try to get node ID from cluster info
                    if let Ok(info) = client.get(format!("{}/api/v1/cluster/info", url)).send().await {
                        if let Ok(json) = info.json::<serde_json::Value>().await {
                            json.get("node_id").and_then(|v| v.as_str()).map(|s| s.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => {
                    eprintln!("Warning: Could not connect to {} to verify", url);
                    None
                }
            };

            registry.add(&name, &url, node_id, None, None);
            registry.save()?;
            println!("Added node '{}' -> {}", name, url);
        }

        NodesCommands::Remove { name } => {
            let mut registry = NodeRegistry::load();

            if registry.remove(&name) {
                registry.save()?;
                println!("Removed node '{}'", name);
            } else {
                return Err(format!("Node '{}' not found", name));
            }
        }

        NodesCommands::Sync { from } => {
            println!("Syncing nodes from {}...", from);

            // Query the cluster for nodes
            let url = format!("{}/api/v1/admin/nodes", from);
            let response = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Failed to connect to {}: {}", from, e))?;

            if !response.status().is_success() {
                return Err(format!("Failed to get nodes: HTTP {}", response.status()));
            }

            // Response is a direct array of nodes
            let nodes: Vec<ClusterNode> = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            let mut registry = NodeRegistry::load();
            let mut added = 0;

            for node in nodes {
                // Extract name from ID if present (format: "node-xxx (NodeName)")
                let shorthand = if let Some(start) = node.id.find('(') {
                    if let Some(end) = node.id.find(')') {
                        node.id[start + 1..end].to_lowercase().replace([' ', '_'], "-")
                    } else {
                        node.id.split('-').next_back().unwrap_or(&node.id).to_string()
                    }
                } else {
                    // Use last part of node ID as shorthand
                    node.id.split('-').next_back().unwrap_or(&node.id).to_string()
                };

                // Convert address to URL
                let url = if node.address.starts_with("http") {
                    node.address.clone()
                } else {
                    format!("http://{}", node.address)
                };

                // Extract clean node ID (without name)
                let clean_id = if let Some(paren_pos) = node.id.find(" (") {
                    node.id[..paren_pos].to_string()
                } else {
                    node.id.clone()
                };

                // Check if already exists with same URL
                let exists = registry.nodes.values().any(|e| e.url == url);
                if !exists {
                    let role = if node.role.is_empty() { None } else { Some(node.role.clone()) };
                    let status = if node.status.is_empty() { None } else { Some(node.status.clone()) };
                    registry.add(&shorthand, &url, Some(clean_id.clone()), role.clone(), status.clone());
                    println!("  Added: {} -> {} [{}] ({})", shorthand, url,
                        role.as_deref().unwrap_or("unknown"), clean_id);
                    added += 1;
                }
            }

            registry.save()?;

            if added > 0 {
                println!("\nSynced {} new node(s)", added);
            } else {
                println!("No new nodes to add (all already registered)");
            }

            // Show current nodes
            println!("\nCurrent nodes:");
            let mut entries: Vec<_> = registry.nodes.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            for (name, entry) in entries {
                println!("  {} -> {}", name, entry.url);
            }
        }
    }

    Ok(())
}
