//! Aegis Server - API Gateway
//!
//! Multi-protocol API server supporting REST, GraphQL, WebSocket, and gRPC.
//! Handles authentication, authorization, rate limiting, and request routing.
//!
//! Key Features:
//! - REST API with OpenAPI documentation
//! - GraphQL endpoint with subscriptions
//! - WebSocket for real-time streaming
//! - JWT and API key authentication
//! - Admin API for web dashboard
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod activity;
pub mod admin;
pub mod auth;
pub mod backup;
pub mod breach;
pub mod config;
pub mod consent;
pub mod gdpr;
pub mod handlers;
pub mod middleware;
pub mod router;
pub mod secrets;
pub mod state;

pub use activity::{Activity, ActivityLogger, ActivityType};
pub use admin::{AdminService, ClusterInfo, DashboardSummary, NodeInfo, QueryStats};
pub use auth::{
    AuthProvider, AuthResponse, AuthService, AuditEntry, AuditEventType, AuditLogger,
    AuditResult, LdapAuthenticator, LdapConfig, LoginRequest, MfaVerifyRequest,
    OAuth2Authenticator, OAuth2Config, Permission, RbacManager, Role, RowLevelPolicy,
    RowPolicyOperation, UserInfo, UserRole,
};
pub use backup::{BackupInfo, BackupManager, BackupStatus};
pub use breach::{
    BreachDetector, BreachIncident, BreachNotifier, BreachSeverity, BreachStats, DetectionConfig,
    IncidentReport, IncidentStatus, LogNotifier, SecurityEvent, SecurityEventType, WebhookNotifier,
};
pub use config::{ClusterTlsConfig, ServerConfig};
pub use consent::{
    check_consent, check_all_consents, check_any_consent, ConsentAction, ConsentHistoryEntry,
    ConsentManager, ConsentRecord, ConsentSource, ConsentStats, Purpose, SubjectConsentExport,
};
pub use gdpr::{
    DeletionAuditEntry, DeletionAuditLog, DeletionCertificate, DeletionEventType, DeletionRequest,
    DeletionResponse, DeletionScope, DeletedItem, GdprService,
};
pub use router::create_router;
pub use state::AppState;
