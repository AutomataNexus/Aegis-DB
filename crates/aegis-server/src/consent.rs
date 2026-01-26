//! Aegis Consent Management Module
//!
//! GDPR/CCPA compliance with consent tracking, audit trails, and enforcement.
//! Provides APIs for recording, querying, and withdrawing user consent.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Consent Types
// =============================================================================

/// Purpose categories for consent under GDPR/CCPA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Marketing communications and promotional materials
    Marketing,
    /// Analytics and usage tracking
    Analytics,
    /// Sharing data with third parties
    ThirdPartySharing,
    /// General data processing for service operation
    DataProcessing,
    /// Personalization and recommendations
    Personalization,
    /// Location tracking and geolocation services
    LocationTracking,
    /// Profiling for automated decision-making
    Profiling,
    /// Cross-device tracking
    CrossDeviceTracking,
    /// Advertising and targeted ads
    Advertising,
    /// Research and statistical purposes
    Research,
    /// CCPA-specific: Do Not Sell My Personal Information
    DoNotSell,
    /// Custom purpose with arbitrary identifier
    Custom(u32),
}

impl Purpose {
    /// Get a human-readable description of the purpose.
    pub fn description(&self) -> &'static str {
        match self {
            Purpose::Marketing => "Marketing communications and promotional materials",
            Purpose::Analytics => "Analytics and usage tracking",
            Purpose::ThirdPartySharing => "Sharing data with third parties",
            Purpose::DataProcessing => "General data processing for service operation",
            Purpose::Personalization => "Personalization and recommendations",
            Purpose::LocationTracking => "Location tracking and geolocation services",
            Purpose::Profiling => "Profiling for automated decision-making",
            Purpose::CrossDeviceTracking => "Cross-device tracking",
            Purpose::Advertising => "Advertising and targeted ads",
            Purpose::Research => "Research and statistical purposes",
            Purpose::DoNotSell => "CCPA: Do Not Sell My Personal Information",
            Purpose::Custom(_) => "Custom purpose",
        }
    }

    /// Parse a purpose from a string identifier.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "marketing" => Some(Purpose::Marketing),
            "analytics" => Some(Purpose::Analytics),
            "third_party_sharing" | "thirdpartysharing" => Some(Purpose::ThirdPartySharing),
            "data_processing" | "dataprocessing" => Some(Purpose::DataProcessing),
            "personalization" => Some(Purpose::Personalization),
            "location_tracking" | "locationtracking" => Some(Purpose::LocationTracking),
            "profiling" => Some(Purpose::Profiling),
            "cross_device_tracking" | "crossdevicetracking" => Some(Purpose::CrossDeviceTracking),
            "advertising" => Some(Purpose::Advertising),
            "research" => Some(Purpose::Research),
            "do_not_sell" | "donotsell" => Some(Purpose::DoNotSell),
            s if s.starts_with("custom:") => {
                s.strip_prefix("custom:").and_then(|id| id.parse().ok()).map(Purpose::Custom)
            }
            _ => None,
        }
    }

    /// Convert purpose to a string identifier.
    pub fn to_str(&self) -> String {
        match self {
            Purpose::Marketing => "marketing".to_string(),
            Purpose::Analytics => "analytics".to_string(),
            Purpose::ThirdPartySharing => "third_party_sharing".to_string(),
            Purpose::DataProcessing => "data_processing".to_string(),
            Purpose::Personalization => "personalization".to_string(),
            Purpose::LocationTracking => "location_tracking".to_string(),
            Purpose::Profiling => "profiling".to_string(),
            Purpose::CrossDeviceTracking => "cross_device_tracking".to_string(),
            Purpose::Advertising => "advertising".to_string(),
            Purpose::Research => "research".to_string(),
            Purpose::DoNotSell => "do_not_sell".to_string(),
            Purpose::Custom(id) => format!("custom:{}", id),
        }
    }
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

/// Source of consent (how it was obtained).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentSource {
    /// User explicitly provided consent via web form
    WebForm,
    /// Consent obtained through mobile app
    MobileApp,
    /// Consent obtained through API call
    Api,
    /// Consent imported from external system
    Import,
    /// Consent obtained through paper form (digitized)
    PaperForm,
    /// Consent obtained via email confirmation
    Email,
    /// Consent obtained via verbal agreement (logged)
    Verbal,
    /// System-generated default (e.g., legitimate interest)
    SystemDefault,
    /// Custom source
    Custom(String),
}

impl ConsentSource {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "web_form" | "webform" | "web" => ConsentSource::WebForm,
            "mobile_app" | "mobileapp" | "mobile" => ConsentSource::MobileApp,
            "api" => ConsentSource::Api,
            "import" => ConsentSource::Import,
            "paper_form" | "paperform" | "paper" => ConsentSource::PaperForm,
            "email" => ConsentSource::Email,
            "verbal" => ConsentSource::Verbal,
            "system_default" | "systemdefault" | "system" => ConsentSource::SystemDefault,
            other => ConsentSource::Custom(other.to_string()),
        }
    }
}

impl std::fmt::Display for ConsentSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsentSource::WebForm => write!(f, "web_form"),
            ConsentSource::MobileApp => write!(f, "mobile_app"),
            ConsentSource::Api => write!(f, "api"),
            ConsentSource::Import => write!(f, "import"),
            ConsentSource::PaperForm => write!(f, "paper_form"),
            ConsentSource::Email => write!(f, "email"),
            ConsentSource::Verbal => write!(f, "verbal"),
            ConsentSource::SystemDefault => write!(f, "system_default"),
            ConsentSource::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// A consent record tracking a subject's consent for a specific purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// Unique identifier for this consent record
    pub id: String,
    /// Identifier of the data subject (user/customer)
    pub subject_id: String,
    /// The purpose for which consent is given/denied
    pub purpose: Purpose,
    /// Whether consent is granted (true) or denied/withdrawn (false)
    pub granted: bool,
    /// Timestamp when this consent was recorded (milliseconds since epoch)
    pub timestamp: u64,
    /// How the consent was obtained
    pub source: ConsentSource,
    /// Optional expiration timestamp (milliseconds since epoch)
    pub expires_at: Option<u64>,
    /// Privacy policy version this consent applies to
    pub version: String,
    /// Additional metadata (e.g., IP address, user agent, consent text shown)
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ConsentRecord {
    /// Create a new consent record.
    pub fn new(
        subject_id: &str,
        purpose: Purpose,
        granted: bool,
        source: ConsentSource,
        version: &str,
    ) -> Self {
        Self {
            id: generate_consent_id(),
            subject_id: subject_id.to_string(),
            purpose,
            granted,
            timestamp: now_timestamp_ms(),
            source,
            expires_at: None,
            version: version.to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Check if this consent has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            now_timestamp_ms() > expires_at
        } else {
            false
        }
    }

    /// Check if consent is currently valid (granted and not expired).
    pub fn is_valid(&self) -> bool {
        self.granted && !self.is_expired()
    }
}

/// Consent history entry for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentHistoryEntry {
    /// The consent record at this point in history
    pub record: ConsentRecord,
    /// Action that created this entry
    pub action: ConsentAction,
    /// Who/what initiated this action (user_id, system, etc.)
    pub actor: String,
    /// Reason for the action (if applicable)
    pub reason: Option<String>,
}

/// Actions that can be performed on consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentAction {
    /// Initial consent grant
    Grant,
    /// Consent denial
    Deny,
    /// Consent withdrawal
    Withdraw,
    /// Consent renewal/refresh
    Renew,
    /// Consent update (e.g., policy version change)
    Update,
    /// Consent expiration
    Expire,
}

// =============================================================================
// Consent Manager
// =============================================================================

/// Manager for consent records with in-memory storage.
/// In production, this would be backed by persistent storage.
pub struct ConsentManager {
    /// Current consent records indexed by subject_id -> purpose -> record
    records: RwLock<HashMap<String, HashMap<Purpose, ConsentRecord>>>,
    /// Full audit history indexed by subject_id
    history: RwLock<HashMap<String, Vec<ConsentHistoryEntry>>>,
    /// CCPA Do Not Sell list (subject_ids)
    do_not_sell_list: RwLock<std::collections::HashSet<String>>,
    /// Record counter for ID generation
    record_counter: RwLock<u64>,
}

impl ConsentManager {
    /// Create a new consent manager.
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
            history: RwLock::new(HashMap::new()),
            do_not_sell_list: RwLock::new(std::collections::HashSet::new()),
            record_counter: RwLock::new(0),
        }
    }

    /// Record new consent or update existing consent.
    pub fn record_consent(
        &self,
        subject_id: &str,
        purpose: Purpose,
        granted: bool,
        source: ConsentSource,
        version: &str,
        expires_at: Option<u64>,
        metadata: Option<HashMap<String, String>>,
        actor: &str,
    ) -> ConsentRecord {
        let mut record = ConsentRecord::new(subject_id, purpose, granted, source, version);
        record.expires_at = expires_at;
        if let Some(meta) = metadata {
            record.metadata = meta;
        }

        // Determine the action
        let action = {
            let records = self.records.read();
            if let Some(subject_records) = records.get(subject_id) {
                if let Some(existing) = subject_records.get(&purpose) {
                    if granted && !existing.granted {
                        ConsentAction::Grant
                    } else if !granted && existing.granted {
                        ConsentAction::Withdraw
                    } else if granted {
                        ConsentAction::Renew
                    } else {
                        ConsentAction::Deny
                    }
                } else if granted {
                    ConsentAction::Grant
                } else {
                    ConsentAction::Deny
                }
            } else if granted {
                ConsentAction::Grant
            } else {
                ConsentAction::Deny
            }
        };

        // Store the record
        {
            let mut records = self.records.write();
            let subject_records = records.entry(subject_id.to_string()).or_default();
            subject_records.insert(purpose, record.clone());
        }

        // Update Do Not Sell list if applicable
        if purpose == Purpose::DoNotSell {
            let mut dns_list = self.do_not_sell_list.write();
            if granted {
                // If they grant "do not sell", add to the list
                dns_list.insert(subject_id.to_string());
            } else {
                // If they opt back in (deny do-not-sell), remove from list
                dns_list.remove(subject_id);
            }
        }

        // Add to history
        {
            let mut history = self.history.write();
            let subject_history = history.entry(subject_id.to_string()).or_default();
            subject_history.push(ConsentHistoryEntry {
                record: record.clone(),
                action,
                actor: actor.to_string(),
                reason: None,
            });
        }

        tracing::info!(
            "Consent recorded: subject={}, purpose={}, granted={}, action={:?}",
            subject_id,
            purpose,
            granted,
            action
        );

        record
    }

    /// Get current consent status for a subject.
    pub fn get_consent(&self, subject_id: &str) -> Vec<ConsentRecord> {
        let records = self.records.read();
        records
            .get(subject_id)
            .map(|r| r.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get consent for a specific purpose.
    pub fn get_consent_for_purpose(&self, subject_id: &str, purpose: Purpose) -> Option<ConsentRecord> {
        let records = self.records.read();
        records
            .get(subject_id)
            .and_then(|r| r.get(&purpose))
            .cloned()
    }

    /// Check if consent is granted for a specific purpose.
    /// This is the main enforcement function to call before data operations.
    pub fn check_consent(&self, subject_id: &str, purpose: Purpose) -> bool {
        let records = self.records.read();
        if let Some(subject_records) = records.get(subject_id) {
            if let Some(record) = subject_records.get(&purpose) {
                return record.is_valid();
            }
        }
        false
    }

    /// Withdraw consent for a specific purpose.
    pub fn withdraw_consent(
        &self,
        subject_id: &str,
        purpose: Purpose,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<ConsentRecord, String> {
        // Check if consent exists
        let existing = {
            let records = self.records.read();
            records
                .get(subject_id)
                .and_then(|r| r.get(&purpose))
                .cloned()
        };

        let existing = existing.ok_or_else(|| {
            format!(
                "No consent record found for subject {} and purpose {}",
                subject_id, purpose
            )
        })?;

        // Create withdrawal record
        let mut record = ConsentRecord::new(
            subject_id,
            purpose,
            false,
            existing.source.clone(),
            &existing.version,
        );
        record.metadata = existing.metadata.clone();
        record.metadata.insert("withdrawn_from".to_string(), existing.id.clone());

        // Update records
        {
            let mut records = self.records.write();
            if let Some(subject_records) = records.get_mut(subject_id) {
                subject_records.insert(purpose, record.clone());
            }
        }

        // Update Do Not Sell list if applicable
        if purpose == Purpose::DoNotSell {
            let mut dns_list = self.do_not_sell_list.write();
            // Withdrawing "do not sell" means they're opting back in
            dns_list.remove(subject_id);
        }

        // Add to history
        {
            let mut history = self.history.write();
            let subject_history = history.entry(subject_id.to_string()).or_default();
            subject_history.push(ConsentHistoryEntry {
                record: record.clone(),
                action: ConsentAction::Withdraw,
                actor: actor.to_string(),
                reason: reason.map(String::from),
            });
        }

        tracing::info!(
            "Consent withdrawn: subject={}, purpose={}, actor={}",
            subject_id,
            purpose,
            actor
        );

        Ok(record)
    }

    /// Get consent audit history for a subject.
    pub fn get_history(&self, subject_id: &str) -> Vec<ConsentHistoryEntry> {
        let history = self.history.read();
        history.get(subject_id).cloned().unwrap_or_default()
    }

    /// Get history for a specific purpose.
    pub fn get_history_for_purpose(&self, subject_id: &str, purpose: Purpose) -> Vec<ConsentHistoryEntry> {
        let history = self.history.read();
        history
            .get(subject_id)
            .map(|h| {
                h.iter()
                    .filter(|e| e.record.purpose == purpose)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a subject is on the CCPA Do Not Sell list.
    pub fn is_on_do_not_sell_list(&self, subject_id: &str) -> bool {
        self.do_not_sell_list.read().contains(subject_id)
    }

    /// Get all subjects on the Do Not Sell list.
    pub fn get_do_not_sell_list(&self) -> Vec<String> {
        self.do_not_sell_list.read().iter().cloned().collect()
    }

    /// Export all consent records for a subject (for data portability/GDPR).
    pub fn export_subject_data(&self, subject_id: &str) -> SubjectConsentExport {
        let records = self.get_consent(subject_id);
        let history = self.get_history(subject_id);
        let on_dns_list = self.is_on_do_not_sell_list(subject_id);

        SubjectConsentExport {
            subject_id: subject_id.to_string(),
            export_timestamp: now_timestamp_ms(),
            current_consents: records,
            consent_history: history,
            on_do_not_sell_list: on_dns_list,
        }
    }

    /// Delete all consent records for a subject (for GDPR right to erasure).
    pub fn delete_subject_data(&self, subject_id: &str, actor: &str) -> bool {
        let had_records = {
            let mut records = self.records.write();
            records.remove(subject_id).is_some()
        };

        let had_history = {
            let mut history = self.history.write();
            history.remove(subject_id).is_some()
        };

        {
            let mut dns_list = self.do_not_sell_list.write();
            dns_list.remove(subject_id);
        }

        if had_records || had_history {
            tracing::info!(
                "Subject consent data deleted: subject={}, actor={}",
                subject_id,
                actor
            );
        }

        had_records || had_history
    }

    /// Get statistics about consent records.
    pub fn get_stats(&self) -> ConsentStats {
        let records = self.records.read();
        let history = self.history.read();
        let dns_list = self.do_not_sell_list.read();

        let total_subjects = records.len();
        let total_records: usize = records.values().map(|r| r.len()).sum();
        let total_history_entries: usize = history.values().map(|h| h.len()).sum();

        let mut consents_by_purpose: HashMap<String, (usize, usize)> = HashMap::new();
        for subject_records in records.values() {
            for (purpose, record) in subject_records {
                let entry = consents_by_purpose
                    .entry(purpose.to_str())
                    .or_insert((0, 0));
                if record.is_valid() {
                    entry.0 += 1;
                } else {
                    entry.1 += 1;
                }
            }
        }

        ConsentStats {
            total_subjects,
            total_records,
            total_history_entries,
            do_not_sell_count: dns_list.len(),
            consents_by_purpose,
        }
    }
}

impl Default for ConsentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Export format for subject consent data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectConsentExport {
    pub subject_id: String,
    pub export_timestamp: u64,
    pub current_consents: Vec<ConsentRecord>,
    pub consent_history: Vec<ConsentHistoryEntry>,
    pub on_do_not_sell_list: bool,
}

/// Statistics about consent records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentStats {
    pub total_subjects: usize,
    pub total_records: usize,
    pub total_history_entries: usize,
    pub do_not_sell_count: usize,
    /// Map of purpose -> (granted_count, denied_count)
    pub consents_by_purpose: HashMap<String, (usize, usize)>,
}

// =============================================================================
// API Request/Response Types
// =============================================================================

/// Request to record new consent.
#[derive(Debug, Clone, Deserialize)]
pub struct RecordConsentRequest {
    pub subject_id: String,
    pub purpose: String,
    pub granted: bool,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_version")]
    pub version: String,
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

fn default_source() -> String {
    "api".to_string()
}

fn default_version() -> String {
    "1.0".to_string()
}

/// Response for consent operations.
#[derive(Debug, Clone, Serialize)]
pub struct ConsentResponse {
    pub success: bool,
    pub record: Option<ConsentRecord>,
    pub error: Option<String>,
}

/// Response for consent status query.
#[derive(Debug, Clone, Serialize)]
pub struct ConsentStatusResponse {
    pub subject_id: String,
    pub consents: Vec<ConsentRecord>,
    pub on_do_not_sell_list: bool,
}

/// Request to withdraw consent.
#[derive(Debug, Clone, Deserialize)]
pub struct WithdrawConsentRequest {
    pub reason: Option<String>,
}

// =============================================================================
// API Handlers
// =============================================================================

/// Record new consent.
/// POST /api/v1/compliance/consent
pub async fn record_consent(
    State(state): State<AppState>,
    Json(request): Json<RecordConsentRequest>,
) -> impl IntoResponse {
    let purpose = match Purpose::from_str(&request.purpose) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ConsentResponse {
                    success: false,
                    record: None,
                    error: Some(format!("Invalid purpose: {}", request.purpose)),
                }),
            );
        }
    };

    let source = ConsentSource::from_str(&request.source);

    let record = state.consent_manager.record_consent(
        &request.subject_id,
        purpose,
        request.granted,
        source,
        &request.version,
        request.expires_at,
        request.metadata,
        "api", // In production, get from authenticated user
    );

    state.activity.log_write(
        &format!(
            "Consent recorded: subject={}, purpose={}, granted={}",
            request.subject_id, purpose, request.granted
        ),
        None,
    );

    (
        StatusCode::CREATED,
        Json(ConsentResponse {
            success: true,
            record: Some(record),
            error: None,
        }),
    )
}

/// Get consent status for a subject.
/// GET /api/v1/compliance/consent/{subject_id}
pub async fn get_consent_status(
    State(state): State<AppState>,
    Path(subject_id): Path<String>,
) -> Json<ConsentStatusResponse> {
    let consents = state.consent_manager.get_consent(&subject_id);
    let on_dns_list = state.consent_manager.is_on_do_not_sell_list(&subject_id);

    Json(ConsentStatusResponse {
        subject_id,
        consents,
        on_do_not_sell_list: on_dns_list,
    })
}

/// Withdraw consent for a specific purpose.
/// DELETE /api/v1/compliance/consent/{subject_id}/{purpose}
pub async fn withdraw_consent(
    State(state): State<AppState>,
    Path((subject_id, purpose_str)): Path<(String, String)>,
    Json(request): Json<WithdrawConsentRequest>,
) -> impl IntoResponse {
    let purpose = match Purpose::from_str(&purpose_str) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ConsentResponse {
                    success: false,
                    record: None,
                    error: Some(format!("Invalid purpose: {}", purpose_str)),
                }),
            );
        }
    };

    match state.consent_manager.withdraw_consent(
        &subject_id,
        purpose,
        "api", // In production, get from authenticated user
        request.reason.as_deref(),
    ) {
        Ok(record) => {
            state.activity.log_write(
                &format!(
                    "Consent withdrawn: subject={}, purpose={}",
                    subject_id, purpose
                ),
                None,
            );

            (
                StatusCode::OK,
                Json(ConsentResponse {
                    success: true,
                    record: Some(record),
                    error: None,
                }),
            )
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ConsentResponse {
                success: false,
                record: None,
                error: Some(e),
            }),
        ),
    }
}

/// Get consent audit history for a subject.
/// GET /api/v1/compliance/consent/{subject_id}/history
pub async fn get_consent_history(
    State(state): State<AppState>,
    Path(subject_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<Vec<ConsentHistoryEntry>> {
    let history = if let Some(purpose_str) = params.get("purpose") {
        if let Some(purpose) = Purpose::from_str(purpose_str) {
            state.consent_manager.get_history_for_purpose(&subject_id, purpose)
        } else {
            state.consent_manager.get_history(&subject_id)
        }
    } else {
        state.consent_manager.get_history(&subject_id)
    };

    Json(history)
}

/// Check consent for a specific purpose.
/// GET /api/v1/compliance/consent/{subject_id}/check/{purpose}
pub async fn check_consent_status(
    State(state): State<AppState>,
    Path((subject_id, purpose_str)): Path<(String, String)>,
) -> impl IntoResponse {
    let purpose = match Purpose::from_str(&purpose_str) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid purpose: {}", purpose_str)
                })),
            );
        }
    };

    let granted = state.consent_manager.check_consent(&subject_id, purpose);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "subject_id": subject_id,
            "purpose": purpose.to_str(),
            "granted": granted
        })),
    )
}

/// Export all consent data for a subject (GDPR data portability).
/// GET /api/v1/compliance/consent/{subject_id}/export
pub async fn export_consent_data(
    State(state): State<AppState>,
    Path(subject_id): Path<String>,
) -> Json<SubjectConsentExport> {
    let export = state.consent_manager.export_subject_data(&subject_id);
    Json(export)
}

/// Delete all consent data for a subject (GDPR right to erasure).
/// DELETE /api/v1/compliance/consent/{subject_id}
pub async fn delete_consent_data(
    State(state): State<AppState>,
    Path(subject_id): Path<String>,
) -> impl IntoResponse {
    let deleted = state.consent_manager.delete_subject_data(&subject_id, "api");

    state.activity.log(
        crate::activity::ActivityType::Delete,
        &format!("Consent data deleted for subject: {}", subject_id),
    );

    if deleted {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": format!("Consent data deleted for subject: {}", subject_id)
            })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": format!("No consent data found for subject: {}", subject_id)
            })),
        )
    }
}

/// Get consent statistics.
/// GET /api/v1/compliance/consent/stats
pub async fn get_consent_stats(State(state): State<AppState>) -> Json<ConsentStats> {
    Json(state.consent_manager.get_stats())
}

/// Get the CCPA Do Not Sell list.
/// GET /api/v1/compliance/do-not-sell
pub async fn get_do_not_sell_list(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(state.consent_manager.get_do_not_sell_list())
}

// =============================================================================
// Consent Enforcement Helper
// =============================================================================

/// Check consent before performing a data operation.
/// Returns true if the operation is allowed, false otherwise.
///
/// Usage:
/// ```ignore
/// if !check_consent(&state, "user123", Purpose::Analytics) {
///     return Err("Consent not granted for analytics");
/// }
/// // Proceed with analytics operation
/// ```
pub fn check_consent(state: &AppState, subject_id: &str, purpose: Purpose) -> bool {
    state.consent_manager.check_consent(subject_id, purpose)
}

/// Check multiple purposes at once.
/// Returns true only if all purposes have valid consent.
pub fn check_all_consents(state: &AppState, subject_id: &str, purposes: &[Purpose]) -> bool {
    purposes.iter().all(|p| state.consent_manager.check_consent(subject_id, *p))
}

/// Check if any of the purposes have valid consent.
pub fn check_any_consent(state: &AppState, subject_id: &str, purposes: &[Purpose]) -> bool {
    purposes.iter().any(|p| state.consent_manager.check_consent(subject_id, *p))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get current timestamp in milliseconds.
fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Generate a unique consent record ID.
fn generate_consent_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = now_timestamp_ms();
    format!("consent-{}-{:06}", timestamp, count)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consent_record_creation() {
        let record = ConsentRecord::new(
            "user123",
            Purpose::Marketing,
            true,
            ConsentSource::WebForm,
            "1.0",
        );

        assert_eq!(record.subject_id, "user123");
        assert_eq!(record.purpose, Purpose::Marketing);
        assert!(record.granted);
        assert!(record.is_valid());
    }

    #[test]
    fn test_consent_expiration() {
        let mut record = ConsentRecord::new(
            "user123",
            Purpose::Analytics,
            true,
            ConsentSource::Api,
            "1.0",
        );

        // Set expiration to the past
        record.expires_at = Some(1000);
        assert!(record.is_expired());
        assert!(!record.is_valid());
    }

    #[test]
    fn test_consent_manager_basic_operations() {
        let manager = ConsentManager::new();

        // Record consent
        let record = manager.record_consent(
            "user123",
            Purpose::Marketing,
            true,
            ConsentSource::WebForm,
            "1.0",
            None,
            None,
            "test",
        );

        assert!(record.granted);

        // Check consent
        assert!(manager.check_consent("user123", Purpose::Marketing));
        assert!(!manager.check_consent("user123", Purpose::Analytics));
        assert!(!manager.check_consent("user456", Purpose::Marketing));
    }

    #[test]
    fn test_consent_withdrawal() {
        let manager = ConsentManager::new();

        // Grant consent
        manager.record_consent(
            "user123",
            Purpose::Marketing,
            true,
            ConsentSource::WebForm,
            "1.0",
            None,
            None,
            "test",
        );

        assert!(manager.check_consent("user123", Purpose::Marketing));

        // Withdraw consent
        let result = manager.withdraw_consent("user123", Purpose::Marketing, "test", Some("User request"));
        assert!(result.is_ok());
        assert!(!manager.check_consent("user123", Purpose::Marketing));
    }

    #[test]
    fn test_consent_history() {
        let manager = ConsentManager::new();

        // Record multiple consents
        manager.record_consent(
            "user123",
            Purpose::Marketing,
            true,
            ConsentSource::WebForm,
            "1.0",
            None,
            None,
            "test",
        );

        manager.record_consent(
            "user123",
            Purpose::Marketing,
            false,
            ConsentSource::WebForm,
            "1.0",
            None,
            None,
            "test",
        );

        let history = manager.get_history("user123");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].action, ConsentAction::Grant);
        assert_eq!(history[1].action, ConsentAction::Withdraw);
    }

    #[test]
    fn test_do_not_sell_list() {
        let manager = ConsentManager::new();

        // Opt into Do Not Sell
        manager.record_consent(
            "user123",
            Purpose::DoNotSell,
            true,
            ConsentSource::WebForm,
            "1.0",
            None,
            None,
            "test",
        );

        assert!(manager.is_on_do_not_sell_list("user123"));
        assert!(!manager.is_on_do_not_sell_list("user456"));

        // Opt out of Do Not Sell
        manager.record_consent(
            "user123",
            Purpose::DoNotSell,
            false,
            ConsentSource::WebForm,
            "1.0",
            None,
            None,
            "test",
        );

        assert!(!manager.is_on_do_not_sell_list("user123"));
    }

    #[test]
    fn test_purpose_parsing() {
        assert_eq!(Purpose::from_str("marketing"), Some(Purpose::Marketing));
        assert_eq!(Purpose::from_str("ANALYTICS"), Some(Purpose::Analytics));
        assert_eq!(Purpose::from_str("third_party_sharing"), Some(Purpose::ThirdPartySharing));
        assert_eq!(Purpose::from_str("do_not_sell"), Some(Purpose::DoNotSell));
        assert_eq!(Purpose::from_str("custom:42"), Some(Purpose::Custom(42)));
        assert_eq!(Purpose::from_str("unknown"), None);
    }

    #[test]
    fn test_consent_stats() {
        let manager = ConsentManager::new();

        manager.record_consent("user1", Purpose::Marketing, true, ConsentSource::Api, "1.0", None, None, "test");
        manager.record_consent("user1", Purpose::Analytics, false, ConsentSource::Api, "1.0", None, None, "test");
        manager.record_consent("user2", Purpose::Marketing, true, ConsentSource::Api, "1.0", None, None, "test");

        let stats = manager.get_stats();
        assert_eq!(stats.total_subjects, 2);
        assert_eq!(stats.total_records, 3);

        let marketing_stats = stats.consents_by_purpose.get("marketing").unwrap();
        assert_eq!(marketing_stats.0, 2); // 2 granted
        assert_eq!(marketing_stats.1, 0); // 0 denied
    }

    #[test]
    fn test_subject_data_export() {
        let manager = ConsentManager::new();

        manager.record_consent("user123", Purpose::Marketing, true, ConsentSource::Api, "1.0", None, None, "test");
        manager.record_consent("user123", Purpose::Analytics, true, ConsentSource::Api, "1.0", None, None, "test");

        let export = manager.export_subject_data("user123");
        assert_eq!(export.subject_id, "user123");
        assert_eq!(export.current_consents.len(), 2);
        assert_eq!(export.consent_history.len(), 2);
    }

    #[test]
    fn test_subject_data_deletion() {
        let manager = ConsentManager::new();

        manager.record_consent("user123", Purpose::Marketing, true, ConsentSource::Api, "1.0", None, None, "test");

        assert!(manager.delete_subject_data("user123", "test"));
        assert!(manager.get_consent("user123").is_empty());
        assert!(manager.get_history("user123").is_empty());

        // Deleting non-existent data should return false
        assert!(!manager.delete_subject_data("user123", "test"));
    }
}
