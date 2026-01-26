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
pub mod config;
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
pub use config::ServerConfig;
pub use router::create_router;
pub use state::AppState;
