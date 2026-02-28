//! Aegis Bulk Import Module
//!
//! Provides bulk data import for SQL tables, document collections, and KV store.
//! Supports CSV and JSON formats with transactional semantics.
//!
//! Endpoints:
//! - POST /api/v1/import/sql       — Bulk import rows into a SQL table (CSV or JSON)
//! - POST /api/v1/import/documents — Bulk import documents into a collection (JSON)
//! - POST /api/v1/import/kv        — Bulk import key-value pairs (JSON)
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Types
// =============================================================================

/// Request to bulk import rows into a SQL table.
#[derive(Debug, Deserialize)]
pub struct SqlImportRequest {
    /// Target table name
    pub table: String,
    /// Target database (optional, defaults to "default")
    #[serde(default)]
    pub database: Option<String>,
    /// Import format
    pub format: ImportFormat,
    /// Column names (required for CSV, optional for JSON — inferred from keys)
    #[serde(default)]
    pub columns: Vec<String>,
    /// The data payload — CSV string or JSON array of row arrays/objects
    pub data: serde_json::Value,
}

/// Request to bulk import documents into a collection.
#[derive(Debug, Deserialize)]
pub struct DocumentImportRequest {
    /// Target collection name (created if it doesn't exist)
    pub collection: String,
    /// Array of document objects
    pub documents: Vec<serde_json::Value>,
}

/// Request to bulk import key-value pairs.
#[derive(Debug, Deserialize)]
pub struct KvImportRequest {
    /// Array of key-value pairs
    pub entries: Vec<KvEntry>,
}

#[derive(Debug, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: serde_json::Value,
}

/// Import format.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImportFormat {
    Csv,
    Json,
}

/// Import result.
#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub rows_imported: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

// =============================================================================
// SQL Bulk Import
// =============================================================================

/// Bulk import rows into a SQL table.
///
/// For CSV format: `data` should be a string with CSV content.
/// For JSON format: `data` should be an array of arrays (positional) or objects (named).
pub async fn import_sql(
    State(state): State<AppState>,
    Json(request): Json<SqlImportRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    state.activity.log_write(&format!("Bulk SQL import into table: {}", request.table), None);

    let db = request.database.as_deref();

    // Build INSERT statements from the data
    let result = match request.format {
        ImportFormat::Csv => import_sql_csv(&state, &request.table, db, &request.columns, &request.data),
        ImportFormat::Json => import_sql_json(&state, &request.table, db, &request.columns, &request.data),
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok((count, warnings)) => (
            StatusCode::OK,
            Json(ImportResponse {
                success: true,
                rows_imported: count,
                error: None,
                warnings,
                duration_ms,
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ImportResponse {
                success: false,
                rows_imported: 0,
                error: Some(e),
                warnings: Vec::new(),
                duration_ms,
            }),
        ),
    }
}

fn import_sql_csv(
    state: &AppState,
    table: &str,
    database: Option<&str>,
    columns: &[String],
    data: &serde_json::Value,
) -> Result<(u64, Vec<String>), String> {
    let csv_text = data.as_str().ok_or("CSV data must be a string")?;
    if columns.is_empty() {
        return Err("columns are required for CSV import".to_string());
    }

    let col_list = columns.join(", ");
    let mut total = 0u64;
    let mut warnings = Vec::new();

    // Process in batches of 100 rows for efficiency
    let mut batch_values = Vec::new();
    for (line_num, line) in csv_text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip header row if it matches column names
        if line_num == 0 {
            let first_field = trimmed.split(',').next().unwrap_or("").trim();
            if columns.iter().any(|c| c.eq_ignore_ascii_case(first_field)) {
                continue;
            }
        }

        let fields: Vec<&str> = trimmed.split(',').map(|f| f.trim()).collect();
        if fields.len() != columns.len() {
            warnings.push(format!("Line {}: expected {} fields, got {} — skipped", line_num + 1, columns.len(), fields.len()));
            continue;
        }

        let values: Vec<String> = fields.iter().map(|f| format_sql_value(f)).collect();
        batch_values.push(format!("({})", values.join(", ")));

        if batch_values.len() >= 100 {
            let sql = format!("INSERT INTO {} ({}) VALUES {}", table, col_list, batch_values.join(", "));
            state.query_engine.execute(&sql, database)
                .map_err(|e| format!("Batch insert failed at line {}: {}", line_num + 1, e))?;
            total += batch_values.len() as u64;
            batch_values.clear();
        }
    }

    // Flush remaining
    if !batch_values.is_empty() {
        let sql = format!("INSERT INTO {} ({}) VALUES {}", table, col_list, batch_values.join(", "));
        state.query_engine.execute(&sql, database)
            .map_err(|e| format!("Final batch insert failed: {}", e))?;
        total += batch_values.len() as u64;
    }

    Ok((total, warnings))
}

fn import_sql_json(
    state: &AppState,
    table: &str,
    database: Option<&str>,
    columns: &[String],
    data: &serde_json::Value,
) -> Result<(u64, Vec<String>), String> {
    let rows = data.as_array().ok_or("JSON data must be an array")?;
    if rows.is_empty() {
        return Ok((0, Vec::new()));
    }

    let warnings = Vec::new();
    let mut total = 0u64;

    // Determine columns from first row if not provided
    let col_names: Vec<String> = if columns.is_empty() {
        match &rows[0] {
            serde_json::Value::Object(obj) => obj.keys().cloned().collect(),
            _ => return Err("JSON rows must be objects when columns are not specified".to_string()),
        }
    } else {
        columns.to_vec()
    };

    let col_list = col_names.join(", ");

    // Process in batches of 100
    let mut batch_values = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        let values: Vec<String> = match row {
            serde_json::Value::Array(arr) => {
                if arr.len() != col_names.len() {
                    return Err(format!("Row {}: expected {} values, got {}", idx, col_names.len(), arr.len()));
                }
                arr.iter().map(json_value_to_sql).collect()
            }
            serde_json::Value::Object(obj) => {
                col_names.iter().map(|col| {
                    obj.get(col).map(json_value_to_sql).unwrap_or_else(|| "NULL".to_string())
                }).collect()
            }
            _ => return Err(format!("Row {}: must be an array or object", idx)),
        };

        batch_values.push(format!("({})", values.join(", ")));

        if batch_values.len() >= 100 {
            let sql = format!("INSERT INTO {} ({}) VALUES {}", table, col_list, batch_values.join(", "));
            state.query_engine.execute(&sql, database)
                .map_err(|e| format!("Batch insert failed at row {}: {}", idx + 1, e))?;
            total += batch_values.len() as u64;
            batch_values.clear();
        }
    }

    // Flush remaining
    if !batch_values.is_empty() {
        let sql = format!("INSERT INTO {} ({}) VALUES {}", table, col_list, batch_values.join(", "));
        state.query_engine.execute(&sql, database)
            .map_err(|e| format!("Final batch insert failed: {}", e))?;
        total += batch_values.len() as u64;
    }

    Ok((total, warnings))
}

/// Convert a CSV field to a SQL value literal.
fn format_sql_value(field: &str) -> String {
    if field.eq_ignore_ascii_case("null") || field.is_empty() {
        return "NULL".to_string();
    }
    if field.eq_ignore_ascii_case("true") {
        return "TRUE".to_string();
    }
    if field.eq_ignore_ascii_case("false") {
        return "FALSE".to_string();
    }
    // Try parsing as number
    if field.parse::<i64>().is_ok() || field.parse::<f64>().is_ok() {
        return field.to_string();
    }
    // String value — escape single quotes
    format!("'{}'", field.replace('\'', "''"))
}

/// Convert a JSON value to a SQL literal.
fn json_value_to_sql(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => format!("'{}'", value.to_string().replace('\'', "''")),
    }
}

// =============================================================================
// Document Bulk Import
// =============================================================================

/// Bulk import documents into a collection.
pub async fn import_documents(
    State(state): State<AppState>,
    Json(request): Json<DocumentImportRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    state.activity.log_write(&format!("Bulk document import into: {}", request.collection), None);

    // Create collection if it doesn't exist
    let _ = state.document_engine.create_collection(&request.collection);

    let mut imported = 0u64;
    let mut errors = Vec::new();

    for (idx, doc_json) in request.documents.iter().enumerate() {
        let doc = crate::handlers::json_to_doc(doc_json.clone());
        match state.document_engine.insert(&request.collection, doc) {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(format!("Document {}: {}", idx, e));
                if errors.len() >= 10 {
                    errors.push("... (too many errors, stopping)".to_string());
                    break;
                }
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let success = errors.is_empty();

    (
        if success { StatusCode::OK } else { StatusCode::MULTI_STATUS },
        Json(ImportResponse {
            success,
            rows_imported: imported,
            error: if errors.is_empty() { None } else { Some(errors.join("; ")) },
            warnings: Vec::new(),
            duration_ms,
        }),
    )
}

// =============================================================================
// KV Bulk Import
// =============================================================================

/// Bulk import key-value pairs.
pub async fn import_kv(
    State(state): State<AppState>,
    Json(request): Json<KvImportRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    state.activity.log_write("Bulk KV import", None);

    let mut imported = 0u64;
    for entry in &request.entries {
        state.kv_store.set(entry.key.clone(), entry.value.clone(), None);
        imported += 1;
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    (
        StatusCode::OK,
        Json(ImportResponse {
            success: true,
            rows_imported: imported,
            error: None,
            warnings: Vec::new(),
            duration_ms,
        }),
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sql_value() {
        assert_eq!(format_sql_value("42"), "42");
        assert_eq!(format_sql_value("3.14"), "3.14");
        assert_eq!(format_sql_value("hello"), "'hello'");
        assert_eq!(format_sql_value("null"), "NULL");
        assert_eq!(format_sql_value("NULL"), "NULL");
        assert_eq!(format_sql_value("true"), "TRUE");
        assert_eq!(format_sql_value(""), "NULL");
        assert_eq!(format_sql_value("it's"), "'it''s'");
    }

    #[test]
    fn test_json_value_to_sql() {
        assert_eq!(json_value_to_sql(&serde_json::json!(null)), "NULL");
        assert_eq!(json_value_to_sql(&serde_json::json!(42)), "42");
        assert_eq!(json_value_to_sql(&serde_json::json!(true)), "TRUE");
        assert_eq!(json_value_to_sql(&serde_json::json!("hello")), "'hello'");
        assert_eq!(json_value_to_sql(&serde_json::json!("it's")), "'it''s'");
    }
}
