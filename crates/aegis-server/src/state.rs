//! Aegis Server State
//!
//! Application state shared across request handlers. Provides access to
//! database engines, query engine, and configuration.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityLogger;
use crate::admin::AdminService;
use crate::auth::{AuthService, RbacManager};
use crate::breach::{BreachDetector, WebhookNotifier};
use crate::compress::{decompress_docs, encode_docs};
use crate::config::ServerConfig;
use crate::consent::ConsentManager;
use crate::gdpr::GdprService;
use crate::handlers::{MetricsDataPoint, ServerSettings};
use crate::middleware::RateLimiter;
use aegis_document::{Document, DocumentEngine};
use aegis_query::executor::{ExecutionContext, ExecutionContextSnapshot};
use aegis_query::planner::{PlanNode, PlannerSchema};
use aegis_query::{Executor, Parser, Planner, Statement};
use aegis_shield::ShieldEngine;
use aegis_streaming::StreamingEngine;
use aegis_timeseries::TimeSeriesEngine;
use aegis_updates::orchestrator::UpdateOrchestrator;
use aegis_vault::AegisVault;
use chrono::Utc;
use parking_lot::RwLock as SyncRwLock;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// Application State
// =============================================================================

/// Shared application state with real engine integrations.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub query_engine: Arc<QueryEngine>,
    pub document_engine: Arc<DocumentEngine>,
    pub timeseries_engine: Arc<TimeSeriesEngine>,
    pub streaming_engine: Arc<StreamingEngine>,
    pub kv_store: Arc<KvStore>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub admin: Arc<AdminService>,
    pub auth: Arc<AuthService>,
    pub activity: Arc<ActivityLogger>,
    pub settings: Arc<RwLock<ServerSettings>>,
    pub metrics_history: Arc<RwLock<VecDeque<MetricsDataPoint>>>,
    pub graph_store: Arc<GraphStore>,
    pub rbac: Arc<RbacManager>,
    pub rate_limiter: Arc<RateLimiter>,
    pub login_rate_limiter: Arc<RateLimiter>,
    pub gdpr: Arc<GdprService>,
    pub consent_manager: Arc<ConsentManager>,
    pub breach_detector: Arc<BreachDetector>,
    pub update_orchestrator: Arc<UpdateOrchestrator>,
    pub vault: Arc<AegisVault>,
    pub shield: Arc<ShieldEngine>,
    data_dir: Option<PathBuf>,
}

impl AppState {
    /// Create new application state with the given configuration.
    /// Admin credentials are resolved from environment variables only.
    /// For vault-backed credentials, use `with_secrets` instead.
    pub fn new(config: ServerConfig) -> Self {
        Self::with_secrets(config, None)
    }

    /// Create new application state with secrets provider for admin credentials.
    pub fn with_secrets(
        config: ServerConfig,
        secrets: Option<&dyn crate::secrets::SecretsProvider>,
    ) -> Self {
        let data_dir = config.data_dir.as_ref().map(PathBuf::from);

        // Create activity logger with persistence if data directory is configured
        let activity = if let Some(ref dir) = data_dir {
            let audit_dir = dir.join("audit_logs");
            match ActivityLogger::with_persistence(audit_dir.clone()) {
                Ok(logger) => {
                    tracing::info!("Audit logging enabled with persistence to {:?}", audit_dir);
                    Arc::new(logger)
                }
                Err(e) => {
                    tracing::error!("Failed to initialize persistent audit logging: {}. Falling back to in-memory only.", e);
                    Arc::new(ActivityLogger::new())
                }
            }
        } else {
            Arc::new(ActivityLogger::new())
        };

        // Create data directory if specified
        if let Some(ref dir) = data_dir {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::error!("Failed to create data directory {:?}: {}", dir, e);
            }
        }

        // Initialize engines
        let document_engine = Arc::new(DocumentEngine::new());
        let kv_store = Arc::new(KvStore::with_data_dir(data_dir.clone()));

        // Load persisted data if data directory is specified
        if let Some(ref dir) = data_dir {
            // Load document collections
            let docs_dir = dir.join("documents");
            if docs_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&docs_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "json") {
                            if let Some(collection_name) = path.file_stem().and_then(|s| s.to_str())
                            {
                                // Files may be a NexusCompress frame or legacy plain
                                // JSON; decompress_docs() transparently reads either.
                                if let Ok(bytes) = std::fs::read(&path) {
                                    if let Some(docs) = decompress_docs(&bytes) {
                                        let _ = document_engine.create_collection(collection_name);
                                        let mut count = 0;
                                        for doc_json in docs {
                                            let doc = json_to_document(doc_json);
                                            if document_engine.insert(collection_name, doc).is_ok()
                                            {
                                                count += 1;
                                            }
                                        }
                                        tracing::info!(
                                            "Loaded {} documents into collection '{}'",
                                            count,
                                            collection_name
                                        );
                                    } else {
                                        tracing::error!(
                                            "Failed to decode collection file {:?} — leaving it untouched",
                                            path
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Log server startup
        let node_name_display = config
            .node_name
            .as_ref()
            .map(|n| format!(" ({})", n))
            .unwrap_or_default();
        activity.log_system(&format!(
            "Aegis DB server started - Node: {}{}",
            config.node_id, node_name_display
        ));

        // Create graph store with persistence
        let graph_store = Arc::new(GraphStore::with_data_dir(data_dir.clone()));

        // Initialize metrics history with some data points
        let metrics_history = Arc::new(RwLock::new(VecDeque::new()));

        // Start metrics collection background task
        let metrics_history_clone = metrics_history.clone();
        tokio::spawn(async move {
            Self::collect_metrics_loop(metrics_history_clone).await;
        });

        // Create admin service with cluster config
        let admin = Arc::new(AdminService::with_config(
            &config.node_id,
            config.node_name.clone(),
            &config.address(),
            &config.cluster_name,
            config.peers.clone(),
        ));

        // Create rate limiters using config values
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit_per_minute));
        let login_rate_limiter = Arc::new(RateLimiter::new(config.login_rate_limit_per_minute));

        // Create query engine with persistence if data_dir is configured
        let query_engine = match &data_dir {
            Some(dir) => Arc::new(QueryEngine::with_persistence(dir)),
            None => Arc::new(QueryEngine::new()),
        };

        // Set peer addresses for mutation replication
        if !config.peers.is_empty() {
            query_engine.set_peers(config.peers.clone());
        }

        // Create breach detector with optional webhook notifier
        let breach_detector = Arc::new(BreachDetector::with_data_dir(data_dir.clone()));
        if let Ok(webhook_url) = std::env::var("AEGIS_BREACH_WEBHOOK_URL") {
            if !webhook_url.is_empty() {
                tracing::info!("Breach webhook notification enabled: {}", webhook_url);
                breach_detector.register_notifier(Box::new(WebhookNotifier::new(&webhook_url)));
            }
        }

        // Initialize update orchestrator
        let update_orchestrator = {
            let binary_path =
                std::env::current_exe().unwrap_or_else(|_| PathBuf::from("aegis-server"));
            let base_dir = data_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("/tmp/aegis-updates"));
            Arc::new(UpdateOrchestrator::new(
                binary_path,
                base_dir.join("staging"),
                base_dir.join("backups"),
            ))
        };

        Self {
            config: Arc::new(config),
            query_engine,
            document_engine,
            timeseries_engine: Arc::new({
                let ts_config = aegis_timeseries::engine::EngineConfig {
                    data_path: data_dir.as_ref().map(|d| d.join("timeseries")),
                    ..Default::default()
                };
                TimeSeriesEngine::with_config(ts_config)
            }),
            streaming_engine: Arc::new(StreamingEngine::new()),
            kv_store,
            metrics: Arc::new(RwLock::new(Metrics::default())),
            admin,
            auth: Arc::new(AuthService::with_data_dir_and_secrets(
                data_dir.clone(),
                secrets,
            )),
            activity,
            settings: Arc::new(RwLock::new({
                let mut loaded_settings = ServerSettings::default();
                if let Some(ref dir) = data_dir {
                    let settings_path = dir.join("settings.json");
                    if settings_path.exists() {
                        match std::fs::read_to_string(&settings_path) {
                            Ok(contents) => {
                                match serde_json::from_str::<ServerSettings>(&contents) {
                                    Ok(s) => {
                                        tracing::info!("Loaded server settings from disk");
                                        loaded_settings = s;
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to parse settings from {}: {}",
                                            settings_path.display(),
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to read settings from {}: {}",
                                    settings_path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
                loaded_settings
            })),
            metrics_history,
            graph_store,
            rbac: Arc::new(RbacManager::with_data_dir(data_dir.clone())),
            rate_limiter,
            login_rate_limiter,
            gdpr: Arc::new(GdprService::new()),
            consent_manager: Arc::new(ConsentManager::with_data_dir(data_dir.clone())),
            breach_detector,
            update_orchestrator,
            vault: {
                let vault = AegisVault::new_auto(data_dir.as_ref().map(|d| d.join("vault")));
                // In deny-by-default mode (AEGIS_VAULT_DEFAULT_DENY) the server's
                // own components must be granted explicitly or every internal
                // secret operation fails. External/self-reported components
                // remain denied unless an operator adds policies for them.
                if vault.config().access_default_deny {
                    vault.add_access_policy(aegis_vault::AccessPolicy {
                        name: "server-internal".into(),
                        allowed_components: ["api", "secrets_manager"]
                            .into_iter()
                            .map(String::from)
                            .collect(),
                        allowed_prefixes: Vec::new(),
                        read: true,
                        write: true,
                        delete: true,
                    });
                    tracing::info!(
                        "Vault deny-by-default active: seeded 'server-internal' policy \
                         for components [api, secrets_manager]"
                    );
                }
                Arc::new(vault)
            },
            shield: Arc::new(ShieldEngine::new(aegis_shield::ShieldConfig::default())),
            data_dir,
        }
    }

    /// Save server settings to disk (if data_dir is configured).
    pub async fn save_settings(&self) {
        let Some(ref dir) = self.data_dir else {
            return;
        };
        let path = dir.join("settings.json");
        let settings = self.settings.read().await;
        match serde_json::to_string_pretty(&*settings) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::error!("Failed to write settings to {}: {}", path.display(), e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize settings: {}", e);
            }
        }
    }

    /// Flush a single document collection to disk (if data_dir is configured).
    pub fn flush_collection(&self, collection_name: &str) {
        let Some(ref dir) = self.data_dir else { return };
        let docs_dir = dir.join("documents");
        if let Err(e) = std::fs::create_dir_all(&docs_dir) {
            tracing::error!("Failed to create documents dir: {}", e);
            return;
        }
        let query = aegis_document::Query::new();
        if let Ok(result) = self.document_engine.find(collection_name, &query) {
            let docs: Vec<serde_json::Value> =
                result.documents.iter().map(document_to_json).collect();
            let path = docs_dir.join(format!("{}.json", collection_name));
            // Stored compressed-at-rest as a NexusCompress frame when that beats
            // raw JSON; the loader reads either form transparently.
            let (bytes, _) = encode_docs(&docs);
            if let Err(e) = std::fs::write(&path, bytes) {
                tracing::error!("Failed to write collection '{}': {}", collection_name, e);
            }
        }
    }

    /// Compress one document collection's at-rest file as a NexusCompress frame
    /// now, returning `(doc_count, uncompressed_bytes, compressed_bytes)`. Returns
    /// `None` if there's no data dir or the collection doesn't exist.
    pub fn compress_collection(&self, collection_name: &str) -> Option<(usize, u64, u64)> {
        let dir = self.data_dir.as_ref()?;
        if !self
            .document_engine
            .list_collections()
            .iter()
            .any(|c| c == collection_name)
        {
            return None;
        }
        let docs_dir = dir.join("documents");
        if let Err(e) = std::fs::create_dir_all(&docs_dir) {
            tracing::error!("Failed to create documents dir: {}", e);
            return None;
        }
        let query = aegis_document::Query::new();
        let result = self.document_engine.find(collection_name, &query).ok()?;
        let docs: Vec<serde_json::Value> = result.documents.iter().map(document_to_json).collect();
        let (bytes, uncompressed) = encode_docs(&docs);
        let stored = bytes.len() as u64;
        let path = docs_dir.join(format!("{}.json", collection_name));
        if let Err(e) = std::fs::write(&path, bytes) {
            tracing::error!("Failed to write collection '{}': {}", collection_name, e);
            return None;
        }
        Some((docs.len(), uncompressed as u64, stored))
    }

    /// Save all data to disk (if data_dir is configured).
    pub fn save_to_disk(&self) -> std::io::Result<()> {
        let Some(ref dir) = self.data_dir else {
            return Ok(());
        };

        // KV and documents already flush per-mutation, so periodic save
        // uses compact JSON as a consistency checkpoint
        let kv_path = dir.join("kv_store.json");
        let entries = self.kv_store.list(None, usize::MAX);
        let json = serde_json::to_vec(&entries)?;
        std::fs::write(&kv_path, crate::compress::encode_blob(&json))?;

        let docs_dir = dir.join("documents");
        std::fs::create_dir_all(&docs_dir)?;
        for collection_name in self.document_engine.list_collections() {
            let query = aegis_document::Query::new();
            if let Ok(result) = self.document_engine.find(&collection_name, &query) {
                let docs: Vec<serde_json::Value> =
                    result.documents.iter().map(document_to_json).collect();
                let path = docs_dir.join(format!("{}.json", collection_name));
                // Compressed-at-rest as a NexusCompress frame when smaller than
                // raw JSON; loader reads either form transparently.
                let (bytes, _) = encode_docs(&docs);
                std::fs::write(&path, bytes)?;
            }
        }

        // SQL tables — flush all contexts
        self.query_engine.flush();

        // Flush timeseries data
        self.timeseries_engine.flush();
        tracing::debug!("Flushed timeseries data to disk");

        // Flush audit logs
        if let Err(e) = self.activity.flush() {
            tracing::error!("Failed to flush audit logs: {}", e);
        }
        tracing::debug!("Flushed audit logs to disk");

        Ok(())
    }

    /// Execute a SQL query against the specified database.
    pub async fn execute_query(
        &self,
        sql: &str,
        database: Option<&str>,
    ) -> Result<QueryResult, QueryError> {
        let result = self.query_engine.execute(sql, database)?;
        let db_name = database.unwrap_or("default");
        // Replicate mutations to peers and emit CDC events
        if QueryEngine::is_mutation(sql) {
            self.query_engine.replicate_to_peers(sql, db_name);
            self.emit_cdc_event(sql, db_name, result.rows_affected);
        }
        Ok(result)
    }

    /// Emit a CDC event to the streaming engine for a SQL mutation.
    fn emit_cdc_event(&self, sql: &str, database: &str, rows_affected: u64) {
        use aegis_streaming::cdc::{ChangeEvent, ChangeSource, ChangeType};

        // Extract table name and change type from SQL
        let sql_trimmed = sql.trim();
        let (change_type, table_name) = if sql_trimmed.len() > 6 {
            let upper = sql_trimmed[..12.min(sql_trimmed.len())].to_uppercase();
            if upper.starts_with("INSERT") {
                (ChangeType::Insert, extract_table_after(sql_trimmed, "INTO"))
            } else if upper.starts_with("UPDATE") {
                (
                    ChangeType::Update,
                    extract_table_after(sql_trimmed, "UPDATE"),
                )
            } else if upper.starts_with("DELETE") {
                (ChangeType::Delete, extract_table_after(sql_trimmed, "FROM"))
            } else if upper.starts_with("TRUNCATE") {
                (
                    ChangeType::Truncate,
                    extract_table_after(sql_trimmed, "TRUNCATE"),
                )
            } else {
                return; // DDL — skip CDC
            }
        } else {
            return;
        };

        let source = ChangeSource::new(database, &table_name);
        let change = ChangeEvent {
            change_type,
            source,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            key: None,
            before: None,
            after: Some(serde_json::json!({
                "sql": sql,
                "rows_affected": rows_affected,
            })),
            metadata: std::collections::HashMap::new(),
        };

        // Publish to a CDC channel named "cdc.{database}.{table}"
        let channel_name = format!("cdc.{}.{}", database, table_name);
        let channel_id = aegis_streaming::channel::ChannelId::new(&channel_name);

        // Auto-create channel if it doesn't exist
        let _ = self.streaming_engine.create_channel(channel_name.clone());

        if let Err(e) = self.streaming_engine.publish_change(&channel_id, change) {
            tracing::debug!("CDC publish to {}: {}", channel_name, e);
        }
    }

    /// Execute a query that was received via replication (skip re-replication).
    pub async fn execute_query_replicated(
        &self,
        sql: &str,
        database: Option<&str>,
    ) -> Result<QueryResult, QueryError> {
        self.query_engine.execute(sql, database)
    }

    /// Execute a SQL query with bound parameters.
    pub async fn execute_query_with_params(
        &self,
        sql: &str,
        database: Option<&str>,
        params: &[serde_json::Value],
    ) -> Result<QueryResult, QueryError> {
        self.query_engine.execute_with_params(sql, database, params)
    }

    /// Record a request metric.
    pub async fn record_request(&self, duration_ms: u64, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.total_duration_ms += duration_ms;
        if !success {
            metrics.failed_requests += 1;
        }
    }

    /// Background task to collect real system metrics periodically.
    async fn collect_metrics_loop(metrics_history: Arc<RwLock<VecDeque<MetricsDataPoint>>>) {
        use sysinfo::{Networks, System};

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut last_bytes_in: u64 = 0;
        let mut last_bytes_out: u64 = 0;

        // Initialize network bytes
        for data in networks.list().values() {
            last_bytes_in += data.total_received();
            last_bytes_out += data.total_transmitted();
        }

        loop {
            interval.tick().await;

            // Refresh system info
            sys.refresh_all();
            networks.refresh();

            let now = Utc::now().timestamp();

            // Calculate CPU usage (average across all CPUs)
            let cpu_percent = sys
                .cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage() as f64)
                .sum::<f64>()
                / sys.cpus().len().max(1) as f64;

            // Calculate memory usage
            let memory_total = sys.total_memory();
            let memory_used = sys.used_memory();
            let memory_percent = if memory_total > 0 {
                (memory_used as f64 / memory_total as f64) * 100.0
            } else {
                0.0
            };

            // Calculate network throughput
            let mut current_bytes_in: u64 = 0;
            let mut current_bytes_out: u64 = 0;
            for data in networks.list().values() {
                current_bytes_in += data.total_received();
                current_bytes_out += data.total_transmitted();
            }

            let bytes_in = current_bytes_in.saturating_sub(last_bytes_in);
            let bytes_out = current_bytes_out.saturating_sub(last_bytes_out);
            last_bytes_in = current_bytes_in;
            last_bytes_out = current_bytes_out;

            // Get process count as proxy for connections
            let connections = sys.processes().len() as u64;

            let point = MetricsDataPoint {
                timestamp: now,
                cpu_percent,
                memory_percent,
                queries_per_second: 0.0, // Will be updated by actual query tracking
                latency_ms: 0.0,         // Will be updated by actual query tracking
                connections,
                bytes_in,
                bytes_out,
            };

            let mut history = metrics_history.write().await;

            // Keep last 30 days of minute-resolution data (43200 points)
            if history.len() >= 43200 {
                history.pop_front();
            }
            history.push_back(point);
        }
    }

    /// Initialize metrics history - starts empty, will be populated by real metrics collection.
    pub async fn init_metrics_history(&self) {
        // Metrics history starts empty and is populated by the background collection task
        // with real system metrics. No fake historical data is generated.
        let history = self.metrics_history.write().await;
        tracing::info!(
            "Metrics history initialized (currently {} data points)",
            history.len()
        );
    }

    /// Get comprehensive database statistics.
    pub fn get_database_stats(&self) -> DatabaseStats {
        // Count KV entries
        let total_keys = self.kv_store.count();

        // Count documents across all collections
        let collections = self.document_engine.list_collections();
        let total_documents: usize = collections
            .iter()
            .filter_map(|name| self.document_engine.collection_stats(name))
            .map(|stats| stats.document_count)
            .sum();

        // Get engine stats
        let engine_stats = self.document_engine.stats();

        DatabaseStats {
            total_keys,
            total_documents,
            collection_count: collections.len(),
            documents_inserted: engine_stats.documents_inserted,
            documents_updated: engine_stats.documents_updated,
            documents_deleted: engine_stats.documents_deleted,
            queries_executed: engine_stats.queries_executed,
        }
    }
}

/// Comprehensive database statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseStats {
    pub total_keys: usize,
    pub total_documents: usize,
    pub collection_count: usize,
    pub documents_inserted: u64,
    pub documents_updated: u64,
    pub documents_deleted: u64,
    pub queries_executed: u64,
}

// =============================================================================
// Key-Value Store
// =============================================================================

/// In-memory key-value store with real persistence.
pub struct KvStore {
    data: SyncRwLock<HashMap<String, KvEntry>>,
    data_dir: Option<PathBuf>,
}

/// Key-value entry with metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl KvEntry {
    /// Whether this entry's TTL (seconds since last write) has elapsed by `now`.
    /// Entries without a TTL never expire.
    fn is_expired_at(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        match self.ttl {
            Some(secs) => now > self.updated_at + chrono::Duration::seconds(secs as i64),
            None => false,
        }
    }
}

impl KvStore {
    pub fn new() -> Self {
        Self::with_data_dir(None)
    }

    pub fn with_data_dir(data_dir: Option<PathBuf>) -> Self {
        let mut entries = HashMap::new();

        if let Some(ref dir) = data_dir {
            let kv_path = dir.join("kv_store.json");
            if kv_path.exists() {
                if let Ok(data) = crate::compress::read_blob_file(&kv_path) {
                    if let Ok(loaded) = serde_json::from_slice::<Vec<KvEntry>>(&data) {
                        for entry in loaded {
                            entries.insert(entry.key.clone(), entry);
                        }
                        tracing::info!("Loaded {} KV entries from disk", entries.len());
                    }
                }
            }
        }

        Self {
            data: SyncRwLock::new(entries),
            data_dir,
        }
    }

    /// Flush all KV entries to disk as JSON.
    fn flush_to_disk(&self) {
        if let Some(ref dir) = self.data_dir {
            let kv_path = dir.join("kv_store.json");
            let data = self.data.read();
            let entries: Vec<&KvEntry> = data.values().collect();
            match serde_json::to_vec(&entries) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&kv_path, crate::compress::encode_blob(&json)) {
                        tracing::error!("Failed to flush KV store to {:?}: {}", kv_path, e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to serialize KV store: {}", e);
                }
            }
        }
    }

    /// Set a key-value pair.
    pub fn set(&self, key: String, value: serde_json::Value, ttl: Option<u64>) -> KvEntry {
        let now = chrono::Utc::now();
        let mut data = self.data.write();

        let entry = if let Some(existing) = data.get(&key) {
            KvEntry {
                key: key.clone(),
                value,
                ttl,
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            KvEntry {
                key: key.clone(),
                value,
                ttl,
                created_at: now,
                updated_at: now,
            }
        };

        data.insert(key, entry.clone());
        drop(data);
        self.flush_to_disk();
        entry
    }

    /// Get a value by key. Expired (TTL-elapsed) keys are treated as absent and
    /// lazily evicted on access.
    pub fn get(&self, key: &str) -> Option<KvEntry> {
        let now = chrono::Utc::now();
        {
            let data = self.data.read();
            match data.get(key) {
                Some(entry) if !entry.is_expired_at(now) => return Some(entry.clone()),
                Some(_) => {} // expired — fall through to evict
                None => return None,
            }
        }
        // Evict the expired entry under a write lock.
        let mut data = self.data.write();
        if data.get(key).is_some_and(|e| e.is_expired_at(now)) {
            data.remove(key);
            drop(data);
            self.flush_to_disk();
        }
        None
    }

    /// Delete a key.
    pub fn delete(&self, key: &str) -> Option<KvEntry> {
        let mut data = self.data.write();
        let removed = data.remove(key);
        drop(data);
        if removed.is_some() {
            self.flush_to_disk();
        }
        removed
    }

    /// List all keys with optional prefix filter. Expired entries are omitted.
    pub fn list(&self, prefix: Option<&str>, limit: usize) -> Vec<KvEntry> {
        let now = chrono::Utc::now();
        let data = self.data.read();
        data.values()
            .filter(|e| !e.is_expired_at(now))
            .filter(|e| prefix.map(|p| e.key.starts_with(p)).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get total count of non-expired keys.
    pub fn count(&self) -> usize {
        let now = chrono::Utc::now();
        self.data
            .read()
            .values()
            .filter(|e| !e.is_expired_at(now))
            .count()
    }
}

impl Default for KvStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Graph Store
// =============================================================================

/// Graph node for visualization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Graph edge for visualization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship: String,
}

/// In-memory graph store.
pub struct GraphStore {
    nodes: SyncRwLock<HashMap<String, GraphNode>>,
    edges: SyncRwLock<HashMap<String, GraphEdge>>,
    node_counter: AtomicU64,
    edge_counter: AtomicU64,
    data_dir: Option<PathBuf>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GraphSnapshot {
    nodes: HashMap<String, GraphNode>,
    edges: HashMap<String, GraphEdge>,
    node_counter: u64,
    edge_counter: u64,
}

impl GraphStore {
    pub fn new() -> Self {
        Self::with_data_dir(None)
    }

    pub fn with_data_dir(data_dir: Option<PathBuf>) -> Self {
        if let Some(ref dir) = data_dir {
            let graph_path = dir.join("graph_store.json");
            if graph_path.exists() {
                if let Ok(data) = crate::compress::read_blob_file(&graph_path) {
                    if let Ok(snapshot) = serde_json::from_slice::<GraphSnapshot>(&data) {
                        tracing::info!(
                            "Loaded {} graph nodes and {} edges from disk",
                            snapshot.nodes.len(),
                            snapshot.edges.len()
                        );
                        return Self {
                            nodes: SyncRwLock::new(snapshot.nodes),
                            edges: SyncRwLock::new(snapshot.edges),
                            node_counter: AtomicU64::new(snapshot.node_counter),
                            edge_counter: AtomicU64::new(snapshot.edge_counter),
                            data_dir,
                        };
                    }
                }
            }
        }

        Self {
            nodes: SyncRwLock::new(HashMap::new()),
            edges: SyncRwLock::new(HashMap::new()),
            node_counter: AtomicU64::new(1),
            edge_counter: AtomicU64::new(1),
            data_dir,
        }
    }

    /// Flush graph data to disk as JSON.
    fn flush_to_disk(&self) {
        if let Some(ref dir) = self.data_dir {
            let graph_path = dir.join("graph_store.json");
            let snapshot = GraphSnapshot {
                nodes: self.nodes.read().clone(),
                edges: self.edges.read().clone(),
                node_counter: self.node_counter.load(Ordering::SeqCst),
                edge_counter: self.edge_counter.load(Ordering::SeqCst),
            };
            match serde_json::to_vec(&snapshot) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&graph_path, crate::compress::encode_blob(&json))
                    {
                        tracing::error!("Failed to flush graph store to {:?}: {}", graph_path, e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to serialize graph store: {}", e);
                }
            }
        }
    }

    /// Create a new node.
    pub fn create_node(&self, label: &str, properties: serde_json::Value) -> GraphNode {
        let id = format!(
            "{}:{}",
            label.to_lowercase(),
            self.node_counter.fetch_add(1, Ordering::SeqCst)
        );
        let node = GraphNode {
            id: id.clone(),
            label: label.to_string(),
            properties,
        };
        self.nodes.write().insert(id, node.clone());
        self.flush_to_disk();
        node
    }

    /// Create a new edge between nodes.
    pub fn create_edge(
        &self,
        source: &str,
        target: &str,
        relationship: &str,
    ) -> Result<GraphEdge, String> {
        let nodes = self.nodes.read();
        if !nodes.contains_key(source) {
            return Err(format!("Source node '{}' not found", source));
        }
        if !nodes.contains_key(target) {
            return Err(format!("Target node '{}' not found", target));
        }
        drop(nodes);

        let id = format!("e{}", self.edge_counter.fetch_add(1, Ordering::SeqCst));
        let edge = GraphEdge {
            id: id.clone(),
            source: source.to_string(),
            target: target.to_string(),
            relationship: relationship.to_string(),
        };
        self.edges.write().insert(id, edge.clone());
        self.flush_to_disk();
        Ok(edge)
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: &str) -> Option<GraphNode> {
        self.nodes.read().get(id).cloned()
    }

    /// Delete a node and its edges.
    /// Lock order: nodes first, then edges (consistent with create_edge).
    pub fn delete_node(&self, id: &str) -> Result<(), String> {
        // Acquire both locks in consistent order: nodes → edges
        let mut nodes = self.nodes.write();
        let mut edges = self.edges.write();

        if nodes.remove(id).is_none() {
            return Err(format!("Node '{}' not found", id));
        }
        edges.retain(|_, e| e.source != id && e.target != id);

        drop(edges);
        drop(nodes);
        self.flush_to_disk();
        Ok(())
    }

    /// Delete an edge.
    pub fn delete_edge(&self, id: &str) -> Result<(), String> {
        if self.edges.write().remove(id).is_none() {
            return Err(format!("Edge '{}' not found", id));
        }
        self.flush_to_disk();
        Ok(())
    }

    /// Update a node's label and/or properties. Fields left as `None` are kept.
    pub fn update_node(
        &self,
        id: &str,
        label: Option<String>,
        properties: Option<serde_json::Value>,
    ) -> Result<GraphNode, String> {
        let updated = {
            let mut nodes = self.nodes.write();
            let node = nodes
                .get_mut(id)
                .ok_or_else(|| format!("Node '{}' not found", id))?;
            if let Some(l) = label {
                node.label = l;
            }
            if let Some(p) = properties {
                node.properties = p;
            }
            node.clone()
        };
        self.flush_to_disk();
        Ok(updated)
    }

    /// Update an edge's relationship type.
    pub fn update_edge(&self, id: &str, relationship: String) -> Result<GraphEdge, String> {
        let updated = {
            let mut edges = self.edges.write();
            let edge = edges
                .get_mut(id)
                .ok_or_else(|| format!("Edge '{}' not found", id))?;
            edge.relationship = relationship;
            edge.clone()
        };
        self.flush_to_disk();
        Ok(updated)
    }

    /// List all nodes.
    pub fn list_nodes(&self) -> Vec<GraphNode> {
        self.nodes.read().values().cloned().collect()
    }

    /// List all edges.
    pub fn list_edges(&self) -> Vec<GraphEdge> {
        self.edges.read().values().cloned().collect()
    }

    /// Get all nodes and edges.
    pub fn get_all(&self) -> (Vec<GraphNode>, Vec<GraphEdge>) {
        (self.list_nodes(), self.list_edges())
    }

    /// Search nodes by label.
    pub fn find_by_label(&self, label: &str) -> Vec<GraphNode> {
        self.nodes
            .read()
            .values()
            .filter(|n| n.label.to_lowercase() == label.to_lowercase())
            .cloned()
            .collect()
    }

    /// Get edges for a node.
    pub fn get_edges_for_node(&self, node_id: &str) -> Vec<GraphEdge> {
        self.edges
            .read()
            .values()
            .filter(|e| e.source == node_id || e.target == node_id)
            .cloned()
            .collect()
    }
}

impl Default for GraphStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Query Engine Wrapper
// =============================================================================

/// A server-side prepared statement: SQL parsed and planned once, then executed
/// many times with different bound parameters (skips re-parse and re-plan).
struct PreparedStatement {
    plan: aegis_query::QueryPlan,
    database: String,
    is_mutation: bool,
}

/// Query engine for executing SQL statements.
/// Maintains separate ExecutionContexts per database for multi-tenancy.
/// Now with disk persistence support for crash recovery.
pub struct QueryEngine {
    parser: Parser,
    planner: Planner,
    /// Map of database name -> ExecutionContext
    contexts: Arc<std::sync::RwLock<HashMap<String, Arc<std::sync::RwLock<ExecutionContext>>>>>,
    /// Path to persist SQL tables (if set, enables persistence)
    data_path: Option<PathBuf>,
    /// Peer addresses for mutation replication (e.g. ["127.0.0.1:9091", "127.0.0.1:7001"])
    peers: Arc<std::sync::RwLock<Vec<String>>>,
    /// Write-ahead log for crash recovery
    wal: Option<Arc<aegis_storage::wal::WriteAheadLog>>,
    /// Query plan cache: SQL string -> planned QueryPlan (LRU-style, max 1024 entries)
    plan_cache: std::sync::RwLock<HashMap<String, aegis_query::QueryPlan>>,
    /// Server-side prepared statements: id -> pre-planned statement.
    prepared: std::sync::RwLock<HashMap<String, PreparedStatement>>,
    /// Monotonic counter for prepared-statement ids.
    prepared_counter: AtomicU64,
}

impl QueryEngine {
    pub fn new() -> Self {
        let schema = Arc::new(PlannerSchema::new());
        let mut contexts = HashMap::new();
        // Create default database
        contexts.insert(
            "default".to_string(),
            Arc::new(std::sync::RwLock::new(ExecutionContext::new())),
        );
        Self {
            parser: Parser::new(),
            planner: Planner::new(schema),
            contexts: Arc::new(std::sync::RwLock::new(contexts)),
            data_path: None,
            peers: Arc::new(std::sync::RwLock::new(Vec::new())),
            wal: None,
            plan_cache: std::sync::RwLock::new(HashMap::new()),
            prepared: std::sync::RwLock::new(HashMap::new()),
            prepared_counter: AtomicU64::new(1),
        }
    }

    /// Create a QueryEngine with persistence to the specified directory.
    pub fn with_persistence(data_dir: &std::path::Path) -> Self {
        let schema = Arc::new(PlannerSchema::new());
        let db_dir = data_dir.join("databases");

        // Create databases directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&db_dir) {
            tracing::warn!("Failed to create databases directory: {}", e);
        }

        let mut contexts = HashMap::new();

        // Load all existing databases from the directory
        if let Ok(entries) = std::fs::read_dir(&db_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Some(db_name) = path.file_stem().and_then(|s| s.to_str()) {
                        match crate::compress::read_blob_file(&path).and_then(|bytes| {
                            serde_json::from_slice::<
                                aegis_query::executor::ExecutionContextSnapshot,
                            >(&bytes)
                            .map(ExecutionContext::from_snapshot)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                        }) {
                            Ok(ctx) => {
                                tracing::info!("Loaded database '{}' from {:?}", db_name, path);
                                contexts.insert(
                                    db_name.to_string(),
                                    Arc::new(std::sync::RwLock::new(ctx)),
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to load database '{}' from {:?}: {}",
                                    db_name,
                                    path,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        // Ensure default database exists
        if !contexts.contains_key("default") {
            contexts.insert(
                "default".to_string(),
                Arc::new(std::sync::RwLock::new(ExecutionContext::new())),
            );
        }

        // Initialize WAL with crash recovery — replay any committed records
        let wal_dir = data_dir.join("wal");
        let wal = match aegis_storage::wal::WriteAheadLog::open_and_recover(wal_dir, true) {
            Ok((w, recovery)) => {
                if recovery.records_processed > 0 {
                    tracing::info!(
                        "WAL recovery: {} records processed, {} segments scanned, {} incomplete transactions",
                        recovery.records_processed,
                        recovery.segments_scanned,
                        recovery.incomplete_transactions.len(),
                    );
                } else {
                    tracing::info!("WAL initialized (clean startup, no recovery needed)");
                }
                Some(Arc::new(w))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize WAL: {}. Continuing without WAL.", e);
                None
            }
        };

        Self {
            parser: Parser::new(),
            planner: Planner::new(schema),
            contexts: Arc::new(std::sync::RwLock::new(contexts)),
            data_path: Some(db_dir),
            peers: Arc::new(std::sync::RwLock::new(Vec::new())),
            wal,
            plan_cache: std::sync::RwLock::new(HashMap::new()),
            prepared: std::sync::RwLock::new(HashMap::new()),
            prepared_counter: AtomicU64::new(1),
        }
    }

    /// Set peer addresses for mutation replication.
    pub fn set_peers(&self, peers: Vec<String>) {
        *self.peers.write().unwrap() = peers;
    }

    /// Asynchronously replicate a SQL mutation to all peer nodes.
    fn replicate_to_peers(&self, sql: &str, database: &str) {
        let peers = self.peers.read().unwrap().clone();
        if peers.is_empty() {
            return;
        }

        let sql = sql.to_string();
        let db = database.to_string();

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            for peer in &peers {
                let url = format!("http://{}/api/v1/query", peer);
                let body = serde_json::json!({
                    "sql": sql,
                    "database": db,
                });

                // Retry up to 3 times with exponential backoff
                let mut success = false;
                for attempt in 0..3u32 {
                    if attempt > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            100 * 2u64.pow(attempt),
                        ))
                        .await;
                    }
                    let resp = client
                        .post(&url)
                        .header("X-Aegis-Replicated", "true")
                        .json(&body)
                        .send()
                        .await;

                    match resp {
                        Ok(r) if r.status().is_success() => {
                            tracing::debug!(
                                "Replicated to {}: {}",
                                peer,
                                &sql[..sql.len().min(80)]
                            );
                            success = true;
                            break;
                        }
                        Ok(r) => {
                            tracing::warn!(
                                "Replication to {} returned {} (attempt {})",
                                peer,
                                r.status(),
                                attempt + 1
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Replication to {} failed: {} (attempt {})",
                                peer,
                                e,
                                attempt + 1
                            );
                        }
                    }
                }
                if !success {
                    tracing::error!("Replication to {} failed after 3 attempts", peer);
                }
            }
        });
    }

    /// Plan a statement, using cache for SELECT queries.
    fn plan_cached(&self, stmt: &Statement) -> Result<aegis_query::QueryPlan, QueryError> {
        // Only cache read-only queries
        let cache_key = if !Self::is_mutation_stmt(stmt) {
            Some(format!("{:?}", stmt))
        } else {
            None
        };

        // Check cache
        if let Some(ref key) = cache_key {
            if let Ok(cache) = self.plan_cache.read() {
                if let Some(plan) = cache.get(key) {
                    return Ok(plan.clone());
                }
            }
        }

        // Plan and cache
        let plan = self
            .planner
            .plan(stmt)
            .map_err(|e| QueryError::Plan(e.to_string()))?;

        if let Some(key) = cache_key {
            if let Ok(mut cache) = self.plan_cache.write() {
                // LRU eviction: if cache is full, clear it
                if cache.len() >= 1024 {
                    cache.clear();
                }
                cache.insert(key, plan.clone());
            }
        }

        Ok(plan)
    }

    /// Get or create an ExecutionContext for the specified database.
    fn get_or_create_context(&self, database: &str) -> Arc<std::sync::RwLock<ExecutionContext>> {
        let db_name = if database.is_empty() {
            "default"
        } else {
            database
        };

        // Try to get existing context
        {
            let contexts = self.contexts.read().unwrap();
            if let Some(ctx) = contexts.get(db_name) {
                return ctx.clone();
            }
        }

        // Create new context for this database
        let mut contexts = self.contexts.write().unwrap();
        // Double-check after acquiring write lock
        if let Some(ctx) = contexts.get(db_name) {
            return ctx.clone();
        }

        tracing::info!("Creating new database: {}", db_name);
        let ctx = Arc::new(std::sync::RwLock::new(ExecutionContext::new()));
        contexts.insert(db_name.to_string(), ctx.clone());
        ctx
    }

    /// Persist a specific database to disk.
    fn persist(&self, database: &str) {
        if let Some(ref base_path) = self.data_path {
            let db_name = if database.is_empty() {
                "default"
            } else {
                database
            };
            let path = base_path.join(format!("{}.json", db_name));

            // Write to WAL before persisting snapshot (write-ahead guarantee)
            if let Some(ref wal) = self.wal {
                use aegis_common::TransactionId;
                use aegis_storage::wal::{LogRecord, LogRecordType};

                let lsn = wal.next_lsn();
                let record = LogRecord {
                    lsn,
                    prev_lsn: None,
                    tx_id: TransactionId(0),
                    record_type: LogRecordType::Checkpoint,
                    page_id: None,
                    data: ::bytes::Bytes::from(format!("persist:{}", db_name)),
                };
                if let Err(e) = wal.append(record) {
                    tracing::warn!("WAL append failed: {}", e);
                }
            }

            let contexts = self.contexts.read().unwrap();
            if let Some(ctx) = contexts.get(db_name) {
                if let Ok(ctx_guard) = ctx.read() {
                    if let Err(e) =
                        crate::compress::write_blob_json(&path, &ctx_guard.to_snapshot())
                    {
                        tracing::error!(
                            "Failed to persist database '{}' to {:?}: {}",
                            db_name,
                            path,
                            e
                        );
                    }
                }
            }
        }
    }

    /// Check if a SQL statement is a mutation (DDL/DML that modifies data).
    pub fn is_mutation(sql: &str) -> bool {
        let sql_upper = sql.trim().to_uppercase();
        sql_upper.starts_with("CREATE")
            || sql_upper.starts_with("DROP")
            || sql_upper.starts_with("ALTER")
            || sql_upper.starts_with("INSERT")
            || sql_upper.starts_with("UPDATE")
            || sql_upper.starts_with("DELETE")
            || sql_upper.starts_with("TRUNCATE")
    }

    /// Check if a parsed statement is a mutation (DDL/DML).
    fn is_mutation_stmt(stmt: &Statement) -> bool {
        matches!(
            stmt,
            Statement::Insert(_)
                | Statement::Update(_)
                | Statement::Delete(_)
                | Statement::CreateTable(_)
                | Statement::DropTable(_)
                | Statement::AlterTable(_)
                | Statement::CreateIndex(_)
                | Statement::DropIndex(_)
        )
    }

    /// Check if a statement is DDL (schema-changing) — invalidates plan cache.
    fn is_ddl(stmt: &Statement) -> bool {
        matches!(
            stmt,
            Statement::CreateTable(_)
                | Statement::DropTable(_)
                | Statement::AlterTable(_)
                | Statement::CreateIndex(_)
                | Statement::DropIndex(_)
        )
    }

    /// Invalidate the plan cache (called after DDL changes).
    fn invalidate_plan_cache(&self) {
        if let Ok(mut cache) = self.plan_cache.write() {
            cache.clear();
        }
    }

    /// Return an [`Executor`] bound to the shared context for `database`
    /// (or `"default"`). Gives direct, no-HTTP, no-SQL access to the engine —
    /// used by the engine-level benchmark suite and hot closure-based paths
    /// such as `execute_update_indexed_fn`.
    pub fn get_executor(&self, database: Option<&str>) -> Executor {
        let context = self.get_or_create_context(database.unwrap_or("default"));
        Executor::with_shared_context(context)
    }

    /// Execute a SQL query against the specified database.
    /// Supports multi-statement input with transaction control:
    /// `BEGIN; INSERT ...; INSERT ...; COMMIT;` executes atomically.
    /// On ROLLBACK (explicit or on error inside a transaction), all
    /// changes since BEGIN are undone.
    pub fn execute(&self, sql: &str, database: Option<&str>) -> Result<QueryResult, QueryError> {
        let db_name = database.unwrap_or("default");

        let statements = self
            .parser
            .parse(sql)
            .map_err(|e| QueryError::Parse(e.to_string()))?;

        if statements.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: 0,
            });
        }

        let context = self.get_or_create_context(db_name);
        let executor = Executor::with_shared_context(context.clone());

        let mut last_result = QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: 0,
        };
        let mut in_transaction = false;
        let mut txn_snapshot: Option<ExecutionContextSnapshot> = None;
        let mut had_mutation = false;

        for statement in &statements {
            let plan = self.plan_cached(statement)?;

            match &plan.root {
                PlanNode::BeginTransaction => {
                    if in_transaction {
                        return Err(QueryError::Execute("Already in a transaction".to_string()));
                    }
                    // Snapshot current state for rollback + MVCC snapshot isolation
                    let mut ctx = context
                        .write()
                        .map_err(|_| QueryError::Execute("Lock poisoned".to_string()))?;
                    txn_snapshot = Some(ctx.to_snapshot());
                    ctx.begin_snapshot();
                    drop(ctx);
                    in_transaction = true;
                    last_result = QueryResult {
                        columns: vec!["status".to_string()],
                        rows: vec![vec![serde_json::Value::String("BEGIN".to_string())]],
                        rows_affected: 0,
                    };
                }
                PlanNode::CommitTransaction => {
                    if !in_transaction {
                        return Err(QueryError::Execute(
                            "No transaction in progress".to_string(),
                        ));
                    }
                    // Commit: release MVCC snapshot, advance version, discard rollback snapshot, persist
                    {
                        let mut ctx = context
                            .write()
                            .map_err(|_| QueryError::Execute("Lock poisoned".to_string()))?;
                        ctx.commit_snapshot();
                    }
                    txn_snapshot = None;
                    in_transaction = false;
                    if had_mutation {
                        self.persist(db_name);
                        had_mutation = false;
                    }
                    last_result = QueryResult {
                        columns: vec!["status".to_string()],
                        rows: vec![vec![serde_json::Value::String("COMMIT".to_string())]],
                        rows_affected: 0,
                    };
                }
                PlanNode::RollbackTransaction => {
                    if !in_transaction {
                        return Err(QueryError::Execute(
                            "No transaction in progress".to_string(),
                        ));
                    }
                    // Rollback: restore snapshot and release MVCC snapshot
                    if let Some(snapshot) = txn_snapshot.take() {
                        let mut ctx = context
                            .write()
                            .map_err(|_| QueryError::Execute("Lock poisoned".to_string()))?;
                        ctx.restore_from_snapshot(snapshot);
                        ctx.rollback_snapshot();
                    }
                    in_transaction = false;
                    had_mutation = false;
                    last_result = QueryResult {
                        columns: vec!["status".to_string()],
                        rows: vec![vec![serde_json::Value::String("ROLLBACK".to_string())]],
                        rows_affected: 0,
                    };
                }
                _ => {
                    // Regular statement — execute it
                    let result = match executor.execute(&plan) {
                        Ok(r) => r,
                        Err(e) => {
                            // If inside a transaction, auto-rollback on error
                            if let Some(snapshot) = txn_snapshot.take() {
                                if let Ok(mut ctx) = context.write() {
                                    ctx.restore_from_snapshot(snapshot);
                                    ctx.rollback_snapshot();
                                }
                                tracing::warn!("Transaction rolled back due to error: {}", e);
                            }
                            return Err(QueryError::Execute(e.to_string()));
                        }
                    };

                    let is_mut = Self::is_mutation_stmt(statement);
                    if is_mut {
                        had_mutation = true;
                        if Self::is_ddl(statement) {
                            self.invalidate_plan_cache();
                        }
                    }

                    last_result = QueryResult {
                        columns: result.columns,
                        rows: result
                            .rows
                            .into_iter()
                            .map(|r| r.values.into_iter().map(value_to_json).collect())
                            .collect(),
                        rows_affected: result.rows_affected,
                    };

                    // Persist immediately if not inside a transaction
                    if is_mut && !in_transaction {
                        self.persist(db_name);
                    }
                }
            }
        }

        // If transaction was started but never committed, auto-rollback
        if in_transaction {
            if let Some(snapshot) = txn_snapshot.take() {
                if let Ok(mut ctx) = context.write() {
                    ctx.restore_from_snapshot(snapshot);
                    ctx.rollback_snapshot();
                }
            }
            return Err(QueryError::Execute(
                "Transaction was not committed (missing COMMIT). Changes rolled back.".to_string(),
            ));
        }

        Ok(last_result)
    }

    /// List all tables in the specified database.
    /// Execute a parameterized SQL query.
    /// Params are JSON values that replace $1, $2, ... placeholders.
    pub fn execute_with_params(
        &self,
        sql: &str,
        database: Option<&str>,
        params: &[serde_json::Value],
    ) -> Result<QueryResult, QueryError> {
        if params.is_empty() {
            return self.execute(sql, database);
        }

        let db_name = database.unwrap_or("default");

        let statements = self
            .parser
            .parse(sql)
            .map_err(|e| QueryError::Parse(e.to_string()))?;

        if statements.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: 0,
            });
        }

        // Convert JSON params to aegis Values
        let values: Vec<aegis_common::Value> = params.iter().map(json_param_to_value).collect();

        let statement = &statements[0];
        let plan = self
            .planner
            .plan(statement)
            .map_err(|e| QueryError::Plan(e.to_string()))?;

        let context = self.get_or_create_context(db_name);
        let executor = Executor::with_shared_context(context);
        let result = executor
            .execute_with_params(&plan, &values)
            .map_err(|e| QueryError::Execute(e.to_string()))?;

        if Self::is_mutation(sql) {
            self.persist(db_name);
        }

        Ok(QueryResult {
            columns: result.columns,
            rows: result
                .rows
                .into_iter()
                .map(|r| r.values.into_iter().map(value_to_json).collect())
                .collect(),
            rows_affected: result.rows_affected,
        })
    }

    /// Prepare a statement: parse and plan it once, store it, and return an id
    /// the client can execute repeatedly with different bound parameters.
    /// Skips re-parsing and re-planning on every execution.
    ///
    /// Note: prepared-statement mutations are persisted locally but are **not**
    /// peer-replicated — use the regular query path for replicated writes.
    pub fn prepare(&self, sql: &str, database: Option<&str>) -> Result<String, QueryError> {
        let db_name = database.unwrap_or("default").to_string();
        let statements = self
            .parser
            .parse(sql)
            .map_err(|e| QueryError::Parse(e.to_string()))?;
        let statement = statements
            .first()
            .ok_or_else(|| QueryError::Parse("empty statement".to_string()))?;
        let plan = self
            .planner
            .plan(statement)
            .map_err(|e| QueryError::Plan(e.to_string()))?;
        let is_mutation = Self::is_mutation(sql);
        let id = format!(
            "stmt_{}",
            self.prepared_counter.fetch_add(1, Ordering::SeqCst)
        );
        self.prepared.write().unwrap().insert(
            id.clone(),
            PreparedStatement {
                plan,
                database: db_name,
                is_mutation,
            },
        );
        Ok(id)
    }

    /// Execute a previously prepared statement with bound parameters.
    pub fn execute_prepared(
        &self,
        id: &str,
        params: &[serde_json::Value],
    ) -> Result<QueryResult, QueryError> {
        let (plan, db_name, is_mutation) = {
            let map = self.prepared.read().unwrap();
            let prepared = map.get(id).ok_or_else(|| {
                QueryError::Execute(format!("unknown prepared statement '{}'", id))
            })?;
            (
                prepared.plan.clone(),
                prepared.database.clone(),
                prepared.is_mutation,
            )
        };

        let values: Vec<aegis_common::Value> = params.iter().map(json_param_to_value).collect();
        let context = self.get_or_create_context(&db_name);
        let executor = Executor::with_shared_context(context);
        let result = executor
            .execute_with_params(&plan, &values)
            .map_err(|e| QueryError::Execute(e.to_string()))?;

        if is_mutation {
            self.persist(&db_name);
        }

        Ok(QueryResult {
            columns: result.columns,
            rows: result
                .rows
                .into_iter()
                .map(|r| r.values.into_iter().map(value_to_json).collect())
                .collect(),
            rows_affected: result.rows_affected,
        })
    }

    /// Deallocate a prepared statement. Returns true if it existed.
    pub fn deallocate(&self, id: &str) -> bool {
        self.prepared.write().unwrap().remove(id).is_some()
    }

    /// Number of currently prepared statements.
    pub fn prepared_count(&self) -> usize {
        self.prepared.read().map(|m| m.len()).unwrap_or(0)
    }

    pub fn list_tables(&self, database: Option<&str>) -> Vec<String> {
        let db_name = database.unwrap_or("default");
        let contexts = self.contexts.read().unwrap();
        contexts
            .get(db_name)
            .and_then(|ctx| ctx.read().ok())
            .map(|ctx| ctx.list_tables())
            .unwrap_or_default()
    }

    /// List all databases.
    pub fn list_databases(&self) -> Vec<String> {
        self.contexts
            .read()
            .map(|contexts| contexts.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get table schema information from the specified database.
    pub fn get_table_info(&self, name: &str, database: Option<&str>) -> Option<TableInfo> {
        let db_name = database.unwrap_or("default");
        let contexts = self.contexts.read().ok()?;
        let ctx_lock = contexts.get(db_name)?;
        let ctx = ctx_lock.read().ok()?;
        let schema = ctx.get_table_schema(name)?;
        let table_data = ctx.get_table(name)?;
        let row_count = table_data.read().ok().map(|t| t.rows.len() as u64);

        Some(TableInfo {
            name: schema.name.clone(),
            columns: schema
                .columns
                .iter()
                .map(|c| ColumnInfo {
                    name: c.name.clone(),
                    data_type: format!("{:?}", c.data_type),
                    nullable: c.nullable,
                })
                .collect(),
            row_count,
        })
    }

    /// Force persist all databases to disk (for graceful shutdown).
    pub fn flush(&self) {
        if let Some(ref base_path) = self.data_path {
            let contexts = self.contexts.read().unwrap();
            for (db_name, ctx) in contexts.iter() {
                let path = base_path.join(format!("{}.json", db_name));
                if let Ok(ctx_guard) = ctx.read() {
                    if let Err(e) =
                        crate::compress::write_blob_json(&path, &ctx_guard.to_snapshot())
                    {
                        tracing::error!(
                            "Failed to persist database '{}' to {:?}: {}",
                            db_name,
                            path,
                            e
                        );
                    }
                }
            }
        }
    }
}

/// Table information for API response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
}

/// Column information for API response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Query Result
// =============================================================================

/// Result of a query execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: u64,
}

/// Convert aegis Value to JSON.
/// Convert a JSON parameter value to an aegis Value for query binding.
fn json_param_to_value(json: &serde_json::Value) -> aegis_common::Value {
    match json {
        serde_json::Value::Null => aegis_common::Value::Null,
        serde_json::Value::Bool(b) => aegis_common::Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                aegis_common::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                aegis_common::Value::Float(f)
            } else {
                aegis_common::Value::Null
            }
        }
        serde_json::Value::String(s) => aegis_common::Value::String(s.clone()),
        _ => aegis_common::Value::String(json.to_string()),
    }
}

/// Extract the table name following a keyword in SQL (e.g., "INTO" in INSERT INTO).
fn extract_table_after(sql: &str, keyword: &str) -> String {
    let upper = sql.to_uppercase();
    if let Some(pos) = upper.find(keyword) {
        let after = &sql[pos + keyword.len()..].trim_start();
        after
            .split(|c: char| c.is_whitespace() || c == '(')
            .next()
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    }
}

fn value_to_json(value: aegis_common::Value) -> serde_json::Value {
    match value {
        aegis_common::Value::Null => serde_json::Value::Null,
        aegis_common::Value::Boolean(b) => serde_json::Value::Bool(b),
        aegis_common::Value::Integer(i) => serde_json::Value::Number(i.into()),
        aegis_common::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        aegis_common::Value::String(s) => serde_json::Value::String(s),
        aegis_common::Value::Bytes(b) => serde_json::Value::String(base64_encode(&b)),
        aegis_common::Value::Timestamp(t) => serde_json::Value::String(t.to_rfc3339()),
        aegis_common::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(value_to_json).collect())
        }
        aegis_common::Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .into_iter()
                .map(|(k, v)| (k, value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(CHARS[b0 >> 2] as char);
        result.push(CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

// =============================================================================
// Query Error
// =============================================================================

/// Errors during query execution.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Planning error: {0}")]
    Plan(String),

    #[error("Execution error: {0}")]
    Execute(String),
}

// =============================================================================
// Metrics
// =============================================================================

/// Server metrics.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Metrics {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub total_duration_ms: u64,
}

impl Metrics {
    /// Calculate average request duration.
    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_requests as f64
        }
    }

    /// Calculate success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            1.0 - (self.failed_requests as f64 / self.total_requests as f64)
        }
    }
}

// =============================================================================
// Document Persistence Helpers
// =============================================================================

/// Convert JSON to Document for loading from disk.
fn json_to_document(json: serde_json::Value) -> Document {
    let mut doc = if let Some(id) = json.get("_id").and_then(|v| v.as_str()) {
        Document::with_id(id)
    } else {
        Document::new()
    };

    if let serde_json::Value::Object(map) = json {
        for (key, value) in map {
            if key != "_id" {
                doc.set(&key, json_to_doc_value(value));
            }
        }
    }
    doc
}

/// Convert Document to JSON for saving to disk.
fn document_to_json(doc: &Document) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "_id".to_string(),
        serde_json::Value::String(doc.id.to_string()),
    );
    for (key, value) in &doc.data {
        map.insert(key.clone(), doc_value_to_json(value));
    }
    serde_json::Value::Object(map)
}

/// Convert JSON value to aegis_document::Value.
fn json_to_doc_value(json: serde_json::Value) -> aegis_document::Value {
    match json {
        serde_json::Value::Null => aegis_document::Value::Null,
        serde_json::Value::Bool(b) => aegis_document::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                aegis_document::Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                aegis_document::Value::Float(f)
            } else {
                aegis_document::Value::Null
            }
        }
        serde_json::Value::String(s) => aegis_document::Value::String(s),
        serde_json::Value::Array(arr) => {
            aegis_document::Value::Array(arr.into_iter().map(json_to_doc_value).collect())
        }
        serde_json::Value::Object(map) => aegis_document::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_to_doc_value(v)))
                .collect(),
        ),
    }
}

/// Convert aegis_document::Value to JSON.
fn doc_value_to_json(value: &aegis_document::Value) -> serde_json::Value {
    match value {
        aegis_document::Value::Null => serde_json::Value::Null,
        aegis_document::Value::Bool(b) => serde_json::Value::Bool(*b),
        aegis_document::Value::Int(i) => serde_json::Value::Number((*i).into()),
        aegis_document::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        aegis_document::Value::String(s) => serde_json::Value::String(s.clone()),
        aegis_document::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(doc_value_to_json).collect())
        }
        aegis_document::Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), doc_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepared_statements() {
        let eng = QueryEngine::new();
        eng.execute("CREATE TABLE t (id INT, name VARCHAR(50))", None)
            .unwrap();
        eng.execute("INSERT INTO t VALUES (1,'a'),(2,'b'),(3,'a')", None)
            .unwrap();

        let id = eng
            .prepare("SELECT * FROM t WHERE name = $1", None)
            .unwrap();
        // Same prepared statement, different bound params.
        assert_eq!(
            eng.execute_prepared(&id, &[serde_json::json!("a")])
                .unwrap()
                .rows
                .len(),
            2
        );
        assert_eq!(
            eng.execute_prepared(&id, &[serde_json::json!("b")])
                .unwrap()
                .rows
                .len(),
            1
        );
        assert_eq!(eng.prepared_count(), 1);

        assert!(eng.deallocate(&id));
        assert_eq!(eng.prepared_count(), 0);
        assert!(eng
            .execute_prepared(&id, &[serde_json::json!("a")])
            .is_err());
    }

    #[test]
    fn test_graph_update_and_delete_edge() {
        let g = GraphStore::new();
        let a = g.create_node("Person", serde_json::json!({"name": "A"}));
        let b = g.create_node("Person", serde_json::json!({"name": "B"}));
        let edge = g.create_edge(&a.id, &b.id, "KNOWS").unwrap();

        // Update node: change properties, keep label.
        let updated = g
            .update_node(&a.id, None, Some(serde_json::json!({"name": "A2"})))
            .unwrap();
        assert_eq!(updated.label, "Person");
        assert_eq!(updated.properties["name"], "A2");

        // Update edge relationship.
        let updated_edge = g.update_edge(&edge.id, "FOLLOWS".to_string()).unwrap();
        assert_eq!(updated_edge.relationship, "FOLLOWS");

        // Delete the edge (previously unroutable).
        assert!(g.delete_edge(&edge.id).is_ok());
        assert!(g.list_edges().is_empty());

        // Updating / deleting a missing id errors.
        assert!(g.update_node("nope:0", None, None).is_err());
        assert!(g.update_edge("nope", "X".to_string()).is_err());
        assert!(g.delete_edge("nope").is_err());
    }

    #[test]
    fn test_metrics_calculations() {
        let metrics = Metrics {
            total_requests: 100,
            failed_requests: 10,
            total_duration_ms: 5000,
        };

        assert_eq!(metrics.avg_duration_ms(), 50.0);
        assert!((metrics.success_rate() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_value_to_json() {
        let value = aegis_common::Value::String("test".to_string());
        let json = value_to_json(value);
        assert_eq!(json, serde_json::Value::String("test".to_string()));
    }

    #[test]
    fn test_kv_store_operations() {
        let store = KvStore::new();

        // Set
        let entry = store.set("key1".to_string(), serde_json::json!("value1"), None);
        assert_eq!(entry.key, "key1");
        assert_eq!(entry.value, serde_json::json!("value1"));

        // Get
        let retrieved = store.get("key1").expect("key1 should exist after set");
        assert_eq!(retrieved.value, serde_json::json!("value1"));

        // List
        store.set("key2".to_string(), serde_json::json!("value2"), None);
        let all = store.list(None, 100);
        assert_eq!(all.len(), 2);

        // Delete
        let deleted = store.delete("key1");
        assert!(deleted.is_some());
        assert!(store.get("key1").is_none());
    }

    #[test]
    fn test_kv_ttl_expiration() {
        let store = KvStore::new();

        // A key whose TTL has already elapsed (we backdate updated_at) must read
        // as absent and be excluded from list/count.
        store.set("ephemeral".to_string(), serde_json::json!("x"), Some(1));
        {
            let mut data = store.data.write();
            if let Some(e) = data.get_mut("ephemeral") {
                e.updated_at = chrono::Utc::now() - chrono::Duration::seconds(10);
            }
        }
        assert!(
            store.get("ephemeral").is_none(),
            "expired key must not be returned"
        );

        store.set("permanent".to_string(), serde_json::json!("y"), None);
        let listed = store.list(None, 100);
        assert_eq!(listed.len(), 1, "expired key must be excluded from list");
        assert_eq!(listed[0].key, "permanent");
        assert_eq!(store.count(), 1, "expired key must be excluded from count");
    }

    #[test]
    fn test_transaction_commit() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE txn_test (id INT, name VARCHAR(50))", None)
            .unwrap();

        // Multi-statement transaction with COMMIT
        let result = engine.execute(
            "BEGIN; INSERT INTO txn_test VALUES (1, 'Alice'); INSERT INTO txn_test VALUES (2, 'Bob'); COMMIT",
            None,
        ).unwrap();
        assert_eq!(
            result.rows[0][0],
            serde_json::Value::String("COMMIT".to_string())
        );

        // Data should be visible after commit
        let select = engine.execute("SELECT * FROM txn_test", None).unwrap();
        assert_eq!(select.rows.len(), 2);
    }

    #[test]
    fn test_transaction_rollback() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE txn_rb (id INT, name VARCHAR(50))", None)
            .unwrap();
        engine
            .execute("INSERT INTO txn_rb VALUES (1, 'Original')", None)
            .unwrap();

        // Transaction with ROLLBACK — inserts should be undone
        let result = engine.execute(
            "BEGIN; INSERT INTO txn_rb VALUES (2, 'Should vanish'); INSERT INTO txn_rb VALUES (3, 'Also gone'); ROLLBACK",
            None,
        ).unwrap();
        assert_eq!(
            result.rows[0][0],
            serde_json::Value::String("ROLLBACK".to_string())
        );

        // Only the original row should remain
        let select = engine.execute("SELECT * FROM txn_rb", None).unwrap();
        assert_eq!(select.rows.len(), 1);
    }

    #[test]
    fn test_transaction_auto_rollback_on_missing_commit() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE txn_nocommit (id INT)", None)
            .unwrap();
        engine
            .execute("INSERT INTO txn_nocommit VALUES (1)", None)
            .unwrap();

        // BEGIN without COMMIT should error and auto-rollback
        let result = engine.execute("BEGIN; INSERT INTO txn_nocommit VALUES (2)", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not committed"));

        // Original row should still be there, uncommitted row should not
        let select = engine.execute("SELECT * FROM txn_nocommit", None).unwrap();
        assert_eq!(select.rows.len(), 1);
    }

    #[test]
    fn test_transaction_auto_rollback_on_error() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE txn_err (id INT, name VARCHAR(50))", None)
            .unwrap();
        engine
            .execute("INSERT INTO txn_err VALUES (1, 'Keep')", None)
            .unwrap();

        // Transaction with an error mid-way — should auto-rollback
        let result = engine.execute(
            "BEGIN; INSERT INTO txn_err VALUES (2, 'Lose'); INSERT INTO nonexistent VALUES (3, 'Fail'); COMMIT",
            None,
        );
        assert!(result.is_err());

        // Only the pre-transaction row should remain
        let select = engine.execute("SELECT * FROM txn_err", None).unwrap();
        assert_eq!(select.rows.len(), 1);
    }

    #[test]
    fn test_single_statement_still_works() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE single (id INT)", None)
            .unwrap();
        engine
            .execute("INSERT INTO single VALUES (1)", None)
            .unwrap();
        engine
            .execute("INSERT INTO single VALUES (2)", None)
            .unwrap();

        let select = engine.execute("SELECT * FROM single", None).unwrap();
        assert_eq!(select.rows.len(), 2);
    }

    #[test]
    fn test_parameterized_insert() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE param_test (id INT, name VARCHAR(50))", None)
            .unwrap();

        // Insert with $1, $2 placeholders
        engine
            .execute_with_params(
                "INSERT INTO param_test VALUES ($1, $2)",
                None,
                &[serde_json::json!(1), serde_json::json!("Alice")],
            )
            .unwrap();

        engine
            .execute_with_params(
                "INSERT INTO param_test VALUES ($1, $2)",
                None,
                &[serde_json::json!(2), serde_json::json!("Bob")],
            )
            .unwrap();

        let select = engine.execute("SELECT * FROM param_test", None).unwrap();
        assert_eq!(select.rows.len(), 2);
        assert_eq!(
            select.rows[0][1],
            serde_json::Value::String("Alice".to_string())
        );
        assert_eq!(
            select.rows[1][1],
            serde_json::Value::String("Bob".to_string())
        );
    }

    #[test]
    fn test_parameterized_select() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE param_sel (id INT, name VARCHAR(50))", None)
            .unwrap();
        engine
            .execute("INSERT INTO param_sel VALUES (1, 'Alice')", None)
            .unwrap();
        engine
            .execute("INSERT INTO param_sel VALUES (2, 'Bob')", None)
            .unwrap();
        engine
            .execute("INSERT INTO param_sel VALUES (3, 'Charlie')", None)
            .unwrap();

        // SELECT with parameterized WHERE
        let result = engine
            .execute_with_params(
                "SELECT * FROM param_sel WHERE id = $1",
                None,
                &[serde_json::json!(2)],
            )
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0][1],
            serde_json::Value::String("Bob".to_string())
        );
    }

    #[test]
    fn test_parameterized_update() {
        let engine = QueryEngine::new();
        engine
            .execute("CREATE TABLE param_upd (id INT, name VARCHAR(50))", None)
            .unwrap();
        engine
            .execute("INSERT INTO param_upd VALUES (1, 'Alice')", None)
            .unwrap();

        engine
            .execute_with_params(
                "UPDATE param_upd SET name = $1 WHERE id = $2",
                None,
                &[serde_json::json!("Alicia"), serde_json::json!(1)],
            )
            .unwrap();

        let result = engine
            .execute("SELECT * FROM param_upd WHERE id = 1", None)
            .unwrap();
        assert_eq!(
            result.rows[0][1],
            serde_json::Value::String("Alicia".to_string())
        );
    }
}
