//! Aegis Activity Logging Module
//!
//! Tracks and stores system activity for auditing and monitoring.
//! Activities are persisted to disk in JSON-lines format with hash chain
//! integrity verification for tamper detection.
//!
//! Key Features:
//! - In-memory buffer for fast queries
//! - Append-only JSON-lines file persistence
//! - Hash chain for tamper detection
//! - Automatic log rotation at configurable size limit
//! - Recovery from persisted logs on startup
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// =============================================================================
// Constants
// =============================================================================

/// Default maximum size of a single log file before rotation (100MB).
pub const DEFAULT_MAX_LOG_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum number of rotated log files to keep.
pub const MAX_ROTATED_FILES: usize = 10;

/// Default maximum entries to keep in memory.
pub const DEFAULT_MAX_ENTRIES: usize = 1000;

/// Default retention duration for in-memory entries (24 hours).
pub const DEFAULT_RETENTION_SECS: u64 = 24 * 60 * 60;

// =============================================================================
// Activity Types
// =============================================================================

/// Type of activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityType {
    Query,
    Write,
    Delete,
    Config,
    Node,
    Auth,
    System,
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityType::Query => write!(f, "query"),
            ActivityType::Write => write!(f, "write"),
            ActivityType::Delete => write!(f, "delete"),
            ActivityType::Config => write!(f, "config"),
            ActivityType::Node => write!(f, "node"),
            ActivityType::Auth => write!(f, "auth"),
            ActivityType::System => write!(f, "system"),
        }
    }
}

/// Activity entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    pub description: String,
    pub timestamp: String,
    pub duration: Option<u64>,
    pub user: Option<String>,
    pub source: Option<String>,
    pub details: Option<serde_json::Value>,
}

/// Persisted activity record with hash chain for integrity verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedActivity {
    /// The activity data
    pub activity: Activity,
    /// SHA-256 hash of the previous record (hex encoded), or "genesis" for first record
    pub prev_hash: String,
    /// SHA-256 hash of this record (hex encoded)
    pub hash: String,
}

/// Internal activity record with instant for TTL.
#[derive(Debug, Clone)]
struct ActivityRecord {
    activity: Activity,
    created_at: Instant,
}

/// Persistence state for the activity logger.
struct PersistenceState {
    /// Path to the audit log directory
    log_dir: PathBuf,
    /// Current log file writer
    writer: Option<BufWriter<File>>,
    /// Current log file size
    current_size: u64,
    /// Maximum log file size before rotation
    max_size: u64,
    /// Hash of the last written record
    last_hash: String,
    /// Current log file number
    current_file_num: u64,
}

// =============================================================================
// Activity Logger Service
// =============================================================================

/// Activity logging service with optional disk persistence.
pub struct ActivityLogger {
    /// In-memory activity buffer for fast queries
    activities: RwLock<VecDeque<ActivityRecord>>,
    /// Next activity ID counter
    next_id: AtomicU64,
    /// Maximum entries to keep in memory
    max_entries: usize,
    /// Retention duration for in-memory entries
    retention_duration: Duration,
    /// Persistence state (None if persistence is disabled)
    persistence: RwLock<Option<PersistenceState>>,
}

impl ActivityLogger {
    /// Create a new activity logger without persistence.
    pub fn new() -> Self {
        Self {
            activities: RwLock::new(VecDeque::with_capacity(DEFAULT_MAX_ENTRIES)),
            next_id: AtomicU64::new(1),
            max_entries: DEFAULT_MAX_ENTRIES,
            retention_duration: Duration::from_secs(DEFAULT_RETENTION_SECS),
            persistence: RwLock::new(None),
        }
    }

    /// Create a new activity logger with persistence to the specified directory.
    pub fn with_persistence(log_dir: PathBuf) -> std::io::Result<Self> {
        Self::with_persistence_and_options(log_dir, DEFAULT_MAX_LOG_SIZE, DEFAULT_MAX_ENTRIES)
    }

    /// Create a new activity logger with persistence and custom options.
    pub fn with_persistence_and_options(
        log_dir: PathBuf,
        max_log_size: u64,
        max_memory_entries: usize,
    ) -> std::io::Result<Self> {
        // Ensure the log directory exists
        std::fs::create_dir_all(&log_dir)?;

        // Find existing log files and determine the current file number
        let (current_file_num, last_hash, loaded_activities) = Self::load_from_directory(&log_dir)?;

        // Determine next activity ID from loaded activities
        let next_id = loaded_activities
            .iter()
            .filter_map(|a| {
                a.id.strip_prefix("act-")
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .max()
            .unwrap_or(0)
            + 1;

        // Convert loaded activities to in-memory records
        let activities: VecDeque<ActivityRecord> = loaded_activities
            .into_iter()
            .map(|activity| ActivityRecord {
                activity,
                created_at: Instant::now(), // Use current time for TTL purposes
            })
            .collect();

        // Open or create the current log file
        let log_file_path = log_dir.join(format!("audit_{:08}.jsonl", current_file_num));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;

        let current_size = file.metadata()?.len();

        let persistence_state = PersistenceState {
            log_dir,
            writer: Some(BufWriter::new(file)),
            current_size,
            max_size: max_log_size,
            last_hash,
            current_file_num,
        };

        Ok(Self {
            activities: RwLock::new(activities),
            next_id: AtomicU64::new(next_id),
            max_entries: max_memory_entries,
            retention_duration: Duration::from_secs(DEFAULT_RETENTION_SECS),
            persistence: RwLock::new(Some(persistence_state)),
        })
    }

    /// Load activities from the log directory.
    /// Returns (current_file_num, last_hash, activities).
    fn load_from_directory(log_dir: &PathBuf) -> std::io::Result<(u64, String, Vec<Activity>)> {
        let mut log_files: Vec<(u64, PathBuf)> = Vec::new();

        // Collect all audit log files
        if let Ok(entries) = std::fs::read_dir(log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(num_str) = name
                        .strip_prefix("audit_")
                        .and_then(|s| s.strip_suffix(".jsonl"))
                    {
                        if let Ok(num) = num_str.parse::<u64>() {
                            log_files.push((num, path));
                        }
                    }
                }
            }
        }

        // Sort by file number (oldest first)
        log_files.sort_by_key(|(num, _)| *num);

        let mut activities = Vec::new();
        let mut last_hash = "genesis".to_string();
        let mut current_file_num = 0u64;
        let mut integrity_verified = true;

        // Load activities from all files, verifying hash chain
        for (file_num, path) in &log_files {
            current_file_num = *file_num;

            let file = match File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("Failed to open audit log {:?}: {}", path, e);
                    continue;
                }
            };

            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::warn!("Failed to read line from {:?}: {}", path, e);
                        continue;
                    }
                };

                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<PersistedActivity>(&line) {
                    Ok(persisted) => {
                        // Verify hash chain integrity
                        if persisted.prev_hash != last_hash {
                            tracing::error!(
                                "Hash chain integrity violation detected in {:?}: expected prev_hash '{}', got '{}'",
                                path,
                                last_hash,
                                persisted.prev_hash
                            );
                            integrity_verified = false;
                        }

                        // Verify record hash
                        let computed_hash = Self::compute_hash(&persisted.activity, &persisted.prev_hash);
                        if computed_hash != persisted.hash {
                            tracing::error!(
                                "Record hash mismatch in {:?} for activity '{}': computed '{}', stored '{}'",
                                path,
                                persisted.activity.id,
                                computed_hash,
                                persisted.hash
                            );
                            integrity_verified = false;
                        }

                        last_hash = persisted.hash;
                        activities.push(persisted.activity);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse audit record from {:?}: {}", path, e);
                    }
                }
            }
        }

        if !integrity_verified {
            tracing::warn!(
                "Audit log integrity verification FAILED. Some records may have been tampered with."
            );
        } else if !activities.is_empty() {
            tracing::info!(
                "Loaded {} audit records from {} files with verified integrity",
                activities.len(),
                log_files.len()
            );
        }

        // If no files exist, start with file 0
        if log_files.is_empty() {
            current_file_num = 0;
        }

        Ok((current_file_num, last_hash, activities))
    }

    /// Compute the SHA-256 hash of an activity record.
    fn compute_hash(activity: &Activity, prev_hash: &str) -> String {
        let mut hasher = Sha256::new();

        // Hash the activity JSON
        if let Ok(json) = serde_json::to_string(activity) {
            hasher.update(json.as_bytes());
        }

        // Include previous hash in the chain
        hasher.update(prev_hash.as_bytes());

        // Return hex-encoded hash
        let result = hasher.finalize();
        hex_encode(&result)
    }

    /// Persist an activity to disk.
    fn persist_activity(&self, activity: &Activity) {
        let mut persistence = self.persistence.write();
        let state = match persistence.as_mut() {
            Some(s) => s,
            None => return, // Persistence not enabled
        };

        // Compute hash chain
        let prev_hash = state.last_hash.clone();
        let hash = Self::compute_hash(activity, &prev_hash);

        let persisted = PersistedActivity {
            activity: activity.clone(),
            prev_hash,
            hash: hash.clone(),
        };

        // Serialize to JSON line
        let json_line = match serde_json::to_string(&persisted) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("Failed to serialize activity: {}", e);
                return;
            }
        };

        let line_size = json_line.len() as u64 + 1; // +1 for newline

        // Check if we need to rotate
        if state.current_size + line_size > state.max_size {
            if let Err(e) = self.rotate_log_file(state) {
                tracing::error!("Failed to rotate audit log: {}", e);
                return;
            }
        }

        // Write to the log file
        if let Some(ref mut writer) = state.writer {
            if let Err(e) = writeln!(writer, "{}", json_line) {
                tracing::error!("Failed to write audit record: {}", e);
                return;
            }

            // Flush to ensure durability
            if let Err(e) = writer.flush() {
                tracing::error!("Failed to flush audit log: {}", e);
                return;
            }

            state.current_size += line_size;
            state.last_hash = hash;
        }
    }

    /// Rotate to a new log file.
    fn rotate_log_file(&self, state: &mut PersistenceState) -> std::io::Result<()> {
        // Flush and close the current file
        if let Some(ref mut writer) = state.writer {
            writer.flush()?;
        }
        state.writer = None;

        // Increment file number
        state.current_file_num += 1;

        // Clean up old files if we have too many
        self.cleanup_old_files(state)?;

        // Open new file
        let new_path = state
            .log_dir
            .join(format!("audit_{:08}.jsonl", state.current_file_num));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&new_path)?;

        tracing::info!(
            "Rotated audit log to file {}",
            state.current_file_num
        );

        state.writer = Some(BufWriter::new(file));
        state.current_size = 0;

        Ok(())
    }

    /// Clean up old log files, keeping only the most recent ones.
    fn cleanup_old_files(&self, state: &PersistenceState) -> std::io::Result<()> {
        let mut log_files: Vec<(u64, PathBuf)> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&state.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(num_str) = name
                        .strip_prefix("audit_")
                        .and_then(|s| s.strip_suffix(".jsonl"))
                    {
                        if let Ok(num) = num_str.parse::<u64>() {
                            log_files.push((num, path));
                        }
                    }
                }
            }
        }

        // Sort oldest first
        log_files.sort_by_key(|(num, _)| *num);

        // Remove old files if we have more than the limit
        while log_files.len() > MAX_ROTATED_FILES {
            if let Some((num, path)) = log_files.first() {
                if *num < state.current_file_num {
                    if let Err(e) = std::fs::remove_file(path) {
                        tracing::warn!("Failed to remove old audit log {:?}: {}", path, e);
                    } else {
                        tracing::debug!("Removed old audit log file {:?}", path);
                    }
                }
            }
            log_files.remove(0);
        }

        Ok(())
    }

    /// Verify the integrity of all persisted audit logs.
    /// Returns Ok(record_count) if integrity is verified, Err with details otherwise.
    ///
    /// Note: If old log files have been cleaned up due to rotation limits,
    /// the first remaining file's first record will have a prev_hash that
    /// references the (now deleted) previous file. In this case, we accept
    /// the first record's prev_hash as valid and continue verification from there.
    pub fn verify_integrity(&self) -> Result<usize, String> {
        let persistence = self.persistence.read();
        let state = match persistence.as_ref() {
            Some(s) => s,
            None => return Err("Persistence not enabled".to_string()),
        };

        let mut log_files: Vec<(u64, PathBuf)> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&state.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(num_str) = name
                        .strip_prefix("audit_")
                        .and_then(|s| s.strip_suffix(".jsonl"))
                    {
                        if let Ok(num) = num_str.parse::<u64>() {
                            log_files.push((num, path));
                        }
                    }
                }
            }
        }

        log_files.sort_by_key(|(num, _)| *num);

        let mut last_hash: Option<String> = None;
        let mut record_count = 0usize;
        let mut is_first_record = true;

        for (_file_num, path) in &log_files {
            let file = File::open(path)
                .map_err(|e| format!("Failed to open {:?}: {}", path, e))?;
            let reader = BufReader::new(file);

            for (line_num, line) in reader.lines().enumerate() {
                let line = line.map_err(|e| {
                    format!("Failed to read line {} in {:?}: {}", line_num, path, e)
                })?;

                if line.trim().is_empty() {
                    continue;
                }

                let persisted: PersistedActivity = serde_json::from_str(&line).map_err(|e| {
                    format!(
                        "Failed to parse record at line {} in {:?}: {}",
                        line_num, path, e
                    )
                })?;

                // For the first record, accept its prev_hash as the starting point
                // (older files may have been cleaned up)
                if is_first_record {
                    is_first_record = false;
                } else if let Some(ref expected) = last_hash {
                    // Verify hash chain linkage for subsequent records
                    if &persisted.prev_hash != expected {
                        return Err(format!(
                            "Hash chain broken at line {} in {:?}: expected '{}', got '{}'",
                            line_num, path, expected, persisted.prev_hash
                        ));
                    }
                }

                // Always verify the record's own hash is computed correctly
                let computed_hash = Self::compute_hash(&persisted.activity, &persisted.prev_hash);
                if computed_hash != persisted.hash {
                    return Err(format!(
                        "Record hash mismatch at line {} in {:?}: computed '{}', stored '{}'",
                        line_num, path, computed_hash, persisted.hash
                    ));
                }

                last_hash = Some(persisted.hash);
                record_count += 1;
            }
        }

        Ok(record_count)
    }

    /// Verify integrity with strict mode - requires chain to start from "genesis".
    /// Use this for complete audit verification when all log files are present.
    pub fn verify_integrity_strict(&self) -> Result<usize, String> {
        let persistence = self.persistence.read();
        let state = match persistence.as_ref() {
            Some(s) => s,
            None => return Err("Persistence not enabled".to_string()),
        };

        let mut log_files: Vec<(u64, PathBuf)> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&state.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(num_str) = name
                        .strip_prefix("audit_")
                        .and_then(|s| s.strip_suffix(".jsonl"))
                    {
                        if let Ok(num) = num_str.parse::<u64>() {
                            log_files.push((num, path));
                        }
                    }
                }
            }
        }

        log_files.sort_by_key(|(num, _)| *num);

        // Check if the first file is file 0 (no files have been cleaned up)
        if !log_files.is_empty() && log_files[0].0 != 0 {
            return Err(format!(
                "Strict verification failed: oldest log file is {:08}, expected 00000000. \
                 Some log files have been cleaned up.",
                log_files[0].0
            ));
        }

        let mut last_hash = "genesis".to_string();
        let mut record_count = 0usize;

        for (_file_num, path) in &log_files {
            let file = File::open(path)
                .map_err(|e| format!("Failed to open {:?}: {}", path, e))?;
            let reader = BufReader::new(file);

            for (line_num, line) in reader.lines().enumerate() {
                let line = line.map_err(|e| {
                    format!("Failed to read line {} in {:?}: {}", line_num, path, e)
                })?;

                if line.trim().is_empty() {
                    continue;
                }

                let persisted: PersistedActivity = serde_json::from_str(&line).map_err(|e| {
                    format!(
                        "Failed to parse record at line {} in {:?}: {}",
                        line_num, path, e
                    )
                })?;

                // Verify hash chain
                if persisted.prev_hash != last_hash {
                    return Err(format!(
                        "Hash chain broken at line {} in {:?}: expected '{}', got '{}'",
                        line_num, path, last_hash, persisted.prev_hash
                    ));
                }

                // Verify record hash
                let computed_hash = Self::compute_hash(&persisted.activity, &persisted.prev_hash);
                if computed_hash != persisted.hash {
                    return Err(format!(
                        "Record hash mismatch at line {} in {:?}: computed '{}', stored '{}'",
                        line_num, path, computed_hash, persisted.hash
                    ));
                }

                last_hash = persisted.hash;
                record_count += 1;
            }
        }

        Ok(record_count)
    }

    /// Force flush any buffered data to disk.
    pub fn flush(&self) -> std::io::Result<()> {
        let mut persistence = self.persistence.write();
        if let Some(ref mut state) = *persistence {
            if let Some(ref mut writer) = state.writer {
                writer.flush()?;
            }
        }
        Ok(())
    }

    /// Log a new activity.
    pub fn log(&self, activity_type: ActivityType, description: &str) -> String {
        self.log_with_details(activity_type, description, None, None, None, None)
    }

    /// Log a query activity with duration.
    pub fn log_query(&self, sql: &str, duration_ms: u64, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Query,
            sql,
            Some(duration_ms),
            user,
            None,
            None,
        );
    }

    /// Log a write activity.
    pub fn log_write(&self, description: &str, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Write,
            description,
            None,
            user,
            None,
            None,
        );
    }

    /// Log a configuration change.
    pub fn log_config(&self, description: &str, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Config,
            description,
            None,
            user,
            None,
            None,
        );
    }

    /// Log a node event.
    pub fn log_node(&self, description: &str) {
        self.log_with_details(
            ActivityType::Node,
            description,
            None,
            None,
            Some("cluster"),
            None,
        );
    }

    /// Log an authentication event.
    pub fn log_auth(&self, description: &str, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Auth,
            description,
            None,
            user,
            None,
            None,
        );
    }

    /// Log a system event.
    pub fn log_system(&self, description: &str) {
        self.log_with_details(
            ActivityType::System,
            description,
            None,
            None,
            Some("system"),
            None,
        );
    }

    /// Log an activity with full details.
    pub fn log_with_details(
        &self,
        activity_type: ActivityType,
        description: &str,
        duration: Option<u64>,
        user: Option<&str>,
        source: Option<&str>,
        details: Option<serde_json::Value>,
    ) -> String {
        let id = format!("act-{:08}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let activity = Activity {
            id: id.clone(),
            activity_type,
            description: description.to_string(),
            timestamp: format_timestamp(now_timestamp()),
            duration,
            user: user.map(|s| s.to_string()),
            source: source.map(|s| s.to_string()),
            details,
        };

        // Persist to disk first (for durability)
        self.persist_activity(&activity);

        let record = ActivityRecord {
            activity,
            created_at: Instant::now(),
        };

        let mut activities = self.activities.write();

        // Remove old entries if at capacity
        while activities.len() >= self.max_entries {
            activities.pop_front();
        }

        activities.push_back(record);
        id
    }

    /// Get recent activities.
    pub fn get_recent(&self, limit: usize) -> Vec<Activity> {
        self.cleanup_expired();

        let activities = self.activities.read();
        activities
            .iter()
            .rev()
            .take(limit)
            .map(|r| r.activity.clone())
            .collect()
    }

    /// Get activities by type.
    pub fn get_by_type(&self, activity_type: ActivityType, limit: usize) -> Vec<Activity> {
        self.cleanup_expired();

        let activities = self.activities.read();
        activities
            .iter()
            .rev()
            .filter(|r| r.activity.activity_type == activity_type)
            .take(limit)
            .map(|r| r.activity.clone())
            .collect()
    }

    /// Get activities by user.
    pub fn get_by_user(&self, username: &str, limit: usize) -> Vec<Activity> {
        self.cleanup_expired();

        let activities = self.activities.read();
        activities
            .iter()
            .rev()
            .filter(|r| r.activity.user.as_deref() == Some(username))
            .take(limit)
            .map(|r| r.activity.clone())
            .collect()
    }

    /// Get activity count.
    pub fn count(&self) -> usize {
        self.activities.read().len()
    }

    /// Clean up expired activities.
    fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut activities = self.activities.write();

        while let Some(front) = activities.front() {
            if now.duration_since(front.created_at) > self.retention_duration {
                activities.pop_front();
            } else {
                break;
            }
        }
    }

    /// Clear all activities.
    pub fn clear(&self) {
        self.activities.write().clear();
    }
}

impl Default for ActivityLogger {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get current timestamp in milliseconds.
fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Format a timestamp to RFC3339 string.
fn format_timestamp(timestamp_ms: u64) -> String {
    let secs = timestamp_ms / 1000;
    let datetime = UNIX_EPOCH + Duration::from_secs(secs);
    let duration = datetime.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = duration.as_secs();

    let days_since_epoch = total_secs / 86400;
    let secs_today = total_secs % 86400;

    let hours = secs_today / 3600;
    let minutes = (secs_today % 3600) / 60;
    let seconds = secs_today % 60;

    let mut year = 1970u64;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u64;
    for &days in &days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

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
    use tempfile::TempDir;

    #[test]
    fn test_log_activity() {
        let logger = ActivityLogger::new();
        let id = logger.log(ActivityType::Query, "SELECT * FROM users");
        assert!(!id.is_empty());
        assert_eq!(logger.count(), 1);
    }

    #[test]
    fn test_get_recent() {
        let logger = ActivityLogger::new();
        logger.log(ActivityType::Query, "Query 1");
        logger.log(ActivityType::Write, "Write 1");
        logger.log(ActivityType::Query, "Query 2");

        let recent = logger.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].description, "Query 2");
        assert_eq!(recent[1].description, "Write 1");
    }

    #[test]
    fn test_get_by_type() {
        let logger = ActivityLogger::new();
        logger.log(ActivityType::Query, "Query 1");
        logger.log(ActivityType::Write, "Write 1");
        logger.log(ActivityType::Query, "Query 2");

        let queries = logger.get_by_type(ActivityType::Query, 10);
        assert_eq!(queries.len(), 2);
    }

    #[test]
    fn test_log_query_with_duration() {
        let logger = ActivityLogger::new();
        logger.log_query("SELECT * FROM metrics", 42, Some("admin"));

        let recent = logger.get_recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].duration, Some(42));
        assert_eq!(recent[0].user, Some("admin".to_string()));
    }

    #[test]
    fn test_max_entries() {
        let logger = ActivityLogger::new();

        // Log more than max entries
        for i in 0..1100 {
            logger.log(ActivityType::Query, &format!("Query {}", i));
        }

        // Should be capped at max
        assert!(logger.count() <= 1000);
    }

    #[test]
    fn test_persistence_basic() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let log_dir = temp_dir.path().to_path_buf();

        // Create logger and log some activities
        {
            let logger = ActivityLogger::with_persistence(log_dir.clone())
                .expect("failed to create logger");
            logger.log(ActivityType::Query, "SELECT * FROM users");
            logger.log(ActivityType::Write, "INSERT INTO users");
            logger.log(ActivityType::Auth, "User login");
            logger.flush().expect("failed to flush");
        }

        // Verify the log file was created
        let log_file = log_dir.join("audit_00000000.jsonl");
        assert!(log_file.exists(), "audit log file should exist");

        // Create a new logger and verify it loads the activities
        let logger2 = ActivityLogger::with_persistence(log_dir)
            .expect("failed to create second logger");

        let recent = logger2.get_recent(10);
        assert_eq!(recent.len(), 3, "should load 3 activities from disk");

        // Verify integrity
        let count = logger2.verify_integrity().expect("integrity check should pass");
        assert_eq!(count, 3, "should have 3 verified records");
    }

    #[test]
    fn test_persistence_hash_chain() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let log_dir = temp_dir.path().to_path_buf();

        let logger = ActivityLogger::with_persistence(log_dir.clone())
            .expect("failed to create logger");

        // Log multiple activities
        for i in 0..5 {
            logger.log(ActivityType::Query, &format!("Query {}", i));
        }
        logger.flush().expect("failed to flush");

        // Read the log file and verify hash chain
        let log_file = log_dir.join("audit_00000000.jsonl");
        let content = std::fs::read_to_string(&log_file).expect("failed to read log file");

        let mut last_hash = "genesis".to_string();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let persisted: PersistedActivity =
                serde_json::from_str(line).expect("failed to parse record");

            // Verify chain linkage
            assert_eq!(
                persisted.prev_hash, last_hash,
                "prev_hash should match last record's hash"
            );

            // Verify hash computation
            let computed = ActivityLogger::compute_hash(&persisted.activity, &persisted.prev_hash);
            assert_eq!(
                persisted.hash, computed,
                "stored hash should match computed hash"
            );

            last_hash = persisted.hash;
        }
    }

    #[test]
    fn test_persistence_recovery_continues_ids() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let log_dir = temp_dir.path().to_path_buf();

        // Create logger and log some activities
        {
            let logger = ActivityLogger::with_persistence(log_dir.clone())
                .expect("failed to create logger");
            logger.log(ActivityType::Query, "Query 1");
            logger.log(ActivityType::Query, "Query 2");
            logger.flush().expect("failed to flush");
        }

        // Create a new logger and log more activities
        let logger2 = ActivityLogger::with_persistence(log_dir)
            .expect("failed to create second logger");
        let id = logger2.log(ActivityType::Query, "Query 3");

        // The new ID should continue from where we left off
        assert!(
            id.contains("00000003"),
            "ID should continue sequence: got {}",
            id
        );
    }

    #[test]
    fn test_persistence_log_rotation() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let log_dir = temp_dir.path().to_path_buf();

        // Create logger with very small max size to force rotation
        let logger = ActivityLogger::with_persistence_and_options(
            log_dir.clone(),
            500, // 500 bytes max
            100,
        )
        .expect("failed to create logger");

        // Log enough activities to trigger rotation
        for i in 0..20 {
            logger.log(ActivityType::Query, &format!("This is query number {}", i));
        }
        logger.flush().expect("failed to flush");

        // Count log files
        let log_files: Vec<_> = std::fs::read_dir(&log_dir)
            .expect("failed to read dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
            })
            .collect();

        assert!(
            log_files.len() > 1,
            "should have multiple log files after rotation, got {}",
            log_files.len()
        );

        // Verify integrity across all remaining files
        // Note: some old files may have been cleaned up, so we use non-strict verification
        let count = logger.verify_integrity().expect("integrity check should pass");
        // Count may be less than 20 if old files were cleaned up
        assert!(count > 0, "should have some verified records");
    }

    #[test]
    fn test_persistence_log_rotation_strict() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let log_dir = temp_dir.path().to_path_buf();

        // Create logger with small max size but we'll log fewer entries
        // so that cleanup doesn't remove files
        let logger = ActivityLogger::with_persistence_and_options(
            log_dir.clone(),
            300, // 300 bytes max
            100,
        )
        .expect("failed to create logger");

        // Log just a few activities to get multiple files without hitting cleanup
        for i in 0..5 {
            logger.log(ActivityType::Query, &format!("Query {}", i));
        }
        logger.flush().expect("failed to flush");

        // Count log files
        let log_files: Vec<_> = std::fs::read_dir(&log_dir)
            .expect("failed to read dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
            })
            .collect();

        // If we have all files starting from 0, strict verification should pass
        if log_files.iter().any(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "audit_00000000.jsonl")
        }) {
            let count = logger
                .verify_integrity_strict()
                .expect("strict integrity check should pass");
            assert_eq!(count, 5, "should have 5 verified records");
        }
    }

    #[test]
    fn test_hex_encode() {
        let data = [0x00, 0x01, 0x0a, 0xff, 0xab];
        let hex = hex_encode(&data);
        assert_eq!(hex, "00010affab");
    }

    #[test]
    fn test_hash_computation() {
        let activity = Activity {
            id: "act-00000001".to_string(),
            activity_type: ActivityType::Query,
            description: "SELECT * FROM users".to_string(),
            timestamp: "2025-01-26T12:00:00Z".to_string(),
            duration: None,
            user: Some("admin".to_string()),
            source: None,
            details: None,
        };

        let hash1 = ActivityLogger::compute_hash(&activity, "genesis");
        let hash2 = ActivityLogger::compute_hash(&activity, "genesis");

        // Same input should produce same hash
        assert_eq!(hash1, hash2);

        // Different prev_hash should produce different hash
        let hash3 = ActivityLogger::compute_hash(&activity, "different");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_integrity_verification_fails_on_tamper() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let log_dir = temp_dir.path().to_path_buf();

        // Create and populate the log
        {
            let logger = ActivityLogger::with_persistence(log_dir.clone())
                .expect("failed to create logger");
            logger.log(ActivityType::Query, "Query 1");
            logger.log(ActivityType::Query, "Query 2");
            logger.log(ActivityType::Query, "Query 3");
            logger.flush().expect("failed to flush");
        }

        // Tamper with the log file
        let log_file = log_dir.join("audit_00000000.jsonl");
        let content = std::fs::read_to_string(&log_file).expect("failed to read");
        let tampered = content.replace("Query 2", "TAMPERED");
        std::fs::write(&log_file, tampered).expect("failed to write");

        // Verify integrity should fail
        let logger2 = ActivityLogger::with_persistence(log_dir)
            .expect("failed to create logger");
        let result = logger2.verify_integrity();
        assert!(result.is_err(), "integrity check should fail after tampering");
    }
}
