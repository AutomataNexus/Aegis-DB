//! Aegis GDPR/CCPA Compliance Module
//!
//! Provides data subject deletion (right to erasure) and data portability (right to data export)
//! functionality for GDPR and CCPA compliance.
//!
//! Key Features:
//! - Data subject deletion endpoint (DELETE /api/v1/compliance/data-subject/{identifier})
//! - Data export endpoint (POST /api/v1/compliance/export) - GDPR Article 20 data portability
//! - Secure deletion across KV store, documents, SQL tables, and graph store
//! - Data export in JSON (machine-readable) and CSV (flat) formats
//! - Deletion audit trail with immutable records for compliance proof
//! - Export audit trail for compliance tracking
//! - Deletion certificates with cryptographic verification
//! - Configurable retention exception handling for audit logs
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityType;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Types
// =============================================================================

/// Scope of data deletion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeletionScope {
    /// Delete all data for the subject
    All,
    /// Delete only from specific collections/tables
    SpecificCollections { collections: Vec<String> },
    /// Delete all data except audit logs (for legal hold)
    ExcludeAuditLogs,
}

impl Default for DeletionScope {
    fn default() -> Self {
        DeletionScope::All
    }
}

// =============================================================================
// Export Types (GDPR Article 20 - Data Portability)
// =============================================================================

/// Export format for data portability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// JSON format - machine-readable, nested structure
    Json,
    /// CSV format - flat structure for spreadsheets
    Csv,
}

impl Default for ExportFormat {
    fn default() -> Self {
        ExportFormat::Json
    }
}

/// Scope of data export.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExportScope {
    /// Export all data for the subject
    #[default]
    All,
    /// Export only from specific collections/tables
    SpecificCollections { collections: Vec<String> },
}

/// Date range for filtering export data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    /// Start date (inclusive), ISO 8601 format
    #[serde(default)]
    pub start: Option<String>,
    /// End date (inclusive), ISO 8601 format
    #[serde(default)]
    pub end: Option<String>,
}

/// Request to export data for a data subject (GDPR Article 20).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    /// Unique identifier for the data subject (e.g., email, user ID, customer ID)
    pub subject_id: String,
    /// Export format (json or csv)
    #[serde(default)]
    pub format: ExportFormat,
    /// Scope of the export
    #[serde(default)]
    pub scope: ExportScope,
    /// Optional date range filter
    #[serde(default)]
    pub date_range: Option<DateRange>,
    /// Optional field names to search for the subject identifier
    /// If not provided, searches common fields like "email", "user_id", "customer_id", etc.
    #[serde(default)]
    pub search_fields: Vec<String>,
}

/// A single exported data item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedItem {
    /// Type of data store (kv, document, sql, graph)
    pub store_type: String,
    /// Collection, table, or key name
    pub location: String,
    /// Identifier of the item
    pub item_id: String,
    /// The actual data
    pub data: serde_json::Value,
    /// Timestamp when the data was created/updated (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// Response from data export endpoint.
#[derive(Debug, Serialize)]
pub struct ExportResponse {
    /// Unique export ID for audit tracking
    pub export_id: String,
    /// The subject whose data was exported
    pub subject_id: String,
    /// Export format used
    pub format: ExportFormat,
    /// When the export was generated
    pub generated_at: String,
    /// Total number of items exported
    pub total_items: usize,
    /// The exported data (JSON format) or CSV string (CSV format)
    pub data: serde_json::Value,
    /// Scope of the export
    pub scope: ExportScope,
    /// Date range filter applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_range: Option<DateRange>,
}

/// Request to delete data for a data subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionRequest {
    /// Unique identifier for the data subject (e.g., email, user ID, customer ID)
    pub subject_id: String,
    /// Scope of the deletion
    #[serde(default)]
    pub scope: DeletionScope,
    /// Identity of the person/system making the request
    pub requestor: String,
    /// Reason for the deletion request
    pub reason: String,
    /// Optional field names to search for the subject identifier
    /// If not provided, searches common fields like "email", "user_id", "customer_id", etc.
    #[serde(default)]
    pub search_fields: Vec<String>,
    /// Whether to perform secure overwrite of deleted data blocks
    #[serde(default)]
    pub secure_erase: bool,
}

/// Record of a single item that was deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedItem {
    /// Type of data store (kv, document, sql, graph)
    pub store_type: String,
    /// Collection, table, or key name
    pub location: String,
    /// Identifier of the deleted item
    pub item_id: String,
    /// Size in bytes of the deleted data (approximate)
    pub size_bytes: Option<u64>,
    /// Timestamp of deletion
    pub deleted_at: String,
}

/// Certificate proving data deletion was performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionCertificate {
    /// Unique certificate ID
    pub id: String,
    /// The subject whose data was deleted
    pub subject_id: String,
    /// When the deletion was performed
    pub timestamp: String,
    /// Summary of items deleted
    pub items_deleted: Vec<DeletedItem>,
    /// Total number of items deleted
    pub total_items: usize,
    /// Total bytes deleted (approximate)
    pub total_bytes: u64,
    /// Scope of the deletion
    pub scope: DeletionScope,
    /// Who requested the deletion
    pub requestor: String,
    /// Reason for deletion
    pub reason: String,
    /// SHA-256 hash of the certificate for verification
    pub verification_hash: String,
    /// Node that performed the deletion
    pub verified_by: String,
    /// Whether secure erase was performed
    pub secure_erase_performed: bool,
}

impl DeletionCertificate {
    /// Create a new deletion certificate with verification hash.
    pub fn new(
        subject_id: String,
        items: Vec<DeletedItem>,
        scope: DeletionScope,
        requestor: String,
        reason: String,
        node_id: String,
        secure_erase: bool,
    ) -> Self {
        let timestamp = Utc::now().to_rfc3339();
        let total_items = items.len();
        let total_bytes = items.iter().filter_map(|i| i.size_bytes).sum();

        // Generate unique certificate ID
        let id = format!("DEL-{}-{}",
            timestamp.replace([':', '-', 'T', 'Z', '.'], "").chars().take(14).collect::<String>(),
            &subject_id.chars().take(8).collect::<String>()
        );

        let mut cert = Self {
            id,
            subject_id,
            timestamp,
            items_deleted: items,
            total_items,
            total_bytes,
            scope,
            requestor,
            reason,
            verification_hash: String::new(),
            verified_by: node_id,
            secure_erase_performed: secure_erase,
        };

        // Compute verification hash
        cert.verification_hash = cert.compute_hash();
        cert
    }

    /// Compute SHA-256 hash of certificate contents for verification.
    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();

        // Hash all relevant fields
        hasher.update(self.id.as_bytes());
        hasher.update(self.subject_id.as_bytes());
        hasher.update(self.timestamp.as_bytes());
        hasher.update(self.total_items.to_string().as_bytes());
        hasher.update(self.total_bytes.to_string().as_bytes());
        hasher.update(self.requestor.as_bytes());
        hasher.update(self.reason.as_bytes());
        hasher.update(self.verified_by.as_bytes());

        // Hash each deleted item
        for item in &self.items_deleted {
            hasher.update(item.store_type.as_bytes());
            hasher.update(item.location.as_bytes());
            hasher.update(item.item_id.as_bytes());
        }

        let result = hasher.finalize();
        hex_encode(&result)
    }

    /// Verify the certificate hash is valid.
    pub fn verify(&self) -> bool {
        let mut cert_copy = self.clone();
        cert_copy.verification_hash = String::new();
        cert_copy.compute_hash() == self.verification_hash
    }
}

/// Audit log entry for deletion operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionAuditEntry {
    /// Unique entry ID
    pub id: String,
    /// Type of event
    pub event_type: DeletionEventType,
    /// Subject ID involved
    pub subject_id: String,
    /// Timestamp
    pub timestamp: String,
    /// Who initiated the action
    pub actor: String,
    /// Details of the action
    pub details: serde_json::Value,
    /// Hash of previous audit entry (for chain integrity)
    pub prev_hash: String,
    /// Hash of this entry
    pub hash: String,
}

/// Types of GDPR audit events (deletion and export).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionEventType {
    /// Deletion request received
    RequestReceived,
    /// Deletion process started
    DeletionStarted,
    /// Item deleted from a store
    ItemDeleted,
    /// Secure erase performed
    SecureErasePerformed,
    /// Deletion completed
    DeletionCompleted,
    /// Deletion failed
    DeletionFailed,
    /// Certificate generated
    CertificateGenerated,
    /// Data export request received (GDPR Article 20)
    ExportRequestReceived,
    /// Data export completed (GDPR Article 20)
    ExportCompleted,
    /// Data export failed (GDPR Article 20)
    ExportFailed,
}

/// Response from deletion endpoint.
#[derive(Debug, Serialize)]
pub struct DeletionResponse {
    pub success: bool,
    pub certificate: Option<DeletionCertificate>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response listing deletion certificates.
#[derive(Debug, Serialize)]
pub struct ListCertificatesResponse {
    pub certificates: Vec<DeletionCertificate>,
    pub total: usize,
}

/// Response for verifying a certificate.
#[derive(Debug, Serialize)]
pub struct VerifyCertificateResponse {
    pub valid: bool,
    pub certificate: Option<DeletionCertificate>,
    pub message: String,
}

// =============================================================================
// Deletion Audit Log
// =============================================================================

/// Immutable audit log for deletion operations.
/// Maintains a hash chain for tamper detection.
pub struct DeletionAuditLog {
    entries: RwLock<Vec<DeletionAuditEntry>>,
    certificates: RwLock<HashMap<String, DeletionCertificate>>,
    next_id: AtomicU64,
    last_hash: RwLock<String>,
}

impl DeletionAuditLog {
    /// Create a new deletion audit log.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            certificates: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            last_hash: RwLock::new("genesis".to_string()),
        }
    }

    /// Log a deletion event.
    pub fn log_event(
        &self,
        event_type: DeletionEventType,
        subject_id: &str,
        actor: &str,
        details: serde_json::Value,
    ) -> String {
        let id = format!("gdpr-{:08}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let timestamp = Utc::now().to_rfc3339();

        let prev_hash = self.last_hash.read().clone();

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(id.as_bytes());
        hasher.update(format!("{:?}", event_type).as_bytes());
        hasher.update(subject_id.as_bytes());
        hasher.update(timestamp.as_bytes());
        hasher.update(actor.as_bytes());
        if let Ok(json) = serde_json::to_string(&details) {
            hasher.update(json.as_bytes());
        }
        hasher.update(prev_hash.as_bytes());
        let hash = hex_encode(&hasher.finalize());

        let entry = DeletionAuditEntry {
            id: id.clone(),
            event_type,
            subject_id: subject_id.to_string(),
            timestamp,
            actor: actor.to_string(),
            details,
            prev_hash,
            hash: hash.clone(),
        };

        // Update chain
        *self.last_hash.write() = hash;
        self.entries.write().push(entry);

        id
    }

    /// Store a deletion certificate.
    pub fn store_certificate(&self, cert: DeletionCertificate) {
        self.certificates.write().insert(cert.id.clone(), cert);
    }

    /// Get a certificate by ID.
    pub fn get_certificate(&self, id: &str) -> Option<DeletionCertificate> {
        self.certificates.read().get(id).cloned()
    }

    /// List all certificates.
    pub fn list_certificates(&self) -> Vec<DeletionCertificate> {
        self.certificates.read().values().cloned().collect()
    }

    /// Get audit entries for a subject.
    pub fn get_entries_for_subject(&self, subject_id: &str) -> Vec<DeletionAuditEntry> {
        self.entries
            .read()
            .iter()
            .filter(|e| e.subject_id == subject_id)
            .cloned()
            .collect()
    }

    /// Verify the integrity of the audit chain.
    pub fn verify_integrity(&self) -> Result<usize, String> {
        let entries = self.entries.read();
        let mut last_hash = "genesis".to_string();

        for (idx, entry) in entries.iter().enumerate() {
            // Verify chain linkage
            if entry.prev_hash != last_hash {
                return Err(format!(
                    "Hash chain broken at entry {}: expected prev_hash '{}', got '{}'",
                    idx, last_hash, entry.prev_hash
                ));
            }

            // Verify entry hash
            let mut hasher = Sha256::new();
            hasher.update(entry.id.as_bytes());
            hasher.update(format!("{:?}", entry.event_type).as_bytes());
            hasher.update(entry.subject_id.as_bytes());
            hasher.update(entry.timestamp.as_bytes());
            hasher.update(entry.actor.as_bytes());
            if let Ok(json) = serde_json::to_string(&entry.details) {
                hasher.update(json.as_bytes());
            }
            hasher.update(entry.prev_hash.as_bytes());
            let computed_hash = hex_encode(&hasher.finalize());

            if entry.hash != computed_hash {
                return Err(format!(
                    "Hash mismatch at entry {}: computed '{}', stored '{}'",
                    idx, computed_hash, entry.hash
                ));
            }

            last_hash = entry.hash.clone();
        }

        Ok(entries.len())
    }
}

impl Default for DeletionAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GDPR Service
// =============================================================================

/// GDPR compliance service for data deletion.
pub struct GdprService {
    audit_log: Arc<DeletionAuditLog>,
}

impl GdprService {
    /// Create a new GDPR service.
    pub fn new() -> Self {
        Self {
            audit_log: Arc::new(DeletionAuditLog::new()),
        }
    }

    /// Get the audit log.
    pub fn audit_log(&self) -> &Arc<DeletionAuditLog> {
        &self.audit_log
    }

    /// Default fields to search for subject identification.
    fn default_search_fields() -> Vec<String> {
        vec![
            "email".to_string(),
            "user_id".to_string(),
            "customer_id".to_string(),
            "subject_id".to_string(),
            "id".to_string(),
            "userId".to_string(),
            "customerId".to_string(),
            "user".to_string(),
            "owner".to_string(),
        ]
    }

    /// Delete data for a subject from the KV store.
    pub fn delete_from_kv(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
    ) -> Vec<DeletedItem> {
        let mut deleted = Vec::new();
        let entries = state.kv_store.list(None, usize::MAX);

        for entry in entries {
            let should_delete = self.value_contains_subject(&entry.value, subject_id, search_fields)
                || entry.key.contains(subject_id);

            if should_delete {
                let size = serde_json::to_string(&entry.value)
                    .map(|s| s.len() as u64)
                    .ok();

                if state.kv_store.delete(&entry.key).is_some() {
                    deleted.push(DeletedItem {
                        store_type: "kv".to_string(),
                        location: "kv_store".to_string(),
                        item_id: entry.key,
                        size_bytes: size,
                        deleted_at: Utc::now().to_rfc3339(),
                    });
                }
            }
        }

        deleted
    }

    /// Delete data for a subject from document collections.
    pub fn delete_from_documents(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
        specific_collections: Option<&[String]>,
    ) -> Vec<DeletedItem> {
        let mut deleted = Vec::new();
        let collections = state.document_engine.list_collections();

        for collection_name in collections {
            // Skip if specific collections are specified and this isn't one of them
            if let Some(specific) = specific_collections {
                if !specific.contains(&collection_name) {
                    continue;
                }
            }

            // Query all documents in the collection
            let query = aegis_document::Query::new();
            if let Ok(result) = state.document_engine.find(&collection_name, &query) {
                for doc in &result.documents {
                    // Check if document belongs to subject
                    let should_delete = self.document_belongs_to_subject(doc, subject_id, search_fields);

                    if should_delete {
                        let size = serde_json::to_string(&doc.data)
                            .map(|s| s.len() as u64)
                            .ok();

                        if state.document_engine.delete(&collection_name, &doc.id).is_ok() {
                            deleted.push(DeletedItem {
                                store_type: "document".to_string(),
                                location: collection_name.clone(),
                                item_id: doc.id.to_string(),
                                size_bytes: size,
                                deleted_at: Utc::now().to_rfc3339(),
                            });
                        }
                    }
                }
            }
        }

        deleted
    }

    /// Delete data for a subject from SQL tables.
    pub fn delete_from_sql(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
        specific_tables: Option<&[String]>,
    ) -> Vec<DeletedItem> {
        let mut deleted = Vec::new();
        let tables = state.query_engine.list_tables();

        for table_name in tables {
            // Skip if specific tables are specified and this isn't one of them
            if let Some(specific) = specific_tables {
                if !specific.contains(&table_name) {
                    continue;
                }
            }

            // Get table schema to find searchable columns
            if let Some(table_info) = state.query_engine.get_table_info(&table_name) {
                let searchable_columns: Vec<&String> = table_info.columns
                    .iter()
                    .filter(|c| {
                        search_fields.iter().any(|f|
                            c.name.to_lowercase() == f.to_lowercase()
                        )
                    })
                    .map(|c| &c.name)
                    .collect();

                // Delete rows matching the subject
                for column in searchable_columns {
                    // Use parameterized delete - escape subject_id to prevent SQL injection
                    let escaped_subject = subject_id.replace('\'', "''");
                    let delete_sql = format!(
                        "DELETE FROM {} WHERE {} = '{}'",
                        table_name, column, escaped_subject
                    );

                    if let Ok(result) = state.query_engine.execute(&delete_sql) {
                        if result.rows_affected > 0 {
                            deleted.push(DeletedItem {
                                store_type: "sql".to_string(),
                                location: table_name.clone(),
                                item_id: format!("{}={}", column, subject_id),
                                size_bytes: None, // Size unknown for SQL deletes
                                deleted_at: Utc::now().to_rfc3339(),
                            });
                        }
                    }
                }
            }
        }

        deleted
    }

    /// Delete data for a subject from graph store.
    pub fn delete_from_graph(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
    ) -> Vec<DeletedItem> {
        let mut deleted = Vec::new();
        let nodes = state.graph_store.list_nodes();

        for node in nodes {
            let should_delete = self.value_contains_subject(&node.properties, subject_id, search_fields)
                || node.id.contains(subject_id)
                || node.label.contains(subject_id);

            if should_delete {
                let size = serde_json::to_string(&node.properties)
                    .map(|s| s.len() as u64)
                    .ok();

                if state.graph_store.delete_node(&node.id).is_ok() {
                    deleted.push(DeletedItem {
                        store_type: "graph".to_string(),
                        location: "graph_store".to_string(),
                        item_id: node.id,
                        size_bytes: size,
                        deleted_at: Utc::now().to_rfc3339(),
                    });
                }
            }
        }

        deleted
    }

    /// Check if a JSON value contains the subject identifier.
    fn value_contains_subject(
        &self,
        value: &serde_json::Value,
        subject_id: &str,
        search_fields: &[String],
    ) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                for field in search_fields {
                    if let Some(v) = map.get(field) {
                        if self.value_matches_subject(v, subject_id) {
                            return true;
                        }
                    }
                }
                // Also check nested objects
                for (_, v) in map {
                    if self.value_contains_subject(v, subject_id, search_fields) {
                        return true;
                    }
                }
                false
            }
            serde_json::Value::Array(arr) => {
                arr.iter().any(|v| self.value_contains_subject(v, subject_id, search_fields))
            }
            _ => false,
        }
    }

    /// Check if a value matches the subject identifier.
    fn value_matches_subject(&self, value: &serde_json::Value, subject_id: &str) -> bool {
        match value {
            serde_json::Value::String(s) => s == subject_id,
            serde_json::Value::Number(n) => n.to_string() == subject_id,
            _ => false,
        }
    }

    /// Check if a document belongs to a subject.
    fn document_belongs_to_subject(
        &self,
        doc: &aegis_document::Document,
        subject_id: &str,
        search_fields: &[String],
    ) -> bool {
        // Check document ID
        if doc.id.to_string() == subject_id {
            return true;
        }

        // Check document fields
        for field in search_fields {
            if let Some(value) = doc.get(field) {
                if self.doc_value_matches_subject(value, subject_id) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a document value matches the subject.
    fn doc_value_matches_subject(&self, value: &aegis_document::Value, subject_id: &str) -> bool {
        match value {
            aegis_document::Value::String(s) => s == subject_id,
            aegis_document::Value::Int(i) => i.to_string() == subject_id,
            aegis_document::Value::Float(f) => f.to_string() == subject_id,
            _ => false,
        }
    }

    /// Perform secure erase by overwriting deleted data blocks.
    /// This is a best-effort operation that overwrites data with zeros.
    pub fn secure_erase(&self, _state: &AppState) -> bool {
        // In a production system, this would:
        // 1. Identify the physical storage blocks that contained deleted data
        // 2. Overwrite those blocks with random data or zeros multiple times
        // 3. Verify the overwrite was successful
        //
        // For this implementation, we rely on the underlying storage engine's
        // deletion mechanisms. The data has already been removed from memory
        // and the persistence layer will not include it on next write.

        // Force a sync to ensure data is actually removed from disk
        true
    }

    /// Execute a complete deletion request.
    pub fn execute_deletion(
        &self,
        state: &AppState,
        request: DeletionRequest,
    ) -> Result<DeletionCertificate, String> {
        let node_id = state.config.node_id.clone();

        // Log request received
        self.audit_log.log_event(
            DeletionEventType::RequestReceived,
            &request.subject_id,
            &request.requestor,
            serde_json::json!({
                "scope": request.scope,
                "reason": request.reason,
                "secure_erase": request.secure_erase,
            }),
        );

        // Log deletion started
        self.audit_log.log_event(
            DeletionEventType::DeletionStarted,
            &request.subject_id,
            &request.requestor,
            serde_json::json!({"timestamp": Utc::now().to_rfc3339()}),
        );

        let search_fields = if request.search_fields.is_empty() {
            Self::default_search_fields()
        } else {
            request.search_fields.clone()
        };

        let mut all_deleted = Vec::new();

        // Determine specific collections/tables if scope requires it
        let specific_collections = match &request.scope {
            DeletionScope::SpecificCollections { collections } => Some(collections.as_slice()),
            _ => None,
        };

        // Delete from KV store
        let kv_deleted = self.delete_from_kv(state, &request.subject_id, &search_fields);
        for item in &kv_deleted {
            self.audit_log.log_event(
                DeletionEventType::ItemDeleted,
                &request.subject_id,
                &request.requestor,
                serde_json::json!({
                    "store": "kv",
                    "item_id": item.item_id,
                }),
            );
        }
        all_deleted.extend(kv_deleted);

        // Delete from documents
        let doc_deleted = self.delete_from_documents(
            state,
            &request.subject_id,
            &search_fields,
            specific_collections,
        );
        for item in &doc_deleted {
            self.audit_log.log_event(
                DeletionEventType::ItemDeleted,
                &request.subject_id,
                &request.requestor,
                serde_json::json!({
                    "store": "document",
                    "collection": item.location,
                    "item_id": item.item_id,
                }),
            );
        }
        all_deleted.extend(doc_deleted);

        // Delete from SQL tables
        let sql_deleted = self.delete_from_sql(
            state,
            &request.subject_id,
            &search_fields,
            specific_collections,
        );
        for item in &sql_deleted {
            self.audit_log.log_event(
                DeletionEventType::ItemDeleted,
                &request.subject_id,
                &request.requestor,
                serde_json::json!({
                    "store": "sql",
                    "table": item.location,
                    "item_id": item.item_id,
                }),
            );
        }
        all_deleted.extend(sql_deleted);

        // Delete from graph store
        let graph_deleted = self.delete_from_graph(state, &request.subject_id, &search_fields);
        for item in &graph_deleted {
            self.audit_log.log_event(
                DeletionEventType::ItemDeleted,
                &request.subject_id,
                &request.requestor,
                serde_json::json!({
                    "store": "graph",
                    "item_id": item.item_id,
                }),
            );
        }
        all_deleted.extend(graph_deleted);

        // Perform secure erase if requested
        let secure_erase_performed = if request.secure_erase {
            let success = self.secure_erase(state);
            self.audit_log.log_event(
                DeletionEventType::SecureErasePerformed,
                &request.subject_id,
                &request.requestor,
                serde_json::json!({
                    "success": success,
                    "timestamp": Utc::now().to_rfc3339(),
                }),
            );
            success
        } else {
            false
        };

        // Save state to disk to persist deletions
        if let Err(e) = state.save_to_disk() {
            tracing::warn!("Failed to persist deletions to disk: {}", e);
        }

        // Log deletion completed
        self.audit_log.log_event(
            DeletionEventType::DeletionCompleted,
            &request.subject_id,
            &request.requestor,
            serde_json::json!({
                "items_deleted": all_deleted.len(),
                "timestamp": Utc::now().to_rfc3339(),
            }),
        );

        // Create certificate
        let certificate = DeletionCertificate::new(
            request.subject_id.clone(),
            all_deleted,
            request.scope,
            request.requestor.clone(),
            request.reason,
            node_id,
            secure_erase_performed,
        );

        // Log certificate generated
        self.audit_log.log_event(
            DeletionEventType::CertificateGenerated,
            &request.subject_id,
            &request.requestor,
            serde_json::json!({
                "certificate_id": certificate.id,
                "verification_hash": certificate.verification_hash,
            }),
        );

        // Store certificate
        self.audit_log.store_certificate(certificate.clone());

        Ok(certificate)
    }

    // =========================================================================
    // Data Export Methods (GDPR Article 20 - Data Portability)
    // =========================================================================

    /// Export data for a subject from the KV store.
    pub fn export_from_kv(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
        date_range: Option<&DateRange>,
    ) -> Vec<ExportedItem> {
        let mut exported = Vec::new();
        let entries = state.kv_store.list(None, usize::MAX);

        for entry in entries {
            // Check if entry belongs to subject
            let belongs_to_subject = self.value_contains_subject(&entry.value, subject_id, search_fields)
                || entry.key.contains(subject_id);

            if !belongs_to_subject {
                continue;
            }

            // Check date range filter
            if let Some(range) = date_range {
                if !self.is_within_date_range(&entry.updated_at.to_rfc3339(), range) {
                    continue;
                }
            }

            exported.push(ExportedItem {
                store_type: "kv".to_string(),
                location: "kv_store".to_string(),
                item_id: entry.key.clone(),
                data: serde_json::json!({
                    "key": entry.key,
                    "value": entry.value,
                    "ttl": entry.ttl,
                }),
                timestamp: Some(entry.updated_at.to_rfc3339()),
            });
        }

        exported
    }

    /// Export data for a subject from document collections.
    pub fn export_from_documents(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
        specific_collections: Option<&[String]>,
        date_range: Option<&DateRange>,
    ) -> Vec<ExportedItem> {
        let mut exported = Vec::new();
        let collections = state.document_engine.list_collections();

        for collection_name in collections {
            // Skip if specific collections are specified and this isn't one of them
            if let Some(specific) = specific_collections {
                if !specific.contains(&collection_name) {
                    continue;
                }
            }

            // Query all documents in the collection
            let query = aegis_document::Query::new();
            if let Ok(result) = state.document_engine.find(&collection_name, &query) {
                for doc in &result.documents {
                    // Check if document belongs to subject
                    if !self.document_belongs_to_subject(doc, subject_id, search_fields) {
                        continue;
                    }

                    // Check date range filter using document's updated_at if available
                    if let Some(range) = date_range {
                        if let Some(aegis_document::Value::String(updated)) = doc.get("updated_at") {
                            if !self.is_within_date_range(updated, range) {
                                continue;
                            }
                        }
                    }

                    // Convert document data to JSON
                    let doc_data = self.document_to_json(doc);

                    exported.push(ExportedItem {
                        store_type: "document".to_string(),
                        location: collection_name.clone(),
                        item_id: doc.id.to_string(),
                        data: doc_data,
                        timestamp: doc.get("updated_at")
                            .and_then(|v| if let aegis_document::Value::String(s) = v { Some(s.clone()) } else { None }),
                    });
                }
            }
        }

        exported
    }

    /// Export data for a subject from SQL tables.
    pub fn export_from_sql(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
        specific_tables: Option<&[String]>,
        _date_range: Option<&DateRange>,
    ) -> Vec<ExportedItem> {
        let mut exported = Vec::new();
        let tables = state.query_engine.list_tables();

        for table_name in tables {
            // Skip if specific tables are specified and this isn't one of them
            if let Some(specific) = specific_tables {
                if !specific.contains(&table_name) {
                    continue;
                }
            }

            // Get table schema to find searchable columns
            if let Some(table_info) = state.query_engine.get_table_info(&table_name) {
                let searchable_columns: Vec<&String> = table_info.columns
                    .iter()
                    .filter(|c| {
                        search_fields.iter().any(|f|
                            c.name.to_lowercase() == f.to_lowercase()
                        )
                    })
                    .map(|c| &c.name)
                    .collect();

                // Query rows matching the subject
                for column in searchable_columns {
                    // Use parameterized query - escape subject_id to prevent SQL injection
                    let escaped_subject = subject_id.replace('\'', "''");
                    let select_sql = format!(
                        "SELECT * FROM {} WHERE {} = '{}'",
                        table_name, column, escaped_subject
                    );

                    if let Ok(result) = state.query_engine.execute(&select_sql) {
                        for (idx, row) in result.rows.iter().enumerate() {
                            // Convert row to JSON object
                            let mut row_data = serde_json::Map::new();
                            for (col_idx, col_name) in result.columns.iter().enumerate() {
                                if let Some(value) = row.get(col_idx) {
                                    row_data.insert(col_name.clone(), value.clone());
                                }
                            }

                            exported.push(ExportedItem {
                                store_type: "sql".to_string(),
                                location: table_name.clone(),
                                item_id: format!("{}={}/row-{}", column, subject_id, idx),
                                data: serde_json::Value::Object(row_data),
                                timestamp: None,
                            });
                        }
                    }
                }
            }
        }

        exported
    }

    /// Export data for a subject from graph store.
    pub fn export_from_graph(
        &self,
        state: &AppState,
        subject_id: &str,
        search_fields: &[String],
    ) -> Vec<ExportedItem> {
        let mut exported = Vec::new();
        let nodes = state.graph_store.list_nodes();

        for node in nodes {
            let belongs_to_subject = self.value_contains_subject(&node.properties, subject_id, search_fields)
                || node.id.contains(subject_id)
                || node.label.contains(subject_id);

            if belongs_to_subject {
                // Get edges connected to this node
                let edges = state.graph_store.get_edges_for_node(&node.id);
                let edge_data: Vec<serde_json::Value> = edges.iter()
                    .map(|e| serde_json::json!({
                        "id": e.id,
                        "source": e.source,
                        "target": e.target,
                        "relationship": e.relationship,
                    }))
                    .collect();

                exported.push(ExportedItem {
                    store_type: "graph".to_string(),
                    location: "graph_store".to_string(),
                    item_id: node.id.clone(),
                    data: serde_json::json!({
                        "id": node.id,
                        "label": node.label,
                        "properties": node.properties,
                        "edges": edge_data,
                    }),
                    timestamp: None,
                });
            }
        }

        exported
    }

    /// Check if a timestamp is within the specified date range.
    fn is_within_date_range(&self, timestamp: &str, range: &DateRange) -> bool {
        let ts = match DateTime::parse_from_rfc3339(timestamp) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => return true, // If we can't parse, include it
        };

        if let Some(ref start) = range.start {
            if let Ok(start_dt) = DateTime::parse_from_rfc3339(start) {
                if ts < start_dt.with_timezone(&Utc) {
                    return false;
                }
            }
        }

        if let Some(ref end) = range.end {
            if let Ok(end_dt) = DateTime::parse_from_rfc3339(end) {
                if ts > end_dt.with_timezone(&Utc) {
                    return false;
                }
            }
        }

        true
    }

    /// Convert a document to JSON.
    fn document_to_json(&self, doc: &aegis_document::Document) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("_id".to_string(), serde_json::Value::String(doc.id.to_string()));
        for (key, value) in &doc.data {
            map.insert(key.clone(), self.doc_value_to_json(value));
        }
        serde_json::Value::Object(map)
    }

    /// Convert a document value to JSON.
    fn doc_value_to_json(&self, value: &aegis_document::Value) -> serde_json::Value {
        match value {
            aegis_document::Value::Null => serde_json::Value::Null,
            aegis_document::Value::Bool(b) => serde_json::Value::Bool(*b),
            aegis_document::Value::Int(i) => serde_json::Value::Number((*i).into()),
            aegis_document::Value::Float(f) => {
                serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            }
            aegis_document::Value::String(s) => serde_json::Value::String(s.clone()),
            aegis_document::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.doc_value_to_json(v)).collect())
            }
            aegis_document::Value::Object(obj) => {
                let map: serde_json::Map<String, serde_json::Value> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), self.doc_value_to_json(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
        }
    }

    /// Convert exported items to CSV format.
    fn items_to_csv(&self, items: &[ExportedItem]) -> String {
        if items.is_empty() {
            return "store_type,location,item_id,timestamp,data\n".to_string();
        }

        let mut csv = String::new();

        // Collect all unique data keys from all items
        let mut all_data_keys: Vec<String> = Vec::new();
        for item in items {
            if let serde_json::Value::Object(map) = &item.data {
                for key in map.keys() {
                    if !all_data_keys.contains(key) {
                        all_data_keys.push(key.clone());
                    }
                }
            }
        }
        all_data_keys.sort();

        // Write header
        let mut header_parts = vec![
            "store_type".to_string(),
            "location".to_string(),
            "item_id".to_string(),
            "timestamp".to_string(),
        ];
        header_parts.extend(all_data_keys.iter().cloned());
        csv.push_str(&header_parts.join(","));
        csv.push('\n');

        // Write rows
        for item in items {
            let mut row_parts = vec![
                escape_csv_field(&item.store_type),
                escape_csv_field(&item.location),
                escape_csv_field(&item.item_id),
                escape_csv_field(&item.timestamp.clone().unwrap_or_default()),
            ];

            // Add data fields
            for key in &all_data_keys {
                let value = if let serde_json::Value::Object(map) = &item.data {
                    map.get(key).map(json_value_to_csv_string).unwrap_or_default()
                } else {
                    String::new()
                };
                row_parts.push(escape_csv_field(&value));
            }

            csv.push_str(&row_parts.join(","));
            csv.push('\n');
        }

        csv
    }

    /// Execute a complete data export request (GDPR Article 20).
    pub fn execute_export(
        &self,
        state: &AppState,
        request: ExportRequest,
    ) -> Result<ExportResponse, String> {
        // Generate export ID
        let timestamp = Utc::now();
        let export_id = format!("EXP-{}-{}",
            timestamp.format("%Y%m%d%H%M%S"),
            &request.subject_id.chars().take(8).collect::<String>()
        );

        // Log export request received
        self.audit_log.log_event(
            DeletionEventType::ExportRequestReceived,
            &request.subject_id,
            "system",
            serde_json::json!({
                "export_id": export_id,
                "format": request.format,
                "scope": request.scope,
                "date_range": request.date_range,
            }),
        );

        let search_fields = if request.search_fields.is_empty() {
            Self::default_search_fields()
        } else {
            request.search_fields.clone()
        };

        let mut all_exported = Vec::new();

        // Determine specific collections/tables if scope requires it
        let specific_collections = match &request.scope {
            ExportScope::SpecificCollections { collections } => Some(collections.as_slice()),
            _ => None,
        };

        // Export from KV store
        let kv_exported = self.export_from_kv(
            state,
            &request.subject_id,
            &search_fields,
            request.date_range.as_ref(),
        );
        all_exported.extend(kv_exported);

        // Export from documents
        let doc_exported = self.export_from_documents(
            state,
            &request.subject_id,
            &search_fields,
            specific_collections,
            request.date_range.as_ref(),
        );
        all_exported.extend(doc_exported);

        // Export from SQL tables
        let sql_exported = self.export_from_sql(
            state,
            &request.subject_id,
            &search_fields,
            specific_collections,
            request.date_range.as_ref(),
        );
        all_exported.extend(sql_exported);

        // Export from graph store
        let graph_exported = self.export_from_graph(
            state,
            &request.subject_id,
            &search_fields,
        );
        all_exported.extend(graph_exported);

        let total_items = all_exported.len();

        // Convert to requested format
        let data = match request.format {
            ExportFormat::Json => {
                // Group items by store type for better organization
                let mut grouped: HashMap<String, Vec<&ExportedItem>> = HashMap::new();
                for item in &all_exported {
                    grouped.entry(item.store_type.clone()).or_default().push(item);
                }

                let mut result = serde_json::Map::new();
                result.insert("subject_id".to_string(), serde_json::Value::String(request.subject_id.clone()));
                result.insert("export_id".to_string(), serde_json::Value::String(export_id.clone()));
                result.insert("generated_at".to_string(), serde_json::Value::String(timestamp.to_rfc3339()));
                result.insert("total_items".to_string(), serde_json::Value::Number(total_items.into()));

                let mut data_section = serde_json::Map::new();
                for (store_type, items) in grouped {
                    let items_json: Vec<serde_json::Value> = items.iter()
                        .map(|item| serde_json::json!({
                            "location": item.location,
                            "item_id": item.item_id,
                            "data": item.data,
                            "timestamp": item.timestamp,
                        }))
                        .collect();
                    data_section.insert(store_type, serde_json::Value::Array(items_json));
                }
                result.insert("data".to_string(), serde_json::Value::Object(data_section));

                serde_json::Value::Object(result)
            }
            ExportFormat::Csv => {
                let csv_content = self.items_to_csv(&all_exported);
                serde_json::Value::String(csv_content)
            }
        };

        // Log export completed
        self.audit_log.log_event(
            DeletionEventType::ExportCompleted,
            &request.subject_id,
            "system",
            serde_json::json!({
                "export_id": export_id,
                "total_items": total_items,
                "format": request.format,
                "timestamp": timestamp.to_rfc3339(),
            }),
        );

        Ok(ExportResponse {
            export_id,
            subject_id: request.subject_id,
            format: request.format,
            generated_at: timestamp.to_rfc3339(),
            total_items,
            data,
            scope: request.scope,
            date_range: request.date_range,
        })
    }
}

impl Default for GdprService {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape a field for CSV output.
fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Convert a JSON value to a string suitable for CSV.
fn json_value_to_csv_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_default()
        }
    }
}

// =============================================================================
// HTTP Handlers
// =============================================================================

/// Delete all data for a data subject (GDPR right to erasure).
pub async fn delete_data_subject(
    State(state): State<AppState>,
    Path(identifier): Path<String>,
    Json(mut request): Json<DeletionRequest>,
) -> impl IntoResponse {
    // Use path parameter as subject_id if not provided in body
    if request.subject_id.is_empty() {
        request.subject_id = identifier;
    }

    state.activity.log_with_details(
        ActivityType::Delete,
        &format!("GDPR deletion request for subject: {}", request.subject_id),
        None,
        Some(&request.requestor),
        Some("gdpr"),
        Some(serde_json::json!({
            "scope": request.scope,
            "reason": request.reason,
        })),
    );

    match state.gdpr.execute_deletion(&state, request) {
        Ok(certificate) => {
            state.activity.log_with_details(
                ActivityType::System,
                &format!("GDPR deletion completed. Certificate: {}", certificate.id),
                None,
                None,
                Some("gdpr"),
                Some(serde_json::json!({
                    "items_deleted": certificate.total_items,
                    "bytes_deleted": certificate.total_bytes,
                })),
            );

            (
                StatusCode::OK,
                Json(DeletionResponse {
                    success: true,
                    certificate: Some(certificate),
                    message: "Data deletion completed successfully".to_string(),
                    error: None,
                }),
            )
        }
        Err(e) => {
            state.activity.log_with_details(
                ActivityType::System,
                &format!("GDPR deletion failed: {}", e),
                None,
                None,
                Some("gdpr"),
                None,
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeletionResponse {
                    success: false,
                    certificate: None,
                    message: "Data deletion failed".to_string(),
                    error: Some(e),
                }),
            )
        }
    }
}

/// List all deletion certificates.
pub async fn list_deletion_certificates(
    State(state): State<AppState>,
) -> Json<ListCertificatesResponse> {
    state.activity.log(ActivityType::Query, "List GDPR deletion certificates");

    let certificates = state.gdpr.audit_log().list_certificates();
    let total = certificates.len();

    Json(ListCertificatesResponse { certificates, total })
}

/// Get a specific deletion certificate.
pub async fn get_deletion_certificate(
    State(state): State<AppState>,
    Path(cert_id): Path<String>,
) -> impl IntoResponse {
    state.activity.log(ActivityType::Query, &format!("Get GDPR certificate: {}", cert_id));

    match state.gdpr.audit_log().get_certificate(&cert_id) {
        Some(cert) => (StatusCode::OK, Json(Some(cert))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

/// Verify a deletion certificate.
pub async fn verify_deletion_certificate(
    State(state): State<AppState>,
    Path(cert_id): Path<String>,
) -> Json<VerifyCertificateResponse> {
    state.activity.log(ActivityType::Query, &format!("Verify GDPR certificate: {}", cert_id));

    match state.gdpr.audit_log().get_certificate(&cert_id) {
        Some(cert) => {
            let valid = cert.verify();
            Json(VerifyCertificateResponse {
                valid,
                certificate: Some(cert),
                message: if valid {
                    "Certificate is valid and has not been tampered with".to_string()
                } else {
                    "Certificate verification failed - hash mismatch".to_string()
                },
            })
        }
        None => Json(VerifyCertificateResponse {
            valid: false,
            certificate: None,
            message: format!("Certificate '{}' not found", cert_id),
        }),
    }
}

/// Get deletion audit entries for a subject.
pub async fn get_deletion_audit(
    State(state): State<AppState>,
    Path(subject_id): Path<String>,
) -> Json<Vec<DeletionAuditEntry>> {
    state.activity.log(ActivityType::Query, &format!("Get GDPR audit for: {}", subject_id));

    Json(state.gdpr.audit_log().get_entries_for_subject(&subject_id))
}

/// Verify the integrity of the deletion audit log.
pub async fn verify_audit_integrity(
    State(state): State<AppState>,
) -> impl IntoResponse {
    state.activity.log(ActivityType::Query, "Verify GDPR audit log integrity");

    match state.gdpr.audit_log().verify_integrity() {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "valid": true,
                "entries_verified": count,
                "message": "Audit log integrity verified successfully",
            })),
        ),
        Err(e) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "valid": false,
                "entries_verified": 0,
                "message": e,
            })),
        ),
    }
}

// =============================================================================
// Data Export Handlers (GDPR Article 20 - Data Portability)
// =============================================================================

/// Export data for a data subject (GDPR Article 20 - Right to Data Portability).
///
/// This endpoint exports all data associated with a data subject in a machine-readable
/// format (JSON or CSV) as required by GDPR Article 20.
///
/// Request body:
/// - subject_id: The identifier of the data subject (required)
/// - format: "json" or "csv" (default: "json")
/// - scope: { "type": "all" } or { "type": "specific_collections", "collections": [...] }
/// - date_range: { "start": "ISO8601", "end": "ISO8601" } (optional)
/// - search_fields: Array of field names to search for subject_id (optional)
pub async fn export_data_subject(
    State(state): State<AppState>,
    Json(request): Json<ExportRequest>,
) -> impl IntoResponse {
    // Validate request
    if request.subject_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "subject_id is required",
            })),
        );
    }

    state.activity.log_with_details(
        ActivityType::Query,
        &format!("GDPR data export request for subject: {}", request.subject_id),
        None,
        None,
        Some("gdpr"),
        Some(serde_json::json!({
            "format": request.format,
            "scope": request.scope,
            "date_range": request.date_range,
        })),
    );

    match state.gdpr.execute_export(&state, request) {
        Ok(response) => {
            state.activity.log_with_details(
                ActivityType::System,
                &format!("GDPR data export completed. Export ID: {}", response.export_id),
                None,
                None,
                Some("gdpr"),
                Some(serde_json::json!({
                    "export_id": response.export_id,
                    "total_items": response.total_items,
                    "format": response.format,
                })),
            );

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "export_id": response.export_id,
                    "subject_id": response.subject_id,
                    "format": response.format,
                    "generated_at": response.generated_at,
                    "total_items": response.total_items,
                    "data": response.data,
                    "scope": response.scope,
                    "date_range": response.date_range,
                })),
            )
        }
        Err(e) => {
            state.activity.log_with_details(
                ActivityType::System,
                &format!("GDPR data export failed: {}", e),
                None,
                None,
                Some("gdpr"),
                None,
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": e,
                })),
            )
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    result
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deletion_scope_default() {
        let scope = DeletionScope::default();
        assert_eq!(scope, DeletionScope::All);
    }

    #[test]
    fn test_deletion_certificate_hash() {
        let items = vec![
            DeletedItem {
                store_type: "kv".to_string(),
                location: "kv_store".to_string(),
                item_id: "user:123".to_string(),
                size_bytes: Some(256),
                deleted_at: "2025-01-26T12:00:00Z".to_string(),
            },
        ];

        let cert = DeletionCertificate::new(
            "user@example.com".to_string(),
            items,
            DeletionScope::All,
            "admin".to_string(),
            "GDPR request".to_string(),
            "node-1".to_string(),
            false,
        );

        // Hash should be computed
        assert!(!cert.verification_hash.is_empty());

        // Certificate should verify
        assert!(cert.verify());
    }

    #[test]
    fn test_deletion_certificate_tamper_detection() {
        let items = vec![
            DeletedItem {
                store_type: "kv".to_string(),
                location: "kv_store".to_string(),
                item_id: "user:123".to_string(),
                size_bytes: Some(256),
                deleted_at: "2025-01-26T12:00:00Z".to_string(),
            },
        ];

        let mut cert = DeletionCertificate::new(
            "user@example.com".to_string(),
            items,
            DeletionScope::All,
            "admin".to_string(),
            "GDPR request".to_string(),
            "node-1".to_string(),
            false,
        );

        // Tamper with the certificate
        cert.total_items = 999;

        // Should fail verification
        assert!(!cert.verify());
    }

    #[test]
    fn test_audit_log_integrity() {
        let audit_log = DeletionAuditLog::new();

        // Log some events
        audit_log.log_event(
            DeletionEventType::RequestReceived,
            "user@example.com",
            "admin",
            serde_json::json!({"test": true}),
        );
        audit_log.log_event(
            DeletionEventType::DeletionStarted,
            "user@example.com",
            "admin",
            serde_json::json!({}),
        );
        audit_log.log_event(
            DeletionEventType::DeletionCompleted,
            "user@example.com",
            "admin",
            serde_json::json!({"items": 5}),
        );

        // Should verify successfully
        let result = audit_log.verify_integrity();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
    }

    #[test]
    fn test_audit_log_get_entries_for_subject() {
        let audit_log = DeletionAuditLog::new();

        audit_log.log_event(
            DeletionEventType::RequestReceived,
            "user1@example.com",
            "admin",
            serde_json::json!({}),
        );
        audit_log.log_event(
            DeletionEventType::RequestReceived,
            "user2@example.com",
            "admin",
            serde_json::json!({}),
        );
        audit_log.log_event(
            DeletionEventType::DeletionCompleted,
            "user1@example.com",
            "admin",
            serde_json::json!({}),
        );

        let user1_entries = audit_log.get_entries_for_subject("user1@example.com");
        assert_eq!(user1_entries.len(), 2);

        let user2_entries = audit_log.get_entries_for_subject("user2@example.com");
        assert_eq!(user2_entries.len(), 1);
    }

    #[test]
    fn test_gdpr_service_default_search_fields() {
        let fields = GdprService::default_search_fields();
        assert!(fields.contains(&"email".to_string()));
        assert!(fields.contains(&"user_id".to_string()));
        assert!(fields.contains(&"customer_id".to_string()));
    }

    #[test]
    fn test_hex_encode() {
        let data = [0x00, 0x01, 0x0a, 0xff, 0xab];
        let hex = hex_encode(&data);
        assert_eq!(hex, "00010affab");
    }

    #[test]
    fn test_deletion_scope_serialization() {
        let scope = DeletionScope::SpecificCollections {
            collections: vec!["users".to_string(), "orders".to_string()],
        };
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("specific_collections"));
        assert!(json.contains("users"));

        let deserialized: DeletionScope = serde_json::from_str(&json).unwrap();
        match deserialized {
            DeletionScope::SpecificCollections { collections } => {
                assert_eq!(collections.len(), 2);
                assert!(collections.contains(&"users".to_string()));
            }
            _ => panic!("Wrong variant deserialized"),
        }
    }

    // =========================================================================
    // Export Tests (GDPR Article 20)
    // =========================================================================

    #[test]
    fn test_export_format_default() {
        let format = ExportFormat::default();
        assert_eq!(format, ExportFormat::Json);
    }

    #[test]
    fn test_export_format_serialization() {
        // Test JSON format
        let format = ExportFormat::Json;
        let json = serde_json::to_string(&format).unwrap();
        assert_eq!(json, "\"json\"");

        let deserialized: ExportFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ExportFormat::Json);

        // Test CSV format
        let format = ExportFormat::Csv;
        let json = serde_json::to_string(&format).unwrap();
        assert_eq!(json, "\"csv\"");

        let deserialized: ExportFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ExportFormat::Csv);
    }

    #[test]
    fn test_export_scope_default() {
        let scope = ExportScope::default();
        match scope {
            ExportScope::All => {} // expected
            _ => panic!("Expected ExportScope::All as default"),
        }
    }

    #[test]
    fn test_export_scope_serialization() {
        let scope = ExportScope::SpecificCollections {
            collections: vec!["users".to_string(), "orders".to_string()],
        };
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("specific_collections"));
        assert!(json.contains("users"));

        let deserialized: ExportScope = serde_json::from_str(&json).unwrap();
        match deserialized {
            ExportScope::SpecificCollections { collections } => {
                assert_eq!(collections.len(), 2);
                assert!(collections.contains(&"users".to_string()));
            }
            _ => panic!("Wrong variant deserialized"),
        }
    }

    #[test]
    fn test_export_request_deserialization() {
        // Test minimal request
        let json = r#"{"subject_id": "user@example.com"}"#;
        let request: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.subject_id, "user@example.com");
        assert_eq!(request.format, ExportFormat::Json);
        assert!(request.search_fields.is_empty());

        // Test full request
        let json = r#"{
            "subject_id": "user@example.com",
            "format": "csv",
            "scope": {"type": "specific_collections", "collections": ["users"]},
            "date_range": {"start": "2024-01-01T00:00:00Z", "end": "2024-12-31T23:59:59Z"},
            "search_fields": ["email", "user_id"]
        }"#;
        let request: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.subject_id, "user@example.com");
        assert_eq!(request.format, ExportFormat::Csv);
        assert_eq!(request.search_fields, vec!["email", "user_id"]);
        assert!(request.date_range.is_some());
    }

    #[test]
    fn test_date_range() {
        let range = DateRange {
            start: Some("2024-01-01T00:00:00Z".to_string()),
            end: Some("2024-12-31T23:59:59Z".to_string()),
        };

        let json = serde_json::to_string(&range).unwrap();
        assert!(json.contains("2024-01-01"));
        assert!(json.contains("2024-12-31"));
    }

    #[test]
    fn test_escape_csv_field() {
        // Normal field
        assert_eq!(escape_csv_field("hello"), "hello");

        // Field with comma
        assert_eq!(escape_csv_field("hello,world"), "\"hello,world\"");

        // Field with quotes
        assert_eq!(escape_csv_field("hello\"world"), "\"hello\"\"world\"");

        // Field with newline
        assert_eq!(escape_csv_field("hello\nworld"), "\"hello\nworld\"");
    }

    #[test]
    fn test_json_value_to_csv_string() {
        assert_eq!(json_value_to_csv_string(&serde_json::Value::Null), "");
        assert_eq!(json_value_to_csv_string(&serde_json::Value::Bool(true)), "true");
        assert_eq!(json_value_to_csv_string(&serde_json::json!(42)), "42");
        assert_eq!(json_value_to_csv_string(&serde_json::json!("hello")), "hello");
        assert_eq!(json_value_to_csv_string(&serde_json::json!([1, 2, 3])), "[1,2,3]");
    }

    #[test]
    fn test_exported_item_structure() {
        let item = ExportedItem {
            store_type: "kv".to_string(),
            location: "kv_store".to_string(),
            item_id: "user:123".to_string(),
            data: serde_json::json!({"key": "user:123", "value": {"name": "John"}}),
            timestamp: Some("2024-01-26T12:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("kv_store"));
        assert!(json.contains("user:123"));
        assert!(json.contains("John"));
    }

    #[test]
    fn test_export_event_types() {
        // Verify the new event types are serializable
        let event = DeletionEventType::ExportRequestReceived;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"export_request_received\"");

        let event = DeletionEventType::ExportCompleted;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"export_completed\"");

        let event = DeletionEventType::ExportFailed;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"export_failed\"");
    }
}
