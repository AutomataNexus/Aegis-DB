//! Aegis Authentication Module
//!
//! Enterprise authentication with LDAP, OAuth2/OIDC, RBAC, and audit logging.
//!
//! @version 0.2.0
//! @author AutomataNexus Development Team

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// =============================================================================
// Authentication Provider Types
// =============================================================================

/// Authentication provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    Local,
    Ldap,
    OAuth2,
    Oidc,
    Saml,
}

/// LDAP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapConfig {
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub base_dn: String,
    pub user_filter: String,
    pub group_filter: String,
    pub use_tls: bool,
    pub group_attribute: String,
    pub admin_groups: Vec<String>,
    pub operator_groups: Vec<String>,
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            server_url: "ldap://localhost:389".to_string(),
            bind_dn: "cn=admin,dc=aegisdb,dc=io".to_string(),
            bind_password: String::new(),
            base_dn: "dc=aegisdb,dc=io".to_string(),
            user_filter: "(uid={username})".to_string(),
            group_filter: "(member={dn})".to_string(),
            use_tls: true,
            group_attribute: "memberOf".to_string(),
            admin_groups: vec!["cn=admins,ou=groups,dc=aegisdb,dc=io".to_string()],
            operator_groups: vec!["cn=operators,ou=groups,dc=aegisdb,dc=io".to_string()],
        }
    }
}

/// OAuth2/OIDC configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Config {
    pub provider_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub role_claim: String,
    pub admin_roles: Vec<String>,
    pub operator_roles: Vec<String>,
}

impl Default for OAuth2Config {
    fn default() -> Self {
        Self {
            provider_name: "default".to_string(),
            client_id: String::new(),
            client_secret: String::new(),
            authorization_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            userinfo_url: "https://auth.example.com/userinfo".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            scopes: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
            role_claim: "roles".to_string(),
            admin_roles: vec!["admin".to_string()],
            operator_roles: vec!["operator".to_string()],
        }
    }
}

/// LDAP authentication result.
#[derive(Debug, Clone)]
pub struct LdapAuthResult {
    pub success: bool,
    pub user_dn: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub groups: Vec<String>,
    pub error: Option<String>,
}

/// OAuth2 token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
}

/// OAuth2 user info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2UserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub roles: Option<Vec<String>>,
}

// =============================================================================
// User Types
// =============================================================================

/// User role enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Operator,
    Viewer,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Operator => write!(f, "operator"),
            UserRole::Viewer => write!(f, "viewer"),
        }
    }
}

/// User information stored in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
    pub mfa_enabled: bool,
    pub mfa_secret: Option<String>,
    pub created_at: u64,
    pub last_login: Option<u64>,
}

/// User information returned to clients (no sensitive data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: UserRole,
    pub mfa_enabled: bool,
    pub created_at: String,
}

impl From<&User> for UserInfo {
    fn from(user: &User) -> Self {
        Self {
            id: user.id.clone(),
            username: user.username.clone(),
            email: user.email.clone(),
            role: user.role,
            mfa_enabled: user.mfa_enabled,
            created_at: format_timestamp(user.created_at),
        }
    }
}

// =============================================================================
// Session Types
// =============================================================================

/// Active session information.
#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub mfa_verified: bool,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Pending MFA verification session.
#[derive(Debug, Clone)]
pub struct PendingMfaSession {
    pub temp_token: String,
    pub user_id: String,
    pub created_at: Instant,
    pub expires_at: Instant,
}

impl PendingMfaSession {
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

// =============================================================================
// Request/Response Types
// =============================================================================

/// Login request.
#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// MFA verification request.
#[derive(Debug, Clone, Deserialize)]
pub struct MfaVerifyRequest {
    pub code: String,
    pub token: String,
}

/// MFA setup data for new MFA enrollment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSetupData {
    pub secret: String,
    pub qr_code: String,
    pub backup_codes: Vec<String>,
}

/// Authentication response.
#[derive(Debug, Clone, Serialize)]
pub struct AuthResponse {
    pub token: Option<String>,
    pub user: Option<UserInfo>,
    pub requires_mfa: Option<bool>,
    pub requires_mfa_setup: Option<bool>,
    pub mfa_setup_data: Option<MfaSetupData>,
    pub error: Option<String>,
}

impl AuthResponse {
    pub fn success(token: String, user: UserInfo) -> Self {
        Self {
            token: Some(token),
            user: Some(user),
            requires_mfa: None,
            requires_mfa_setup: None,
            mfa_setup_data: None,
            error: None,
        }
    }

    pub fn requires_mfa(temp_token: String) -> Self {
        Self {
            token: Some(temp_token),
            user: None,
            requires_mfa: Some(true),
            requires_mfa_setup: None,
            mfa_setup_data: None,
            error: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            token: None,
            user: None,
            requires_mfa: None,
            requires_mfa_setup: None,
            mfa_setup_data: None,
            error: Some(message.to_string()),
        }
    }
}

// =============================================================================
// Authentication Service
// =============================================================================

/// Authentication service managing users and sessions.
pub struct AuthService {
    users: RwLock<HashMap<String, User>>,
    sessions: RwLock<HashMap<String, Session>>,
    pending_mfa: RwLock<HashMap<String, PendingMfaSession>>,
    session_duration: Duration,
    mfa_timeout: Duration,
}

impl AuthService {
    /// Create a new authentication service.
    ///
    /// If AEGIS_ADMIN_USERNAME and AEGIS_ADMIN_PASSWORD environment variables
    /// are set, an initial admin user will be created. Otherwise, the system
    /// starts with no users and the first user must be created via the API
    /// or CLI.
    pub fn new() -> Self {
        let mut users = HashMap::new();
        let mut user_count = 0;

        // Check for initial admin user from environment variables
        if let (Ok(username), Ok(password)) = (
            std::env::var("AEGIS_ADMIN_USERNAME"),
            std::env::var("AEGIS_ADMIN_PASSWORD"),
        ) {
            // Validate password meets security requirements
            if password.len() >= 12 {
                let email = std::env::var("AEGIS_ADMIN_EMAIL")
                    .unwrap_or_else(|_| format!("{}@localhost", username));

                user_count += 1;
                let admin = User {
                    id: format!("user-{:03}", user_count),
                    username: username.clone(),
                    email,
                    password_hash: hash_password(&password),
                    role: UserRole::Admin,
                    mfa_enabled: false,
                    mfa_secret: None,
                    created_at: now_timestamp(),
                    last_login: None,
                };
                tracing::info!("Created initial admin user '{}' from environment", username);
                users.insert(admin.username.clone(), admin);
            } else {
                tracing::warn!(
                    "AEGIS_ADMIN_PASSWORD must be at least 12 characters. Initial admin user not created."
                );
            }
        } else {
            tracing::info!(
                "No initial admin configured. Set AEGIS_ADMIN_USERNAME and AEGIS_ADMIN_PASSWORD to create one."
            );
        }

        Self {
            users: RwLock::new(users),
            sessions: RwLock::new(HashMap::new()),
            pending_mfa: RwLock::new(HashMap::new()),
            session_duration: Duration::from_secs(24 * 60 * 60), // 24 hours
            mfa_timeout: Duration::from_secs(5 * 60), // 5 minutes
        }
    }

    /// Authenticate user with username and password.
    pub fn login(&self, username: &str, password: &str) -> AuthResponse {
        let users = self.users.read();

        let user = match users.get(username) {
            Some(u) => u,
            None => return AuthResponse::error("Invalid credentials"),
        };

        if !verify_password(password, &user.password_hash) {
            return AuthResponse::error("Invalid credentials");
        }

        // Update last login
        drop(users);
        {
            let mut users = self.users.write();
            if let Some(u) = users.get_mut(username) {
                u.last_login = Some(now_timestamp());
            }
        }
        let users = self.users.read();
        let user = users.get(username).expect("user should exist after successful credential check");

        if user.mfa_enabled {
            // Create pending MFA session
            let temp_token = generate_token();
            let pending = PendingMfaSession {
                temp_token: temp_token.clone(),
                user_id: user.id.clone(),
                created_at: Instant::now(),
                expires_at: Instant::now() + self.mfa_timeout,
            };
            self.pending_mfa.write().insert(temp_token.clone(), pending);

            AuthResponse::requires_mfa(temp_token)
        } else {
            // Create session directly
            let token = generate_token();
            let session = Session {
                token: token.clone(),
                user_id: user.id.clone(),
                created_at: Instant::now(),
                expires_at: Instant::now() + self.session_duration,
                mfa_verified: true,
            };
            self.sessions.write().insert(token.clone(), session);

            AuthResponse::success(token, UserInfo::from(user))
        }
    }

    /// Verify MFA code and complete authentication.
    pub fn verify_mfa(&self, code: &str, temp_token: &str) -> AuthResponse {
        // Get pending MFA session
        let pending = {
            let pending_sessions = self.pending_mfa.read();
            match pending_sessions.get(temp_token) {
                Some(p) if !p.is_expired() => p.clone(),
                Some(_) => return AuthResponse::error("MFA session expired"),
                None => return AuthResponse::error("Invalid MFA session"),
            }
        };

        // Get user
        let users = self.users.read();
        let user = match users.values().find(|u| u.id == pending.user_id) {
            Some(u) => u,
            None => return AuthResponse::error("User not found"),
        };

        // Verify TOTP code
        let secret = match &user.mfa_secret {
            Some(s) => s,
            None => return AuthResponse::error("MFA not configured"),
        };

        if !verify_totp(code, secret) {
            return AuthResponse::error("Invalid MFA code");
        }

        // Remove pending session
        self.pending_mfa.write().remove(temp_token);

        // Create authenticated session
        let token = generate_token();
        let session = Session {
            token: token.clone(),
            user_id: user.id.clone(),
            created_at: Instant::now(),
            expires_at: Instant::now() + self.session_duration,
            mfa_verified: true,
        };
        self.sessions.write().insert(token.clone(), session);

        AuthResponse::success(token, UserInfo::from(user))
    }

    /// Validate a session token and return user info.
    pub fn validate_session(&self, token: &str) -> Option<UserInfo> {
        let sessions = self.sessions.read();
        let session = sessions.get(token)?;

        if session.is_expired() {
            return None;
        }

        let users = self.users.read();
        let user = users.values().find(|u| u.id == session.user_id)?;

        Some(UserInfo::from(user))
    }

    /// Logout and invalidate session.
    pub fn logout(&self, token: &str) -> bool {
        self.sessions.write().remove(token).is_some()
    }

    /// Get user by ID.
    pub fn get_user(&self, user_id: &str) -> Option<UserInfo> {
        let users = self.users.read();
        users.values()
            .find(|u| u.id == user_id)
            .map(UserInfo::from)
    }

    /// List all users.
    pub fn list_users(&self) -> Vec<UserInfo> {
        let users = self.users.read();
        users.values().map(UserInfo::from).collect()
    }

    /// Create a new user.
    pub fn create_user(&self, username: &str, email: &str, password: &str, role: &str) -> Result<UserInfo, String> {
        let mut users = self.users.write();

        if users.contains_key(username) {
            return Err(format!("User '{}' already exists", username));
        }

        // Validate password strength
        if password.len() < 8 {
            return Err("Password must be at least 8 characters".to_string());
        }

        // Validate username (alphanumeric and underscore only)
        if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err("Username must contain only alphanumeric characters and underscores".to_string());
        }

        // Basic email validation
        if !email.contains('@') || !email.contains('.') {
            return Err("Invalid email format".to_string());
        }

        let user_role = match role.to_lowercase().as_str() {
            "admin" => UserRole::Admin,
            "operator" => UserRole::Operator,
            "viewer" | _ => UserRole::Viewer,
        };

        let id = format!("user-{:03}", users.len() + 1);
        let user = User {
            id: id.clone(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: hash_password(password),
            role: user_role,
            mfa_enabled: false,
            mfa_secret: None,
            created_at: now_timestamp(),
            last_login: None,
        };

        let user_info = UserInfo::from(&user);
        users.insert(username.to_string(), user);

        // Note: Role assignment should be handled by RbacManager separately
        tracing::info!("Created user '{}' with role '{}'", username, role);

        Ok(user_info)
    }

    /// Update an existing user.
    pub fn update_user(&self, username: &str, email: Option<String>, role: Option<String>, password: Option<String>) -> Result<UserInfo, String> {
        let mut users = self.users.write();

        let user = users.get_mut(username)
            .ok_or_else(|| format!("User '{}' not found", username))?;

        if let Some(new_email) = email {
            user.email = new_email;
        }

        if let Some(new_role) = role {
            user.role = match new_role.to_lowercase().as_str() {
                "admin" => UserRole::Admin,
                "operator" => UserRole::Operator,
                "viewer" | _ => UserRole::Viewer,
            };
        }

        if let Some(new_password) = password {
            user.password_hash = hash_password(&new_password);
        }

        Ok(UserInfo::from(user as &User))
    }

    /// Delete a user.
    pub fn delete_user(&self, username: &str) -> Result<(), String> {
        let mut users = self.users.write();

        if !users.contains_key(username) {
            return Err(format!("User '{}' not found", username));
        }

        // Prevent deleting the admin user
        if username == "admin" {
            return Err("Cannot delete the admin user".to_string());
        }

        users.remove(username);
        Ok(())
    }

    /// Enable MFA for a user and return the generated secret.
    /// The secret should be stored by the user in their authenticator app.
    pub fn enable_mfa(&self, username: &str) -> Result<String, String> {
        let mut users = self.users.write();

        let user = users.get_mut(username)
            .ok_or_else(|| format!("User '{}' not found", username))?;

        if user.mfa_enabled {
            return Err("MFA is already enabled for this user".to_string());
        }

        // Generate a new MFA secret
        let secret = generate_mfa_secret();
        user.mfa_secret = Some(secret.clone());
        user.mfa_enabled = true;

        tracing::info!("Enabled MFA for user '{}'", username);

        Ok(secret)
    }

    /// Disable MFA for a user.
    pub fn disable_mfa(&self, username: &str) -> Result<(), String> {
        let mut users = self.users.write();

        let user = users.get_mut(username)
            .ok_or_else(|| format!("User '{}' not found", username))?;

        if !user.mfa_enabled {
            return Err("MFA is not enabled for this user".to_string());
        }

        user.mfa_secret = None;
        user.mfa_enabled = false;

        tracing::info!("Disabled MFA for user '{}'", username);

        Ok(())
    }

    /// Clean up expired sessions.
    pub fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write();
        sessions.retain(|_, s| !s.is_expired());

        let mut pending = self.pending_mfa.write();
        pending.retain(|_, p| !p.is_expired());
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Generate a cryptographically secure random token (256-bit).
fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(&bytes)
}

/// Generate a cryptographically secure random token using hex encoding.
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut result = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            result.push(HEX_CHARS[(byte >> 4) as usize] as char);
            result.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
        }
        result
    }
}

/// Hash a password using Argon2id with a random salt.
fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string()
}

/// Verify a password against its Argon2 hash.
fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Generate a cryptographically secure MFA secret (Base32 encoded).
fn generate_mfa_secret() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 20] = rng.gen(); // 160 bits for TOTP

    // Base32 encode (RFC 4648)
    const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut result = String::with_capacity(32);

    for chunk in bytes.chunks(5) {
        let mut buffer = [0u8; 5];
        buffer[..chunk.len()].copy_from_slice(chunk);

        result.push(BASE32_ALPHABET[(buffer[0] >> 3) as usize] as char);
        result.push(BASE32_ALPHABET[((buffer[0] & 0x07) << 2 | buffer[1] >> 6) as usize] as char);
        result.push(BASE32_ALPHABET[((buffer[1] & 0x3E) >> 1) as usize] as char);
        result.push(BASE32_ALPHABET[((buffer[1] & 0x01) << 4 | buffer[2] >> 4) as usize] as char);
        result.push(BASE32_ALPHABET[((buffer[2] & 0x0F) << 1 | buffer[3] >> 7) as usize] as char);
        result.push(BASE32_ALPHABET[((buffer[3] & 0x7C) >> 2) as usize] as char);
        result.push(BASE32_ALPHABET[((buffer[3] & 0x03) << 3 | buffer[4] >> 5) as usize] as char);
        result.push(BASE32_ALPHABET[(buffer[4] & 0x1F) as usize] as char);
    }

    result
}

/// Verify a TOTP code using RFC 6238 algorithm.
fn verify_totp(code: &str, secret: &str) -> bool {
    use data_encoding::BASE32_NOPAD;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    // Decode the base32 secret
    let secret_bytes = match BASE32_NOPAD.decode(secret.to_uppercase().as_bytes()) {
        Ok(bytes) => bytes,
        Err(_) => {
            // Try with padding variations
            let padded = format!("{}{}", secret.to_uppercase(), &"========"[..((8 - secret.len() % 8) % 8)]);
            match data_encoding::BASE32.decode(padded.as_bytes()) {
                Ok(bytes) => bytes,
                Err(_) => return false,
            }
        }
    };

    // Get current time step (30-second windows)
    let timestamp = now_timestamp() / 1000; // seconds
    let time_step = timestamp / 30;

    // Check current time step and one before/after for clock drift tolerance
    for offset in [-1i64, 0, 1] {
        let counter = (time_step as i64 + offset) as u64;
        let counter_bytes = counter.to_be_bytes();

        // Compute HMAC-SHA1
        let mut mac = match Hmac::<Sha1>::new_from_slice(&secret_bytes) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(&counter_bytes);
        let result = mac.finalize().into_bytes();

        // Dynamic truncation (RFC 4226)
        let offset_idx = (result[19] & 0x0f) as usize;
        let binary_code = ((result[offset_idx] & 0x7f) as u32) << 24
            | (result[offset_idx + 1] as u32) << 16
            | (result[offset_idx + 2] as u32) << 8
            | (result[offset_idx + 3] as u32);

        // Generate 6-digit code
        let otp = binary_code % 1_000_000;
        let expected = format!("{:06}", otp);

        if code == expected {
            return true;
        }
    }

    false
}

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

    // Simple ISO 8601 formatting
    let duration = datetime.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = duration.as_secs();

    let days_since_epoch = total_secs / 86400;
    let secs_today = total_secs % 86400;

    let hours = secs_today / 3600;
    let minutes = (secs_today % 3600) / 60;
    let seconds = secs_today % 60;

    // Calculate year/month/day (simplified)
    let mut year = 1970;
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

    let mut month = 1;
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

// =============================================================================
// RBAC - Role-Based Access Control
// =============================================================================

/// Permission types for database operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // Database operations
    DatabaseCreate,
    DatabaseDrop,
    DatabaseList,

    // Table operations
    TableCreate,
    TableDrop,
    TableAlter,
    TableList,

    // Data operations
    DataSelect,
    DataInsert,
    DataUpdate,
    DataDelete,

    // Admin operations
    UserCreate,
    UserDelete,
    UserModify,
    RoleCreate,
    RoleDelete,
    RoleAssign,

    // System operations
    ConfigView,
    ConfigModify,
    MetricsView,
    LogsView,
    BackupCreate,
    BackupRestore,

    // Cluster operations
    NodeAdd,
    NodeRemove,
    ClusterManage,
}

/// A role with a set of permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub description: String,
    pub permissions: HashSet<Permission>,
    pub created_at: u64,
    pub created_by: String,
}

impl Role {
    /// Create a new role with the given permissions.
    pub fn new(name: &str, description: &str, permissions: Vec<Permission>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            permissions: permissions.into_iter().collect(),
            created_at: now_timestamp(),
            created_by: "system".to_string(),
        }
    }

    /// Check if this role has a specific permission.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// RBAC manager for role and permission management.
pub struct RbacManager {
    roles: RwLock<HashMap<String, Role>>,
    user_roles: RwLock<HashMap<String, HashSet<String>>>,
    row_policies: RwLock<Vec<RowLevelPolicy>>,
}

impl RbacManager {
    /// Create a new RBAC manager with default roles.
    pub fn new() -> Self {
        let mut roles = HashMap::new();

        // Create admin role with all permissions
        let admin_permissions = vec![
            Permission::DatabaseCreate, Permission::DatabaseDrop, Permission::DatabaseList,
            Permission::TableCreate, Permission::TableDrop, Permission::TableAlter, Permission::TableList,
            Permission::DataSelect, Permission::DataInsert, Permission::DataUpdate, Permission::DataDelete,
            Permission::UserCreate, Permission::UserDelete, Permission::UserModify,
            Permission::RoleCreate, Permission::RoleDelete, Permission::RoleAssign,
            Permission::ConfigView, Permission::ConfigModify, Permission::MetricsView, Permission::LogsView,
            Permission::BackupCreate, Permission::BackupRestore,
            Permission::NodeAdd, Permission::NodeRemove, Permission::ClusterManage,
        ];
        roles.insert("admin".to_string(), Role::new("admin", "Full system administrator", admin_permissions));

        // Create operator role
        let operator_permissions = vec![
            Permission::DatabaseList,
            Permission::TableCreate, Permission::TableAlter, Permission::TableList,
            Permission::DataSelect, Permission::DataInsert, Permission::DataUpdate, Permission::DataDelete,
            Permission::ConfigView, Permission::MetricsView, Permission::LogsView,
            Permission::BackupCreate,
        ];
        roles.insert("operator".to_string(), Role::new("operator", "Database operator", operator_permissions));

        // Create viewer role
        let viewer_permissions = vec![
            Permission::DatabaseList,
            Permission::TableList,
            Permission::DataSelect,
            Permission::MetricsView,
        ];
        roles.insert("viewer".to_string(), Role::new("viewer", "Read-only viewer", viewer_permissions));

        // Create analyst role
        let analyst_permissions = vec![
            Permission::DatabaseList,
            Permission::TableList,
            Permission::DataSelect,
            Permission::MetricsView,
            Permission::LogsView,
        ];
        roles.insert("analyst".to_string(), Role::new("analyst", "Data analyst with read access", analyst_permissions));

        // User-role mappings start empty - assigned when users are created
        let user_roles: HashMap<String, HashSet<String>> = HashMap::new();

        Self {
            roles: RwLock::new(roles),
            user_roles: RwLock::new(user_roles),
            row_policies: RwLock::new(Vec::new()),
        }
    }

    /// Create a new role.
    pub fn create_role(&self, name: &str, description: &str, permissions: Vec<Permission>, created_by: &str) -> Result<(), String> {
        let mut roles = self.roles.write();
        if roles.contains_key(name) {
            return Err(format!("Role '{}' already exists", name));
        }

        let mut role = Role::new(name, description, permissions);
        role.created_by = created_by.to_string();
        roles.insert(name.to_string(), role);
        Ok(())
    }

    /// Delete a role.
    pub fn delete_role(&self, name: &str) -> Result<(), String> {
        let mut roles = self.roles.write();
        if !roles.contains_key(name) {
            return Err(format!("Role '{}' not found", name));
        }
        if name == "admin" || name == "operator" || name == "viewer" {
            return Err("Cannot delete built-in roles".to_string());
        }
        roles.remove(name);
        Ok(())
    }

    /// List all roles.
    pub fn list_roles(&self) -> Vec<Role> {
        self.roles.read().values().cloned().collect()
    }

    /// Get a specific role.
    pub fn get_role(&self, name: &str) -> Option<Role> {
        self.roles.read().get(name).cloned()
    }

    /// Assign a role to a user.
    pub fn assign_role(&self, user_id: &str, role_name: &str) -> Result<(), String> {
        if !self.roles.read().contains_key(role_name) {
            return Err(format!("Role '{}' not found", role_name));
        }

        let mut user_roles = self.user_roles.write();
        user_roles
            .entry(user_id.to_string())
            .or_default()
            .insert(role_name.to_string());
        Ok(())
    }

    /// Revoke a role from a user.
    pub fn revoke_role(&self, user_id: &str, role_name: &str) -> Result<(), String> {
        let mut user_roles = self.user_roles.write();
        if let Some(roles) = user_roles.get_mut(user_id) {
            roles.remove(role_name);
            Ok(())
        } else {
            Err(format!("User '{}' has no roles assigned", user_id))
        }
    }

    /// Get all roles for a user.
    pub fn get_user_roles(&self, user_id: &str) -> Vec<String> {
        self.user_roles
            .read()
            .get(user_id)
            .map(|r| r.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if a user has a specific permission.
    pub fn check_permission(&self, user_id: &str, permission: Permission) -> bool {
        let user_roles = self.user_roles.read();
        let roles = self.roles.read();

        if let Some(user_role_names) = user_roles.get(user_id) {
            for role_name in user_role_names {
                if let Some(role) = roles.get(role_name) {
                    if role.has_permission(permission) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get all permissions for a user.
    pub fn get_user_permissions(&self, user_id: &str) -> HashSet<Permission> {
        let mut permissions = HashSet::new();
        let user_roles = self.user_roles.read();
        let roles = self.roles.read();

        if let Some(user_role_names) = user_roles.get(user_id) {
            for role_name in user_role_names {
                if let Some(role) = roles.get(role_name) {
                    permissions.extend(role.permissions.iter().cloned());
                }
            }
        }
        permissions
    }

    /// Add a row-level security policy.
    pub fn add_row_policy(&self, policy: RowLevelPolicy) {
        self.row_policies.write().push(policy);
    }

    /// Get row-level policies for a table.
    pub fn get_row_policies(&self, table: &str, user_id: &str) -> Vec<RowLevelPolicy> {
        self.row_policies
            .read()
            .iter()
            .filter(|p| p.table == table && (p.applies_to.is_empty() || p.applies_to.contains(&user_id.to_string())))
            .cloned()
            .collect()
    }
}

impl Default for RbacManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Row-Level Security
// =============================================================================

/// Row-level security policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowLevelPolicy {
    pub name: String,
    pub table: String,
    pub operation: RowPolicyOperation,
    pub condition: String,
    pub applies_to: Vec<String>,
    pub enabled: bool,
}

/// Operations that row policies apply to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowPolicyOperation {
    Select,
    Insert,
    Update,
    Delete,
    All,
}

// =============================================================================
// Audit Logging
// =============================================================================

/// Audit event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    // Authentication events
    LoginSuccess,
    LoginFailure,
    Logout,
    MfaVerified,
    MfaFailed,
    SessionExpired,

    // Authorization events
    PermissionGranted,
    PermissionDenied,
    RoleAssigned,
    RoleRevoked,

    // Data events
    DataRead,
    DataWrite,
    DataDelete,
    SchemaChange,

    // Admin events
    UserCreated,
    UserDeleted,
    UserModified,
    ConfigChanged,
    BackupCreated,
    BackupRestored,

    // System events
    ServiceStarted,
    ServiceStopped,
    NodeJoined,
    NodeLeft,
}

/// An audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: u64,
    pub event_type: AuditEventType,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub ip_address: Option<String>,
    pub resource: Option<String>,
    pub action: String,
    pub result: AuditResult,
    pub details: HashMap<String, String>,
}

/// Result of an audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success,
    Failure,
    Denied,
}

/// Audit logger for compliance and security tracking.
pub struct AuditLogger {
    entries: RwLock<Vec<AuditEntry>>,
    max_entries: usize,
    entry_counter: RwLock<u64>,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(Vec::with_capacity(max_entries)),
            max_entries,
            entry_counter: RwLock::new(0),
        }
    }

    /// Log an audit event.
    pub fn log(&self, event_type: AuditEventType, user_id: Option<&str>, username: Option<&str>,
               ip_address: Option<&str>, resource: Option<&str>, action: &str,
               result: AuditResult, details: HashMap<String, String>) {
        let mut counter = self.entry_counter.write();
        *counter += 1;
        let id = format!("audit-{:012}", *counter);

        let entry = AuditEntry {
            id,
            timestamp: now_timestamp(),
            event_type,
            user_id: user_id.map(String::from),
            username: username.map(String::from),
            ip_address: ip_address.map(String::from),
            resource: resource.map(String::from),
            action: action.to_string(),
            result,
            details,
        };

        let mut entries = self.entries.write();
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(entry);
    }

    /// Log a login success.
    pub fn log_login_success(&self, user_id: &str, username: &str, ip: Option<&str>) {
        self.log(
            AuditEventType::LoginSuccess,
            Some(user_id),
            Some(username),
            ip,
            None,
            "User logged in",
            AuditResult::Success,
            HashMap::new(),
        );
    }

    /// Log a login failure.
    pub fn log_login_failure(&self, username: &str, ip: Option<&str>, reason: &str) {
        let mut details = HashMap::new();
        details.insert("reason".to_string(), reason.to_string());
        self.log(
            AuditEventType::LoginFailure,
            None,
            Some(username),
            ip,
            None,
            "Login attempt failed",
            AuditResult::Failure,
            details,
        );
    }

    /// Log a permission denial.
    pub fn log_permission_denied(&self, user_id: &str, username: &str, resource: &str, permission: &str) {
        let mut details = HashMap::new();
        details.insert("permission".to_string(), permission.to_string());
        self.log(
            AuditEventType::PermissionDenied,
            Some(user_id),
            Some(username),
            None,
            Some(resource),
            "Permission denied",
            AuditResult::Denied,
            details,
        );
    }

    /// Log a data access.
    pub fn log_data_access(&self, user_id: &str, username: &str, table: &str, operation: &str, rows_affected: u64) {
        let mut details = HashMap::new();
        details.insert("rows_affected".to_string(), rows_affected.to_string());
        let event_type = match operation {
            "SELECT" => AuditEventType::DataRead,
            "INSERT" | "UPDATE" => AuditEventType::DataWrite,
            "DELETE" => AuditEventType::DataDelete,
            _ => AuditEventType::DataRead,
        };
        self.log(
            event_type,
            Some(user_id),
            Some(username),
            None,
            Some(table),
            &format!("{} on {}", operation, table),
            AuditResult::Success,
            details,
        );
    }

    /// Log a schema change.
    pub fn log_schema_change(&self, user_id: &str, username: &str, object: &str, action: &str) {
        let mut details = HashMap::new();
        details.insert("action".to_string(), action.to_string());
        self.log(
            AuditEventType::SchemaChange,
            Some(user_id),
            Some(username),
            None,
            Some(object),
            &format!("Schema change: {} on {}", action, object),
            AuditResult::Success,
            details,
        );
    }

    /// Get recent audit entries.
    pub fn get_entries(&self, limit: usize, offset: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        let start = entries.len().saturating_sub(limit + offset);
        let end = entries.len().saturating_sub(offset);
        entries[start..end].iter().rev().cloned().collect()
    }

    /// Get entries by event type.
    pub fn get_entries_by_type(&self, event_type: AuditEventType, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.event_type == event_type)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get entries for a specific user.
    pub fn get_entries_for_user(&self, user_id: &str, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.user_id.as_deref() == Some(user_id))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get failed login attempts.
    pub fn get_failed_logins(&self, since: u64) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| e.event_type == AuditEventType::LoginFailure && e.timestamp >= since)
            .cloned()
            .collect()
    }

    /// Count entries.
    pub fn count(&self) -> usize {
        self.entries.read().len()
    }

    /// Export entries for compliance reporting.
    pub fn export(&self, start_time: u64, end_time: u64) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| e.timestamp >= start_time && e.timestamp <= end_time)
            .cloned()
            .collect()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(100_000) // Keep 100k audit entries by default
    }
}

// =============================================================================
// LDAP Authentication
// =============================================================================

/// LDAP authenticator for enterprise directory integration.
pub struct LdapAuthenticator {
    config: LdapConfig,
}

impl LdapAuthenticator {
    /// Create a new LDAP authenticator.
    pub fn new(config: LdapConfig) -> Self {
        Self { config }
    }

    /// Authenticate user against LDAP using the ldap3 crate.
    pub fn authenticate(&self, username: &str, password: &str) -> LdapAuthResult {
        // Validate configuration
        if self.config.server_url.is_empty() {
            return LdapAuthResult {
                success: false,
                user_dn: None,
                email: None,
                display_name: None,
                groups: vec![],
                error: Some("LDAP server URL not configured".to_string()),
            };
        }

        if password.is_empty() {
            return LdapAuthResult {
                success: false,
                user_dn: None,
                email: None,
                display_name: None,
                groups: vec![],
                error: Some("Password is required".to_string()),
            };
        }

        // Use a runtime for blocking LDAP operations
        let runtime = match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle,
            Err(_) => {
                return LdapAuthResult {
                    success: false,
                    user_dn: None,
                    email: None,
                    display_name: None,
                    groups: vec![],
                    error: Some("No async runtime available".to_string()),
                };
            }
        };

        let config = self.config.clone();
        let username = username.to_string();
        let password = password.to_string();

        // Perform LDAP operations
        let result = runtime.block_on(async move {
            Self::authenticate_async(&config, &username, &password).await
        });

        result
    }

    /// Async LDAP authentication implementation.
    async fn authenticate_async(config: &LdapConfig, username: &str, password: &str) -> LdapAuthResult {
        use ldap3::{LdapConnAsync, Scope, SearchEntry};

        // Connect to LDAP server
        let (conn, mut ldap) = match LdapConnAsync::new(&config.server_url).await {
            Ok(c) => c,
            Err(e) => {
                return LdapAuthResult {
                    success: false,
                    user_dn: None,
                    email: None,
                    display_name: None,
                    groups: vec![],
                    error: Some(format!("Failed to connect to LDAP server: {}", e)),
                };
            }
        };

        // Spawn connection driver
        ldap3::drive!(conn);

        // Bind with service account to search for user
        if let Err(e) = ldap.simple_bind(&config.bind_dn, &config.bind_password).await {
            let _ = ldap.unbind().await;
            return LdapAuthResult {
                success: false,
                user_dn: None,
                email: None,
                display_name: None,
                groups: vec![],
                error: Some(format!("Service account bind failed: {}", e)),
            };
        }

        // Search for user
        let user_filter = config.user_filter.replace("{username}", username);
        let search_result = match ldap.search(
            &config.base_dn,
            Scope::Subtree,
            &user_filter,
            vec!["dn", "mail", "displayName", "cn", &config.group_attribute],
        ).await {
            Ok(result) => result,
            Err(e) => {
                let _ = ldap.unbind().await;
                return LdapAuthResult {
                    success: false,
                    user_dn: None,
                    email: None,
                    display_name: None,
                    groups: vec![],
                    error: Some(format!("User search failed: {}", e)),
                };
            }
        };

        let (entries, _result) = search_result.success().unwrap_or((vec![], ldap3::LdapResult {
            rc: 0,
            matched: String::new(),
            text: String::new(),
            refs: vec![],
            ctrls: vec![],
        }));

        if entries.is_empty() {
            let _ = ldap.unbind().await;
            return LdapAuthResult {
                success: false,
                user_dn: None,
                email: None,
                display_name: None,
                groups: vec![],
                error: Some("User not found".to_string()),
            };
        }

        // Parse user entry
        let entry = SearchEntry::construct(entries[0].clone());
        let user_dn = entry.dn.clone();
        let email = entry.attrs.get("mail").and_then(|v| v.first()).cloned();
        let display_name = entry.attrs.get("displayName")
            .or_else(|| entry.attrs.get("cn"))
            .and_then(|v| v.first())
            .cloned();
        let groups: Vec<String> = entry.attrs.get(&config.group_attribute)
            .cloned()
            .unwrap_or_default();

        // Bind as user to verify password
        if let Err(e) = ldap.simple_bind(&user_dn, password).await {
            let _ = ldap.unbind().await;
            return LdapAuthResult {
                success: false,
                user_dn: None,
                email: None,
                display_name: None,
                groups: vec![],
                error: Some(format!("Authentication failed: {}", e)),
            };
        }

        // Verify bind was successful
        let bind_result = ldap.simple_bind(&user_dn, password).await;
        let _ = ldap.unbind().await;

        match bind_result {
            Ok(result) => {
                if result.rc == 0 {
                    LdapAuthResult {
                        success: true,
                        user_dn: Some(user_dn),
                        email,
                        display_name,
                        groups,
                        error: None,
                    }
                } else {
                    LdapAuthResult {
                        success: false,
                        user_dn: None,
                        email: None,
                        display_name: None,
                        groups: vec![],
                        error: Some("Invalid credentials".to_string()),
                    }
                }
            }
            Err(e) => {
                LdapAuthResult {
                    success: false,
                    user_dn: None,
                    email: None,
                    display_name: None,
                    groups: vec![],
                    error: Some(format!("Bind failed: {}", e)),
                }
            }
        }
    }

    /// Determine role from LDAP groups.
    pub fn determine_role(&self, groups: &[String]) -> UserRole {
        for group in groups {
            if self.config.admin_groups.contains(group) {
                return UserRole::Admin;
            }
        }
        for group in groups {
            if self.config.operator_groups.contains(group) {
                return UserRole::Operator;
            }
        }
        UserRole::Viewer
    }
}

// =============================================================================
// OAuth2 Authentication
// =============================================================================

/// OAuth2 authenticator for external identity providers.
pub struct OAuth2Authenticator {
    config: OAuth2Config,
    pending_states: RwLock<HashMap<String, OAuth2State>>,
    http_client: reqwest::Client,
}

/// Pending OAuth2 state for CSRF protection.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OAuth2State {
    created_at: Instant,
    redirect_uri: String,
}

impl OAuth2Authenticator {
    /// Create a new OAuth2 authenticator.
    pub fn new(config: OAuth2Config) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            config,
            pending_states: RwLock::new(HashMap::new()),
            http_client,
        }
    }

    /// Generate authorization URL for OAuth2 flow.
    pub fn get_authorization_url(&self) -> (String, String) {
        let state = generate_token();

        // Store state for verification
        self.pending_states.write().insert(
            state.clone(),
            OAuth2State {
                created_at: Instant::now(),
                redirect_uri: self.config.redirect_uri.clone(),
            },
        );

        let scopes = self.config.scopes.join(" ");
        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            self.config.authorization_url,
            self.config.client_id,
            urlencoding_encode(&self.config.redirect_uri),
            urlencoding_encode(&scopes),
            state
        );

        (url, state)
    }

    /// Verify state parameter.
    pub fn verify_state(&self, state: &str) -> bool {
        let mut states = self.pending_states.write();
        if let Some(stored) = states.remove(state) {
            stored.created_at.elapsed() < Duration::from_secs(600) // 10 minute timeout
        } else {
            false
        }
    }

    /// Exchange authorization code for tokens using real HTTP request.
    pub fn exchange_code(&self, code: &str) -> Result<OAuth2TokenResponse, String> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| "No async runtime available".to_string())?;

        let client = self.http_client.clone();
        let token_url = self.config.token_url.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let redirect_uri = self.config.redirect_uri.clone();
        let code = code.to_string();

        runtime.block_on(async move {
            Self::exchange_code_async(&client, &token_url, &client_id, &client_secret, &redirect_uri, &code).await
        })
    }

    /// Async implementation of token exchange.
    async fn exchange_code_async(
        client: &reqwest::Client,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
        code: &str,
    ) -> Result<OAuth2TokenResponse, String> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
        ];

        let response = client
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Token endpoint returned {}: {}", status, body));
        }

        response
            .json::<OAuth2TokenResponse>()
            .await
            .map_err(|e| format!("Failed to parse token response: {}", e))
    }

    /// Get user info from OAuth2 provider using real HTTP request.
    pub fn get_user_info(&self, access_token: &str) -> Result<OAuth2UserInfo, String> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| "No async runtime available".to_string())?;

        let client = self.http_client.clone();
        let userinfo_url = self.config.userinfo_url.clone();
        let access_token = access_token.to_string();

        runtime.block_on(async move {
            Self::get_user_info_async(&client, &userinfo_url, &access_token).await
        })
    }

    /// Async implementation of user info retrieval.
    async fn get_user_info_async(
        client: &reqwest::Client,
        userinfo_url: &str,
        access_token: &str,
    ) -> Result<OAuth2UserInfo, String> {
        let response = client
            .get(userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Userinfo request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Userinfo endpoint returned {}: {}", status, body));
        }

        response
            .json::<OAuth2UserInfo>()
            .await
            .map_err(|e| format!("Failed to parse userinfo response: {}", e))
    }

    /// Determine role from OAuth2 claims.
    pub fn determine_role(&self, roles: &[String]) -> UserRole {
        for role in roles {
            if self.config.admin_roles.contains(role) {
                return UserRole::Admin;
            }
        }
        for role in roles {
            if self.config.operator_roles.contains(role) {
                return UserRole::Operator;
            }
        }
        UserRole::Viewer
    }
}

/// Simple URL encoding (subset).
fn urlencoding_encode(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('/', "%2F")
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create an auth service with a test user
    fn auth_with_test_user() -> AuthService {
        let auth = AuthService::new();
        auth.create_user("testuser", "test@example.com", "TestPassword123!", "viewer").expect("failed to create test user");
        auth
    }

    /// Helper to create an auth service with an admin user (MFA enabled)
    fn auth_with_admin_user() -> (AuthService, String) {
        let auth = AuthService::new();
        auth.create_user("testadmin", "admin@example.com", "AdminPassword123!", "admin").expect("failed to create admin user");

        // Enable MFA and get the secret
        let secret = generate_mfa_secret();
        {
            let mut users = auth.users.write();
            if let Some(user) = users.get_mut("testadmin") {
                user.mfa_enabled = true;
                user.mfa_secret = Some(secret.clone());
            }
        }

        (auth, secret)
    }

    #[test]
    fn test_login_success() {
        let auth = auth_with_test_user();
        let response = auth.login("testuser", "TestPassword123!");
        assert!(response.token.is_some());
        assert!(response.user.is_some());
    }

    #[test]
    fn test_login_invalid_password() {
        let auth = auth_with_test_user();
        let response = auth.login("testuser", "wrong");
        assert!(response.error.is_some());
    }

    #[test]
    fn test_login_mfa_required() {
        let (auth, _secret) = auth_with_admin_user();
        let response = auth.login("testadmin", "AdminPassword123!");
        assert!(response.requires_mfa == Some(true));
        assert!(response.token.is_some());
    }

    #[test]
    fn test_mfa_verification() {
        let (auth, secret) = auth_with_admin_user();
        let login_response = auth.login("testadmin", "AdminPassword123!");
        let temp_token = login_response.token.expect("login should return temp token for MFA");

        // Generate a valid TOTP code using the user's secret
        let totp_code = generate_test_totp(&secret);

        let mfa_response = auth.verify_mfa(&totp_code, &temp_token);
        assert!(mfa_response.token.is_some(), "MFA verification failed: {:?}", mfa_response.error);
        assert!(mfa_response.user.is_some());
    }

    /// Generate a TOTP code for testing using the same algorithm as verify_totp
    fn generate_test_totp(secret: &str) -> String {
        use data_encoding::BASE32_NOPAD;
        use hmac::{Hmac, Mac};
        use sha1::Sha1;

        let secret_bytes = BASE32_NOPAD.decode(secret.to_uppercase().as_bytes())
            .unwrap_or_else(|_| {
                let padded = format!("{}{}", secret.to_uppercase(), &"========"[..((8 - secret.len() % 8) % 8)]);
                data_encoding::BASE32.decode(padded.as_bytes()).expect("padded BASE32 decode should succeed")
            });

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_secs();
        let time_step = timestamp / 30;
        let counter_bytes = time_step.to_be_bytes();

        let mut mac = Hmac::<Sha1>::new_from_slice(&secret_bytes).expect("HMAC key should be valid");
        mac.update(&counter_bytes);
        let result = mac.finalize().into_bytes();

        let offset_idx = (result[19] & 0x0f) as usize;
        let binary_code = ((result[offset_idx] & 0x7f) as u32) << 24
            | (result[offset_idx + 1] as u32) << 16
            | (result[offset_idx + 2] as u32) << 8
            | (result[offset_idx + 3] as u32);

        let otp = binary_code % 1_000_000;
        format!("{:06}", otp)
    }

    #[test]
    fn test_session_validation() {
        let auth = auth_with_test_user();
        let response = auth.login("testuser", "TestPassword123!");
        let token = response.token.expect("login should return token");

        let user = auth.validate_session(&token);
        assert!(user.is_some());
        assert_eq!(user.expect("user should be present").username, "testuser");
    }

    #[test]
    fn test_logout() {
        let auth = auth_with_test_user();
        let response = auth.login("testuser", "TestPassword123!");
        let token = response.token.expect("login should return token");

        assert!(auth.logout(&token));
        assert!(auth.validate_session(&token).is_none());
    }

    #[test]
    fn test_password_validation() {
        let auth = AuthService::new();

        // Password too short
        let result = auth.create_user("user1", "test@example.com", "short", "viewer");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("8 characters"));
    }

    #[test]
    fn test_username_validation() {
        let auth = AuthService::new();

        // Invalid username (contains special chars)
        let result = auth.create_user("user@name", "test@example.com", "ValidPassword123", "viewer");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("alphanumeric"));
    }

    #[test]
    fn test_email_validation() {
        let auth = AuthService::new();

        // Invalid email
        let result = auth.create_user("username", "invalid-email", "ValidPassword123", "viewer");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("email"));
    }

    // RBAC Tests
    #[test]
    fn test_rbac_default_roles() {
        let rbac = RbacManager::new();
        let roles = rbac.list_roles();
        assert!(roles.iter().any(|r| r.name == "admin"));
        assert!(roles.iter().any(|r| r.name == "operator"));
        assert!(roles.iter().any(|r| r.name == "viewer"));
    }

    #[test]
    fn test_rbac_check_permission() {
        let rbac = RbacManager::new();

        // Assign admin role to a test user
        rbac.assign_role("test-user-1", "admin").expect("failed to assign admin role");
        rbac.assign_role("test-user-2", "viewer").expect("failed to assign viewer role");

        // Admin user should have all permissions
        assert!(rbac.check_permission("test-user-1", Permission::DatabaseCreate));
        assert!(rbac.check_permission("test-user-1", Permission::ClusterManage));

        // Viewer should only have read permissions
        assert!(rbac.check_permission("test-user-2", Permission::DataSelect));
        assert!(!rbac.check_permission("test-user-2", Permission::DataInsert));
    }

    #[test]
    fn test_rbac_create_role() {
        let rbac = RbacManager::new();
        let result = rbac.create_role(
            "custom_role",
            "Custom role for testing",
            vec![Permission::DataSelect, Permission::DataInsert],
            "admin"
        );
        assert!(result.is_ok());

        let role = rbac.get_role("custom_role");
        assert!(role.is_some());
        assert!(role.expect("role should exist").has_permission(Permission::DataSelect));
    }

    #[test]
    fn test_rbac_assign_role() {
        let rbac = RbacManager::new();
        let result = rbac.assign_role("new-user", "analyst");
        assert!(result.is_ok());

        let roles = rbac.get_user_roles("new-user");
        assert!(roles.contains(&"analyst".to_string()));
    }

    #[test]
    fn test_rbac_cannot_delete_builtin() {
        let rbac = RbacManager::new();
        let result = rbac.delete_role("admin");
        assert!(result.is_err());
    }

    // Audit Logging Tests
    #[test]
    fn test_audit_log_entry() {
        let audit = AuditLogger::new(1000);
        audit.log_login_success("user-001", "testuser", Some("192.168.1.1"));

        let entries = audit.get_entries(10, 0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::LoginSuccess);
    }

    #[test]
    fn test_audit_get_by_type() {
        let audit = AuditLogger::new(1000);
        audit.log_login_success("user-001", "admin", None);
        audit.log_login_failure("baduser", None, "Invalid password");
        audit.log_login_success("user-002", "demo", None);

        let failures = audit.get_entries_by_type(AuditEventType::LoginFailure, 10);
        assert_eq!(failures.len(), 1);

        let successes = audit.get_entries_by_type(AuditEventType::LoginSuccess, 10);
        assert_eq!(successes.len(), 2);
    }

    #[test]
    fn test_audit_get_for_user() {
        let audit = AuditLogger::new(1000);
        audit.log_login_success("user-001", "admin", None);
        audit.log_data_access("user-001", "admin", "users", "SELECT", 10);
        audit.log_login_success("user-002", "demo", None);

        let user1_entries = audit.get_entries_for_user("user-001", 10);
        assert_eq!(user1_entries.len(), 2);
    }

    #[test]
    fn test_audit_max_entries() {
        let audit = AuditLogger::new(5);
        for i in 0..10 {
            audit.log_login_success(&format!("user-{}", i), "test", None);
        }

        assert_eq!(audit.count(), 5);
    }

    // Password hashing tests
    #[test]
    fn test_password_hashing_unique() {
        let hash1 = hash_password("testpassword");
        let hash2 = hash_password("testpassword");
        // Each hash should be different due to random salt
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_password_verification() {
        let password = "SecurePassword123!";
        let hash = hash_password(password);
        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrongpassword", &hash));
    }

    #[test]
    fn test_token_generation_unique() {
        let token1 = generate_token();
        let token2 = generate_token();
        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 64); // 256 bits = 64 hex chars
    }

    #[test]
    fn test_mfa_secret_generation() {
        let secret1 = generate_mfa_secret();
        let secret2 = generate_mfa_secret();
        assert_ne!(secret1, secret2);
        assert_eq!(secret1.len(), 32); // 160 bits = 32 base32 chars
    }

    // LDAP Tests
    #[test]
    fn test_ldap_authenticator_config_validation() {
        // Test with empty server URL - should fail validation
        let mut config = LdapConfig::default();
        config.server_url = String::new();
        let ldap = LdapAuthenticator::new(config);

        let result = ldap.authenticate("testuser", "password");
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.as_ref().expect("error should be present").contains("not configured"));
    }

    #[test]
    fn test_ldap_authenticator_empty_password() {
        let config = LdapConfig::default();
        let ldap = LdapAuthenticator::new(config);

        let result = ldap.authenticate("testuser", "");
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.as_ref().expect("error should be present").contains("Password is required"));
    }

    #[test]
    fn test_ldap_role_mapping() {
        let config = LdapConfig::default();
        let ldap = LdapAuthenticator::new(config.clone());

        let admin_role = ldap.determine_role(&config.admin_groups);
        assert_eq!(admin_role, UserRole::Admin);

        let viewer_role = ldap.determine_role(&[]);
        assert_eq!(viewer_role, UserRole::Viewer);
    }

    // OAuth2 Tests
    #[test]
    fn test_oauth2_authorization_url() {
        let config = OAuth2Config::default();
        let oauth = OAuth2Authenticator::new(config);

        let (url, state) = oauth.get_authorization_url();
        assert!(url.contains("client_id="));
        assert!(url.contains(&state));
    }

    #[test]
    fn test_oauth2_state_verification() {
        let config = OAuth2Config::default();
        let oauth = OAuth2Authenticator::new(config);

        let (_, state) = oauth.get_authorization_url();
        assert!(oauth.verify_state(&state));
        assert!(!oauth.verify_state(&state)); // Second call should fail
    }

    #[test]
    fn test_oauth2_role_mapping() {
        let config = OAuth2Config::default();
        let oauth = OAuth2Authenticator::new(config.clone());

        let admin_role = oauth.determine_role(&config.admin_roles);
        assert_eq!(admin_role, UserRole::Admin);
    }
}
