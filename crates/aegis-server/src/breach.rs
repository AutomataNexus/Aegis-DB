//! Aegis Breach Detection and Notification Module
//!
//! HIPAA/GDPR compliance module for detecting security breaches and sending notifications.
//! Monitors security events, detects anomalies, and maintains breach incident records.
//!
//! Key Features:
//! - Security event monitoring (failed logins, unauthorized access, etc.)
//! - Configurable detection thresholds
//! - Breach incident tracking with severity levels
//! - Pluggable notification system (webhooks, log files)
//! - Compliance reporting for HIPAA/GDPR requirements
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// =============================================================================
// Constants
// =============================================================================

/// Default threshold for failed login attempts (5 attempts in 5 minutes).
pub const DEFAULT_FAILED_LOGIN_THRESHOLD: u32 = 5;

/// Default time window for failed login detection (5 minutes).
pub const DEFAULT_FAILED_LOGIN_WINDOW_SECS: u64 = 300;

/// Default threshold for unusual access patterns (100 accesses in 1 minute).
pub const DEFAULT_UNUSUAL_ACCESS_THRESHOLD: u32 = 100;

/// Default time window for unusual access detection (1 minute).
pub const DEFAULT_UNUSUAL_ACCESS_WINDOW_SECS: u64 = 60;

/// Default threshold for mass data operations (1000 rows).
pub const DEFAULT_MASS_DATA_THRESHOLD: u64 = 1000;

/// Maximum security events to keep in memory per type.
pub const MAX_EVENTS_IN_MEMORY: usize = 10000;

/// Maximum breach incidents to keep in memory.
pub const MAX_INCIDENTS_IN_MEMORY: usize = 1000;

// =============================================================================
// Security Event Types
// =============================================================================

/// Types of security events that the breach detector monitors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityEventType {
    /// Multiple failed login attempts.
    FailedLogin,
    /// Unauthorized access attempt (permission denied).
    UnauthorizedAccess,
    /// Unusual data access pattern (high volume).
    UnusualAccessPattern,
    /// Admin action from unknown/untrusted IP.
    AdminFromUnknownIp,
    /// Mass data export operation.
    MassDataExport,
    /// Mass data deletion operation.
    MassDataDeletion,
    /// Session hijacking attempt.
    SessionHijacking,
    /// SQL injection attempt.
    SqlInjection,
    /// Brute force attack detected.
    BruteForceAttack,
    /// Privilege escalation attempt.
    PrivilegeEscalation,
}

impl std::fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityEventType::FailedLogin => write!(f, "failed_login"),
            SecurityEventType::UnauthorizedAccess => write!(f, "unauthorized_access"),
            SecurityEventType::UnusualAccessPattern => write!(f, "unusual_access_pattern"),
            SecurityEventType::AdminFromUnknownIp => write!(f, "admin_from_unknown_ip"),
            SecurityEventType::MassDataExport => write!(f, "mass_data_export"),
            SecurityEventType::MassDataDeletion => write!(f, "mass_data_deletion"),
            SecurityEventType::SessionHijacking => write!(f, "session_hijacking"),
            SecurityEventType::SqlInjection => write!(f, "sql_injection"),
            SecurityEventType::BruteForceAttack => write!(f, "brute_force_attack"),
            SecurityEventType::PrivilegeEscalation => write!(f, "privilege_escalation"),
        }
    }
}

/// A security event recorded by the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    /// Unique event ID.
    pub id: String,
    /// Type of security event.
    pub event_type: SecurityEventType,
    /// Timestamp when the event occurred (Unix milliseconds).
    pub timestamp: u64,
    /// User associated with the event (if known).
    pub user: Option<String>,
    /// IP address associated with the event (if known).
    pub ip_address: Option<String>,
    /// Resource being accessed (table, endpoint, etc.).
    pub resource: Option<String>,
    /// Detailed description of the event.
    pub description: String,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl SecurityEvent {
    /// Create a new security event.
    pub fn new(event_type: SecurityEventType, description: &str) -> Self {
        Self {
            id: generate_event_id(),
            event_type,
            timestamp: now_timestamp(),
            user: None,
            ip_address: None,
            resource: None,
            description: description.to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Set the user associated with this event.
    pub fn with_user(mut self, user: &str) -> Self {
        self.user = Some(user.to_string());
        self
    }

    /// Set the IP address associated with this event.
    pub fn with_ip(mut self, ip: &str) -> Self {
        self.ip_address = Some(ip.to_string());
        self
    }

    /// Set the resource being accessed.
    pub fn with_resource(mut self, resource: &str) -> Self {
        self.resource = Some(resource.to_string());
        self
    }

    /// Add metadata to the event.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

// =============================================================================
// Breach Severity
// =============================================================================

/// Severity level for breach incidents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BreachSeverity {
    /// Low severity - minor security concern.
    Low,
    /// Medium severity - potential security issue requiring attention.
    Medium,
    /// High severity - significant security breach.
    High,
    /// Critical severity - immediate action required.
    Critical,
}

impl std::fmt::Display for BreachSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BreachSeverity::Low => write!(f, "low"),
            BreachSeverity::Medium => write!(f, "medium"),
            BreachSeverity::High => write!(f, "high"),
            BreachSeverity::Critical => write!(f, "critical"),
        }
    }
}

// =============================================================================
// Breach Incident
// =============================================================================

/// Status of a breach incident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    /// Incident is open and being investigated.
    Open,
    /// Incident has been acknowledged by an administrator.
    Acknowledged,
    /// Incident is being actively investigated.
    Investigating,
    /// Incident has been resolved.
    Resolved,
    /// Incident was closed as false positive.
    FalsePositive,
}

impl std::fmt::Display for IncidentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IncidentStatus::Open => write!(f, "open"),
            IncidentStatus::Acknowledged => write!(f, "acknowledged"),
            IncidentStatus::Investigating => write!(f, "investigating"),
            IncidentStatus::Resolved => write!(f, "resolved"),
            IncidentStatus::FalsePositive => write!(f, "false_positive"),
        }
    }
}

/// A breach incident detected by the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreachIncident {
    /// Unique incident ID.
    pub id: String,
    /// Timestamp when the incident was detected (Unix milliseconds).
    pub detected_at: u64,
    /// Type of security event that triggered this incident.
    pub incident_type: SecurityEventType,
    /// Severity of the incident.
    pub severity: BreachSeverity,
    /// Users or subjects potentially affected.
    pub affected_subjects: Vec<String>,
    /// Current status of the incident.
    pub status: IncidentStatus,
    /// Whether notifications have been sent.
    pub notified: bool,
    /// Notification timestamps by notifier type.
    pub notification_timestamps: HashMap<String, u64>,
    /// Description of the incident.
    pub description: String,
    /// Related security event IDs.
    pub related_events: Vec<String>,
    /// IP addresses involved.
    pub involved_ips: Vec<String>,
    /// Additional details.
    pub details: HashMap<String, String>,
    /// Who acknowledged the incident (if applicable).
    pub acknowledged_by: Option<String>,
    /// When the incident was acknowledged.
    pub acknowledged_at: Option<u64>,
    /// Resolution notes.
    pub resolution_notes: Option<String>,
    /// When the incident was resolved.
    pub resolved_at: Option<u64>,
}

impl BreachIncident {
    /// Create a new breach incident.
    pub fn new(
        incident_type: SecurityEventType,
        severity: BreachSeverity,
        description: &str,
    ) -> Self {
        Self {
            id: generate_incident_id(),
            detected_at: now_timestamp(),
            incident_type,
            severity,
            affected_subjects: Vec::new(),
            status: IncidentStatus::Open,
            notified: false,
            notification_timestamps: HashMap::new(),
            description: description.to_string(),
            related_events: Vec::new(),
            involved_ips: Vec::new(),
            details: HashMap::new(),
            acknowledged_by: None,
            acknowledged_at: None,
            resolution_notes: None,
            resolved_at: None,
        }
    }

    /// Add an affected subject.
    pub fn with_affected_subject(mut self, subject: &str) -> Self {
        if !self.affected_subjects.contains(&subject.to_string()) {
            self.affected_subjects.push(subject.to_string());
        }
        self
    }

    /// Add a related event.
    pub fn with_related_event(mut self, event_id: &str) -> Self {
        self.related_events.push(event_id.to_string());
        self
    }

    /// Add an involved IP.
    pub fn with_involved_ip(mut self, ip: &str) -> Self {
        if !self.involved_ips.contains(&ip.to_string()) {
            self.involved_ips.push(ip.to_string());
        }
        self
    }

    /// Add a detail.
    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }

    /// Check if the incident requires immediate notification (HIPAA/GDPR).
    pub fn requires_immediate_notification(&self) -> bool {
        self.severity >= BreachSeverity::High
    }
}

// =============================================================================
// Detection Configuration
// =============================================================================

/// Configuration for breach detection thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    /// Number of failed logins before triggering an incident.
    pub failed_login_threshold: u32,
    /// Time window for failed login detection (seconds).
    pub failed_login_window_secs: u64,
    /// Number of access operations before flagging unusual pattern.
    pub unusual_access_threshold: u32,
    /// Time window for unusual access detection (seconds).
    pub unusual_access_window_secs: u64,
    /// Number of rows before flagging as mass data operation.
    pub mass_data_threshold: u64,
    /// List of trusted IP addresses for admin actions.
    pub trusted_admin_ips: Vec<String>,
    /// Whether to enable brute force detection.
    pub enable_brute_force_detection: bool,
    /// Whether to enable SQL injection detection.
    pub enable_sql_injection_detection: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            failed_login_threshold: DEFAULT_FAILED_LOGIN_THRESHOLD,
            failed_login_window_secs: DEFAULT_FAILED_LOGIN_WINDOW_SECS,
            unusual_access_threshold: DEFAULT_UNUSUAL_ACCESS_THRESHOLD,
            unusual_access_window_secs: DEFAULT_UNUSUAL_ACCESS_WINDOW_SECS,
            mass_data_threshold: DEFAULT_MASS_DATA_THRESHOLD,
            trusted_admin_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
            enable_brute_force_detection: true,
            enable_sql_injection_detection: true,
        }
    }
}

// =============================================================================
// Notifier Trait
// =============================================================================

/// Trait for pluggable breach notification handlers.
pub trait BreachNotifier: Send + Sync {
    /// Get the notifier's name/identifier.
    fn name(&self) -> &str;

    /// Send a notification for a breach incident.
    fn notify(&self, incident: &BreachIncident) -> Result<(), String>;
}

// =============================================================================
// Webhook Notifier
// =============================================================================

/// Webhook notifier that POSTs breach incidents to a configured URL.
pub struct WebhookNotifier {
    name: String,
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::blocking::Client,
}

impl WebhookNotifier {
    /// Create a new webhook notifier.
    pub fn new(url: &str) -> Self {
        Self {
            name: "webhook".to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }

    /// Set a custom name for this notifier.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Add a custom header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }
}

impl BreachNotifier for WebhookNotifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn notify(&self, incident: &BreachIncident) -> Result<(), String> {
        // Build the webhook payload
        let payload = serde_json::json!({
            "incident_id": incident.id,
            "detected_at": format_timestamp(incident.detected_at),
            "type": incident.incident_type.to_string(),
            "severity": incident.severity.to_string(),
            "description": incident.description,
            "affected_subjects": incident.affected_subjects,
            "involved_ips": incident.involved_ips,
            "status": incident.status.to_string(),
            "details": incident.details,
            "source": "aegis-db",
        });

        let mut request = self.client.post(&self.url);

        // Add custom headers
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        let response = request
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .map_err(|e| format!("Failed to send webhook: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "Webhook returned error status: {}",
                response.status()
            ))
        }
    }
}

// =============================================================================
// Log Notifier
// =============================================================================

/// Log notifier that writes breach incidents to a log file.
pub struct LogNotifier {
    name: String,
    log_path: PathBuf,
    writer: RwLock<Option<BufWriter<File>>>,
}

impl LogNotifier {
    /// Create a new log notifier.
    pub fn new(log_path: PathBuf) -> std::io::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        Ok(Self {
            name: "log".to_string(),
            log_path,
            writer: RwLock::new(Some(BufWriter::new(file))),
        })
    }

    /// Set a custom name for this notifier.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
}

impl BreachNotifier for LogNotifier {
    fn name(&self) -> &str {
        &self.name
    }

    fn notify(&self, incident: &BreachIncident) -> Result<(), String> {
        let log_entry = serde_json::json!({
            "timestamp": format_timestamp(now_timestamp()),
            "incident_id": incident.id,
            "detected_at": format_timestamp(incident.detected_at),
            "type": incident.incident_type.to_string(),
            "severity": incident.severity.to_string(),
            "description": incident.description,
            "affected_subjects": incident.affected_subjects,
            "involved_ips": incident.involved_ips,
            "status": incident.status.to_string(),
            "details": incident.details,
        });

        let mut writer = self.writer.write();
        if let Some(ref mut w) = *writer {
            writeln!(w, "{}", log_entry)
                .map_err(|e| format!("Failed to write to breach log: {}", e))?;
            w.flush()
                .map_err(|e| format!("Failed to flush breach log: {}", e))?;
            Ok(())
        } else {
            Err("Log writer not initialized".to_string())
        }
    }
}

// =============================================================================
// Breach Detector
// =============================================================================

/// Internal record for tracking events with timestamps.
struct EventRecord {
    event: SecurityEvent,
    received_at: Instant,
}

/// Main breach detection service.
pub struct BreachDetector {
    /// Detection configuration.
    config: RwLock<DetectionConfig>,
    /// Security events by type.
    events: RwLock<HashMap<SecurityEventType, VecDeque<EventRecord>>>,
    /// Failed login attempts by user/IP.
    failed_logins: RwLock<HashMap<String, VecDeque<Instant>>>,
    /// Access patterns by user.
    access_patterns: RwLock<HashMap<String, VecDeque<Instant>>>,
    /// Active breach incidents.
    incidents: RwLock<VecDeque<BreachIncident>>,
    /// Incident counter for ID generation.
    incident_counter: AtomicU64,
    /// Event counter for ID generation.
    event_counter: AtomicU64,
    /// Registered notifiers.
    notifiers: RwLock<Vec<Box<dyn BreachNotifier>>>,
}

impl BreachDetector {
    /// Create a new breach detector with default configuration.
    pub fn new() -> Self {
        Self {
            config: RwLock::new(DetectionConfig::default()),
            events: RwLock::new(HashMap::new()),
            failed_logins: RwLock::new(HashMap::new()),
            access_patterns: RwLock::new(HashMap::new()),
            incidents: RwLock::new(VecDeque::with_capacity(MAX_INCIDENTS_IN_MEMORY)),
            incident_counter: AtomicU64::new(1),
            event_counter: AtomicU64::new(1),
            notifiers: RwLock::new(Vec::new()),
        }
    }

    /// Create a new breach detector with custom configuration.
    pub fn with_config(config: DetectionConfig) -> Self {
        Self {
            config: RwLock::new(config),
            events: RwLock::new(HashMap::new()),
            failed_logins: RwLock::new(HashMap::new()),
            access_patterns: RwLock::new(HashMap::new()),
            incidents: RwLock::new(VecDeque::with_capacity(MAX_INCIDENTS_IN_MEMORY)),
            incident_counter: AtomicU64::new(1),
            event_counter: AtomicU64::new(1),
            notifiers: RwLock::new(Vec::new()),
        }
    }

    /// Update the detection configuration.
    pub fn update_config(&self, config: DetectionConfig) {
        *self.config.write() = config;
    }

    /// Get the current detection configuration.
    pub fn get_config(&self) -> DetectionConfig {
        self.config.read().clone()
    }

    /// Register a notifier.
    pub fn register_notifier(&self, notifier: Box<dyn BreachNotifier>) {
        self.notifiers.write().push(notifier);
    }

    /// Record a security event and check for breach patterns.
    pub fn record_event(&self, event: SecurityEvent) -> Option<BreachIncident> {
        let event_type = event.event_type;
        let now = Instant::now();

        // Store the event
        {
            let mut events = self.events.write();
            let queue = events
                .entry(event_type)
                .or_insert_with(|| VecDeque::with_capacity(MAX_EVENTS_IN_MEMORY));

            // Enforce max size
            while queue.len() >= MAX_EVENTS_IN_MEMORY {
                queue.pop_front();
            }

            queue.push_back(EventRecord {
                event: event.clone(),
                received_at: now,
            });
        }

        // Check for breach patterns based on event type
        match event_type {
            SecurityEventType::FailedLogin => self.check_failed_login_pattern(&event),
            SecurityEventType::UnauthorizedAccess => self.check_unauthorized_access(&event),
            SecurityEventType::UnusualAccessPattern => self.check_unusual_access_pattern(&event),
            SecurityEventType::AdminFromUnknownIp => self.check_admin_unknown_ip(&event),
            SecurityEventType::MassDataExport => self.check_mass_data_operation(&event, false),
            SecurityEventType::MassDataDeletion => self.check_mass_data_operation(&event, true),
            SecurityEventType::SessionHijacking => self.create_high_severity_incident(&event),
            SecurityEventType::SqlInjection => self.check_sql_injection(&event),
            SecurityEventType::BruteForceAttack => self.create_critical_incident(&event),
            SecurityEventType::PrivilegeEscalation => self.create_critical_incident(&event),
        }
    }

    /// Record a failed login attempt.
    pub fn record_failed_login(&self, username: &str, ip: Option<&str>) -> Option<BreachIncident> {
        let event = SecurityEvent::new(
            SecurityEventType::FailedLogin,
            &format!("Failed login attempt for user: {}", username),
        )
        .with_user(username);

        let event = if let Some(ip) = ip {
            event.with_ip(ip)
        } else {
            event
        };

        self.record_event(event)
    }

    /// Check if a failed login attempt triggers a breach incident.
    /// Convenience method that combines record and check in one call.
    /// Returns Some(BreachIncident) if the threshold is exceeded (brute force detected).
    pub fn check_failed_login(&self, ip: &str, username: &str) -> Option<BreachIncident> {
        self.record_failed_login(username, Some(ip))
    }

    /// Check for mass data access patterns that may indicate data exfiltration.
    /// Returns Some(BreachIncident) if the access pattern is suspicious.
    pub fn check_mass_access(&self, user: &str, count: u64) -> Option<BreachIncident> {
        let config = self.config.read();
        let threshold = config.mass_data_threshold;
        drop(config);

        if count >= threshold {
            let event = SecurityEvent::new(
                SecurityEventType::MassDataExport,
                &format!(
                    "Mass data access detected: {} accessed {} records",
                    user, count
                ),
            )
            .with_user(user)
            .with_metadata("record_count", &count.to_string());

            return self.record_event(event);
        }

        None
    }

    /// Record an unauthorized access attempt.
    pub fn record_unauthorized_access(
        &self,
        user: &str,
        resource: &str,
        permission: &str,
        ip: Option<&str>,
    ) -> Option<BreachIncident> {
        let event = SecurityEvent::new(
            SecurityEventType::UnauthorizedAccess,
            &format!(
                "Unauthorized access attempt: {} tried to {} on {}",
                user, permission, resource
            ),
        )
        .with_user(user)
        .with_resource(resource)
        .with_metadata("permission", permission);

        let event = if let Some(ip) = ip {
            event.with_ip(ip)
        } else {
            event
        };

        self.record_event(event)
    }

    /// Record a data access for pattern monitoring.
    pub fn record_data_access(&self, user: &str, resource: &str, row_count: u64) {
        let now = Instant::now();

        // Track access pattern
        {
            let mut patterns = self.access_patterns.write();
            let queue = patterns
                .entry(user.to_string())
                .or_insert_with(VecDeque::new);
            queue.push_back(now);
        }

        // Check for unusual access pattern
        let config = self.config.read();
        let window = Duration::from_secs(config.unusual_access_window_secs);
        let threshold = config.unusual_access_threshold;
        drop(config);

        let access_count = {
            let mut patterns = self.access_patterns.write();
            if let Some(queue) = patterns.get_mut(user) {
                // Remove old entries
                while let Some(front) = queue.front() {
                    if now.duration_since(*front) > window {
                        queue.pop_front();
                    } else {
                        break;
                    }
                }
                queue.len() as u32
            } else {
                0
            }
        };

        if access_count >= threshold {
            let event = SecurityEvent::new(
                SecurityEventType::UnusualAccessPattern,
                &format!(
                    "High volume data access detected: {} accessed {} rows from {}",
                    user, row_count, resource
                ),
            )
            .with_user(user)
            .with_resource(resource)
            .with_metadata("access_count", &access_count.to_string())
            .with_metadata("row_count", &row_count.to_string());

            self.record_event(event);
        }
    }

    /// Record an admin action from an IP.
    pub fn record_admin_action(&self, user: &str, action: &str, ip: &str) -> Option<BreachIncident> {
        let config = self.config.read();
        let trusted_ips = config.trusted_admin_ips.clone();
        drop(config);

        if !trusted_ips.contains(&ip.to_string()) {
            let event = SecurityEvent::new(
                SecurityEventType::AdminFromUnknownIp,
                &format!(
                    "Admin action '{}' performed by {} from untrusted IP {}",
                    action, user, ip
                ),
            )
            .with_user(user)
            .with_ip(ip)
            .with_metadata("action", action);

            self.record_event(event)
        } else {
            None
        }
    }

    /// Record a mass data operation.
    pub fn record_mass_data_operation(
        &self,
        user: &str,
        resource: &str,
        row_count: u64,
        is_deletion: bool,
    ) -> Option<BreachIncident> {
        let config = self.config.read();
        let threshold = config.mass_data_threshold;
        drop(config);

        if row_count >= threshold {
            let event_type = if is_deletion {
                SecurityEventType::MassDataDeletion
            } else {
                SecurityEventType::MassDataExport
            };

            let operation = if is_deletion { "deleted" } else { "exported" };
            let event = SecurityEvent::new(
                event_type,
                &format!(
                    "Mass data operation: {} {} {} rows from {}",
                    user, operation, row_count, resource
                ),
            )
            .with_user(user)
            .with_resource(resource)
            .with_metadata("row_count", &row_count.to_string());

            self.record_event(event)
        } else {
            None
        }
    }

    /// Check for failed login patterns.
    fn check_failed_login_pattern(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let key = event
            .user
            .clone()
            .or_else(|| event.ip_address.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let now = Instant::now();
        let config = self.config.read();
        let window = Duration::from_secs(config.failed_login_window_secs);
        let threshold = config.failed_login_threshold;
        drop(config);

        // Track failed login
        let count = {
            let mut logins = self.failed_logins.write();
            let queue = logins.entry(key.clone()).or_insert_with(VecDeque::new);
            queue.push_back(now);

            // Remove old entries
            while let Some(front) = queue.front() {
                if now.duration_since(*front) > window {
                    queue.pop_front();
                } else {
                    break;
                }
            }
            queue.len() as u32
        };

        if count >= threshold {
            // Clear the counter to avoid duplicate incidents
            {
                let mut logins = self.failed_logins.write();
                logins.remove(&key);
            }

            let severity = if count >= threshold * 2 {
                BreachSeverity::High
            } else {
                BreachSeverity::Medium
            };

            let mut incident = BreachIncident::new(
                SecurityEventType::FailedLogin,
                severity,
                &format!(
                    "Multiple failed login attempts detected: {} attempts in {} seconds",
                    count,
                    window.as_secs()
                ),
            )
            .with_related_event(&event.id)
            .with_detail("attempt_count", &count.to_string());

            if let Some(ref user) = event.user {
                incident = incident.with_affected_subject(user);
            }
            if let Some(ref ip) = event.ip_address {
                incident = incident.with_involved_ip(ip);
            }

            return self.create_and_notify_incident(incident);
        }

        None
    }

    /// Check for unauthorized access pattern.
    fn check_unauthorized_access(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let severity = BreachSeverity::Medium;

        let mut incident = BreachIncident::new(
            SecurityEventType::UnauthorizedAccess,
            severity,
            &event.description,
        )
        .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref ip) = event.ip_address {
            incident = incident.with_involved_ip(ip);
        }
        if let Some(ref resource) = event.resource {
            incident = incident.with_detail("resource", resource);
        }

        self.create_and_notify_incident(incident)
    }

    /// Check for unusual access patterns.
    fn check_unusual_access_pattern(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let mut incident = BreachIncident::new(
            SecurityEventType::UnusualAccessPattern,
            BreachSeverity::Medium,
            &event.description,
        )
        .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref ip) = event.ip_address {
            incident = incident.with_involved_ip(ip);
        }

        self.create_and_notify_incident(incident)
    }

    /// Check admin action from unknown IP.
    fn check_admin_unknown_ip(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let mut incident = BreachIncident::new(
            SecurityEventType::AdminFromUnknownIp,
            BreachSeverity::High,
            &event.description,
        )
        .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref ip) = event.ip_address {
            incident = incident.with_involved_ip(ip);
        }

        self.create_and_notify_incident(incident)
    }

    /// Check mass data operation.
    fn check_mass_data_operation(
        &self,
        event: &SecurityEvent,
        is_deletion: bool,
    ) -> Option<BreachIncident> {
        let severity = if is_deletion {
            BreachSeverity::Critical
        } else {
            BreachSeverity::High
        };

        let mut incident = BreachIncident::new(event.event_type, severity, &event.description)
            .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref resource) = event.resource {
            incident = incident.with_detail("resource", resource);
        }

        self.create_and_notify_incident(incident)
    }

    /// Check for SQL injection attempts.
    fn check_sql_injection(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let config = self.config.read();
        if !config.enable_sql_injection_detection {
            return None;
        }
        drop(config);

        let mut incident = BreachIncident::new(
            SecurityEventType::SqlInjection,
            BreachSeverity::High,
            &event.description,
        )
        .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref ip) = event.ip_address {
            incident = incident.with_involved_ip(ip);
        }

        self.create_and_notify_incident(incident)
    }

    /// Create a high severity incident.
    fn create_high_severity_incident(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let mut incident = BreachIncident::new(
            event.event_type,
            BreachSeverity::High,
            &event.description,
        )
        .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref ip) = event.ip_address {
            incident = incident.with_involved_ip(ip);
        }

        self.create_and_notify_incident(incident)
    }

    /// Create a critical severity incident.
    fn create_critical_incident(&self, event: &SecurityEvent) -> Option<BreachIncident> {
        let mut incident = BreachIncident::new(
            event.event_type,
            BreachSeverity::Critical,
            &event.description,
        )
        .with_related_event(&event.id);

        if let Some(ref user) = event.user {
            incident = incident.with_affected_subject(user);
        }
        if let Some(ref ip) = event.ip_address {
            incident = incident.with_involved_ip(ip);
        }

        self.create_and_notify_incident(incident)
    }

    /// Create an incident and send notifications.
    fn create_and_notify_incident(&self, mut incident: BreachIncident) -> Option<BreachIncident> {
        // Send notifications
        let notifiers = self.notifiers.read();
        let now = now_timestamp();

        for notifier in notifiers.iter() {
            match notifier.notify(&incident) {
                Ok(()) => {
                    incident
                        .notification_timestamps
                        .insert(notifier.name().to_string(), now);
                    tracing::info!(
                        "Breach notification sent via {}: {}",
                        notifier.name(),
                        incident.id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to send breach notification via {}: {}",
                        notifier.name(),
                        e
                    );
                }
            }
        }
        drop(notifiers);

        incident.notified = !incident.notification_timestamps.is_empty();

        // Store the incident
        {
            let mut incidents = self.incidents.write();
            while incidents.len() >= MAX_INCIDENTS_IN_MEMORY {
                incidents.pop_front();
            }
            incidents.push_back(incident.clone());
        }

        tracing::warn!(
            "Breach incident detected: {} (severity: {})",
            incident.id,
            incident.severity
        );

        Some(incident)
    }

    /// Get all breach incidents.
    pub fn list_incidents(&self) -> Vec<BreachIncident> {
        self.incidents.read().iter().cloned().collect()
    }

    /// List recent security events with optional type filter.
    pub fn list_events(&self, event_type: Option<&str>, limit: usize) -> Vec<SecurityEvent> {
        let events = self.events.read();

        let filter_type = event_type.and_then(|t| match t {
            "failed_login" => Some(SecurityEventType::FailedLogin),
            "unauthorized_access" => Some(SecurityEventType::UnauthorizedAccess),
            "unusual_access_pattern" => Some(SecurityEventType::UnusualAccessPattern),
            "admin_from_unknown_ip" => Some(SecurityEventType::AdminFromUnknownIp),
            "mass_data_export" => Some(SecurityEventType::MassDataExport),
            "mass_data_deletion" => Some(SecurityEventType::MassDataDeletion),
            "session_hijacking" => Some(SecurityEventType::SessionHijacking),
            "sql_injection" => Some(SecurityEventType::SqlInjection),
            "brute_force_attack" => Some(SecurityEventType::BruteForceAttack),
            "privilege_escalation" => Some(SecurityEventType::PrivilegeEscalation),
            _ => None,
        });

        let mut all_events: Vec<SecurityEvent> = if let Some(filter) = filter_type {
            events
                .get(&filter)
                .map(|q| q.iter().map(|r| r.event.clone()).collect())
                .unwrap_or_default()
        } else {
            events
                .values()
                .flat_map(|q| q.iter().map(|r| r.event.clone()))
                .collect()
        };

        // Sort by timestamp descending (most recent first)
        all_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_events.truncate(limit);
        all_events
    }

    /// Get incidents with optional filters.
    pub fn get_incidents(
        &self,
        status: Option<IncidentStatus>,
        severity: Option<BreachSeverity>,
        limit: usize,
    ) -> Vec<BreachIncident> {
        let incidents = self.incidents.read();
        incidents
            .iter()
            .rev()
            .filter(|i| status.is_none() || Some(i.status) == status)
            .filter(|i| severity.is_none() || Some(i.severity) == severity)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get a specific incident by ID.
    pub fn get_incident(&self, id: &str) -> Option<BreachIncident> {
        self.incidents.read().iter().find(|i| i.id == id).cloned()
    }

    /// Acknowledge an incident.
    pub fn acknowledge_incident(&self, id: &str, acknowledged_by: &str) -> Option<BreachIncident> {
        let mut incidents = self.incidents.write();
        for incident in incidents.iter_mut() {
            if incident.id == id {
                incident.status = IncidentStatus::Acknowledged;
                incident.acknowledged_by = Some(acknowledged_by.to_string());
                incident.acknowledged_at = Some(now_timestamp());
                return Some(incident.clone());
            }
        }
        None
    }

    /// Resolve an incident.
    pub fn resolve_incident(
        &self,
        id: &str,
        resolution_notes: &str,
        false_positive: bool,
    ) -> Option<BreachIncident> {
        let mut incidents = self.incidents.write();
        for incident in incidents.iter_mut() {
            if incident.id == id {
                incident.status = if false_positive {
                    IncidentStatus::FalsePositive
                } else {
                    IncidentStatus::Resolved
                };
                incident.resolution_notes = Some(resolution_notes.to_string());
                incident.resolved_at = Some(now_timestamp());
                return Some(incident.clone());
            }
        }
        None
    }

    /// Generate an incident report for compliance purposes.
    pub fn generate_report(&self, id: &str) -> Option<IncidentReport> {
        let incident = self.get_incident(id)?;

        // Gather related events
        let events = self.events.read();
        let related_events: Vec<SecurityEvent> = events
            .values()
            .flat_map(|q| q.iter())
            .filter(|r| incident.related_events.contains(&r.event.id))
            .map(|r| r.event.clone())
            .collect();

        Some(IncidentReport {
            incident,
            related_events,
            generated_at: now_timestamp(),
            generated_at_formatted: format_timestamp(now_timestamp()),
        })
    }

    /// Get incident statistics.
    pub fn get_stats(&self) -> BreachStats {
        let incidents = self.incidents.read();

        let mut stats = BreachStats {
            total_incidents: incidents.len(),
            open_incidents: 0,
            acknowledged_incidents: 0,
            resolved_incidents: 0,
            false_positives: 0,
            by_severity: HashMap::new(),
            by_type: HashMap::new(),
        };

        for incident in incidents.iter() {
            match incident.status {
                IncidentStatus::Open => stats.open_incidents += 1,
                IncidentStatus::Acknowledged | IncidentStatus::Investigating => {
                    stats.acknowledged_incidents += 1
                }
                IncidentStatus::Resolved => stats.resolved_incidents += 1,
                IncidentStatus::FalsePositive => stats.false_positives += 1,
            }

            *stats
                .by_severity
                .entry(incident.severity.to_string())
                .or_insert(0) += 1;
            *stats
                .by_type
                .entry(incident.incident_type.to_string())
                .or_insert(0) += 1;
        }

        stats
    }

    /// Clean up old events and patterns.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let retention = Duration::from_secs(24 * 60 * 60); // 24 hours

        // Clean up events
        {
            let mut events = self.events.write();
            for queue in events.values_mut() {
                while let Some(front) = queue.front() {
                    if now.duration_since(front.received_at) > retention {
                        queue.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }

        // Clean up failed login tracking
        {
            let mut logins = self.failed_logins.write();
            let config = self.config.read();
            let window = Duration::from_secs(config.failed_login_window_secs);
            drop(config);

            for queue in logins.values_mut() {
                while let Some(front) = queue.front() {
                    if now.duration_since(*front) > window {
                        queue.pop_front();
                    } else {
                        break;
                    }
                }
            }
            logins.retain(|_, v| !v.is_empty());
        }

        // Clean up access patterns
        {
            let mut patterns = self.access_patterns.write();
            let config = self.config.read();
            let window = Duration::from_secs(config.unusual_access_window_secs);
            drop(config);

            for queue in patterns.values_mut() {
                while let Some(front) = queue.front() {
                    if now.duration_since(*front) > window {
                        queue.pop_front();
                    } else {
                        break;
                    }
                }
            }
            patterns.retain(|_, v| !v.is_empty());
        }
    }
}

impl Default for BreachDetector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Incident Report
// =============================================================================

/// Full incident report for compliance documentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentReport {
    /// The breach incident.
    pub incident: BreachIncident,
    /// Related security events.
    pub related_events: Vec<SecurityEvent>,
    /// When the report was generated (Unix milliseconds).
    pub generated_at: u64,
    /// Human-readable generation timestamp.
    pub generated_at_formatted: String,
}

// =============================================================================
// Breach Statistics
// =============================================================================

/// Statistics about breach incidents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreachStats {
    /// Total number of incidents.
    pub total_incidents: usize,
    /// Number of open incidents.
    pub open_incidents: usize,
    /// Number of acknowledged incidents.
    pub acknowledged_incidents: usize,
    /// Number of resolved incidents.
    pub resolved_incidents: usize,
    /// Number of false positives.
    pub false_positives: usize,
    /// Incidents by severity.
    pub by_severity: HashMap<String, usize>,
    /// Incidents by type.
    pub by_type: HashMap<String, usize>,
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

/// Generate a unique event ID.
fn generate_event_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("evt-{:012}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

/// Generate a unique incident ID.
fn generate_incident_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("inc-{:012}", COUNTER.fetch_add(1, Ordering::SeqCst))
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

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// =============================================================================
// API Handler Types
// =============================================================================

/// Request to acknowledge a breach incident.
#[derive(Debug, Deserialize)]
pub struct AcknowledgeRequest {
    pub acknowledged_by: String,
}

/// Request to resolve a breach incident.
#[derive(Debug, Deserialize)]
pub struct ResolveRequest {
    pub resolution_notes: String,
    #[serde(default)]
    pub false_positive: bool,
}

/// Query parameters for listing breaches.
#[derive(Debug, Deserialize)]
pub struct ListBreachesQuery {
    pub status: Option<String>,
    pub severity: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

/// Query parameters for listing security events.
#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub event_type: Option<String>,
}

// =============================================================================
// HTTP Handlers
// =============================================================================

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

/// List breach incidents response.
#[derive(Debug, serde::Serialize)]
pub struct ListBreachesResponse {
    pub incidents: Vec<BreachIncident>,
    pub total: usize,
    pub stats: BreachStats,
}

/// Security events response.
#[derive(Debug, serde::Serialize)]
pub struct SecurityEventsResponse {
    pub events: Vec<SecurityEvent>,
    pub total: usize,
}

/// List all breach incidents.
/// GET /api/v1/compliance/breaches
pub async fn list_breaches(
    State(state): State<AppState>,
    Query(params): Query<ListBreachesQuery>,
) -> Json<ListBreachesResponse> {
    let status = params.status.as_ref().and_then(|s| match s.as_str() {
        "open" => Some(IncidentStatus::Open),
        "acknowledged" => Some(IncidentStatus::Acknowledged),
        "investigating" => Some(IncidentStatus::Investigating),
        "resolved" => Some(IncidentStatus::Resolved),
        "false_positive" => Some(IncidentStatus::FalsePositive),
        _ => None,
    });

    let severity = params.severity.as_ref().and_then(|s| match s.as_str() {
        "low" => Some(BreachSeverity::Low),
        "medium" => Some(BreachSeverity::Medium),
        "high" => Some(BreachSeverity::High),
        "critical" => Some(BreachSeverity::Critical),
        _ => None,
    });

    let incidents = state.breach_detector.get_incidents(status, severity, params.limit);
    let total = incidents.len();
    let stats = state.breach_detector.get_stats();

    Json(ListBreachesResponse {
        incidents,
        total,
        stats,
    })
}

/// Get a specific breach incident.
/// GET /api/v1/compliance/breaches/:id
pub async fn get_breach(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.breach_detector.get_incident(&id) {
        Some(incident) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "incident": incident,
        }))),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "success": false,
            "error": format!("Incident '{}' not found", id),
        }))),
    }
}

/// Acknowledge a breach incident.
/// POST /api/v1/compliance/breaches/:id/acknowledge
pub async fn acknowledge_breach(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<AcknowledgeRequest>,
) -> impl IntoResponse {
    match state.breach_detector.acknowledge_incident(&id, &request.acknowledged_by) {
        Some(incident) => {
            tracing::info!(
                "Breach incident {} acknowledged by {}",
                id,
                request.acknowledged_by
            );
            (StatusCode::OK, Json(serde_json::json!({
                "success": true,
                "incident": incident,
                "message": "Incident acknowledged successfully",
            })))
        }
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "success": false,
            "error": format!("Incident '{}' not found", id),
        }))),
    }
}

/// Resolve a breach incident.
/// POST /api/v1/compliance/breaches/:id/resolve
pub async fn resolve_breach(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ResolveRequest>,
) -> impl IntoResponse {
    match state.breach_detector.resolve_incident(&id, &request.resolution_notes, request.false_positive) {
        Some(incident) => {
            let status_str = if request.false_positive { "false positive" } else { "resolved" };
            tracing::info!("Breach incident {} marked as {}", id, status_str);
            (StatusCode::OK, Json(serde_json::json!({
                "success": true,
                "incident": incident,
                "message": format!("Incident marked as {}", status_str),
            })))
        }
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "success": false,
            "error": format!("Incident '{}' not found", id),
        }))),
    }
}

/// Get an incident report for compliance documentation.
/// GET /api/v1/compliance/breaches/:id/report
pub async fn get_breach_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.breach_detector.generate_report(&id) {
        Some(report) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "report": report,
        }))),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "success": false,
            "error": format!("Incident '{}' not found", id),
        }))),
    }
}

/// List recent security events.
/// GET /api/v1/compliance/security-events
pub async fn list_security_events(
    State(state): State<AppState>,
    Query(params): Query<ListEventsQuery>,
) -> Json<SecurityEventsResponse> {
    let events = state.breach_detector.list_events(params.event_type.as_deref(), params.limit);
    let total = events.len();

    Json(SecurityEventsResponse { events, total })
}

/// Get breach detection statistics.
/// GET /api/v1/compliance/breaches/stats
pub async fn get_breach_stats(
    State(state): State<AppState>,
) -> Json<BreachStats> {
    Json(state.breach_detector.get_stats())
}

/// Manually trigger a cleanup of old events and patterns.
/// POST /api/v1/compliance/breaches/cleanup
pub async fn trigger_cleanup(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    state.breach_detector.cleanup();
    Json(serde_json::json!({
        "success": true,
        "message": "Cleanup completed successfully",
    }))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_event_creation() {
        let event = SecurityEvent::new(SecurityEventType::FailedLogin, "Test failed login")
            .with_user("testuser")
            .with_ip("192.168.1.1");

        assert!(event.id.starts_with("evt-"));
        assert_eq!(event.event_type, SecurityEventType::FailedLogin);
        assert_eq!(event.user, Some("testuser".to_string()));
        assert_eq!(event.ip_address, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_breach_incident_creation() {
        let incident = BreachIncident::new(
            SecurityEventType::FailedLogin,
            BreachSeverity::Medium,
            "Multiple failed logins detected",
        )
        .with_affected_subject("user1")
        .with_involved_ip("10.0.0.1");

        assert!(incident.id.starts_with("inc-"));
        assert_eq!(incident.severity, BreachSeverity::Medium);
        assert!(incident.affected_subjects.contains(&"user1".to_string()));
        assert!(incident.involved_ips.contains(&"10.0.0.1".to_string()));
    }

    #[test]
    fn test_breach_detector_failed_login_detection() {
        let config = DetectionConfig {
            failed_login_threshold: 3,
            failed_login_window_secs: 300,
            ..Default::default()
        };
        let detector = BreachDetector::with_config(config);

        // First two attempts should not trigger
        assert!(detector
            .record_failed_login("user1", Some("192.168.1.1"))
            .is_none());
        assert!(detector
            .record_failed_login("user1", Some("192.168.1.1"))
            .is_none());

        // Third attempt should trigger incident
        let incident = detector.record_failed_login("user1", Some("192.168.1.1"));
        assert!(incident.is_some());

        let incident = incident.unwrap();
        assert_eq!(incident.incident_type, SecurityEventType::FailedLogin);
        assert!(incident.affected_subjects.contains(&"user1".to_string()));
    }

    #[test]
    fn test_breach_detector_unauthorized_access() {
        let detector = BreachDetector::new();

        let incident =
            detector.record_unauthorized_access("user1", "admin/users", "write", Some("10.0.0.1"));

        assert!(incident.is_some());
        let incident = incident.unwrap();
        assert_eq!(incident.incident_type, SecurityEventType::UnauthorizedAccess);
    }

    #[test]
    fn test_breach_detector_mass_data_operation() {
        let config = DetectionConfig {
            mass_data_threshold: 100,
            ..Default::default()
        };
        let detector = BreachDetector::with_config(config);

        // Below threshold should not trigger
        assert!(detector
            .record_mass_data_operation("user1", "users", 50, false)
            .is_none());

        // Above threshold should trigger
        let incident = detector.record_mass_data_operation("user1", "users", 1000, true);
        assert!(incident.is_some());

        let incident = incident.unwrap();
        assert_eq!(incident.incident_type, SecurityEventType::MassDataDeletion);
        assert_eq!(incident.severity, BreachSeverity::Critical);
    }

    #[test]
    fn test_breach_detector_admin_unknown_ip() {
        let config = DetectionConfig {
            trusted_admin_ips: vec!["127.0.0.1".to_string()],
            ..Default::default()
        };
        let detector = BreachDetector::with_config(config);

        // Trusted IP should not trigger
        assert!(detector
            .record_admin_action("admin", "delete_user", "127.0.0.1")
            .is_none());

        // Unknown IP should trigger
        let incident = detector.record_admin_action("admin", "delete_user", "192.168.1.100");
        assert!(incident.is_some());

        let incident = incident.unwrap();
        assert_eq!(incident.incident_type, SecurityEventType::AdminFromUnknownIp);
        assert_eq!(incident.severity, BreachSeverity::High);
    }

    #[test]
    fn test_incident_acknowledge_and_resolve() {
        let detector = BreachDetector::new();

        // Create an incident
        let incident =
            detector.record_unauthorized_access("user1", "admin/users", "write", Some("10.0.0.1"));
        let incident = incident.expect("should create incident");

        // Acknowledge it
        let acknowledged = detector.acknowledge_incident(&incident.id, "admin");
        assert!(acknowledged.is_some());
        let acknowledged = acknowledged.unwrap();
        assert_eq!(acknowledged.status, IncidentStatus::Acknowledged);
        assert_eq!(acknowledged.acknowledged_by, Some("admin".to_string()));

        // Resolve it
        let resolved = detector.resolve_incident(&incident.id, "Investigated and addressed", false);
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.status, IncidentStatus::Resolved);
    }

    #[test]
    fn test_incident_report_generation() {
        let detector = BreachDetector::new();

        let incident =
            detector.record_unauthorized_access("user1", "admin/users", "write", Some("10.0.0.1"));
        let incident = incident.expect("should create incident");

        let report = detector.generate_report(&incident.id);
        assert!(report.is_some());

        let report = report.unwrap();
        assert_eq!(report.incident.id, incident.id);
    }

    #[test]
    fn test_breach_stats() {
        let detector = BreachDetector::new();

        // Create some incidents
        detector.record_unauthorized_access("user1", "admin", "write", Some("10.0.0.1"));
        detector.record_unauthorized_access("user2", "admin", "write", Some("10.0.0.2"));

        let stats = detector.get_stats();
        assert_eq!(stats.total_incidents, 2);
        assert_eq!(stats.open_incidents, 2);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(BreachSeverity::Low < BreachSeverity::Medium);
        assert!(BreachSeverity::Medium < BreachSeverity::High);
        assert!(BreachSeverity::High < BreachSeverity::Critical);
    }

    #[test]
    fn test_requires_immediate_notification() {
        let low_incident = BreachIncident::new(
            SecurityEventType::FailedLogin,
            BreachSeverity::Low,
            "test",
        );
        assert!(!low_incident.requires_immediate_notification());

        let high_incident = BreachIncident::new(
            SecurityEventType::MassDataDeletion,
            BreachSeverity::High,
            "test",
        );
        assert!(high_incident.requires_immediate_notification());

        let critical_incident = BreachIncident::new(
            SecurityEventType::BruteForceAttack,
            BreachSeverity::Critical,
            "test",
        );
        assert!(critical_incident.requires_immediate_notification());
    }
}
