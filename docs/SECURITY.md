# AegisDB Security Guide

This document covers security features, best practices, and configuration for production deployments of AegisDB.

## Table of Contents

1. [Overview](#overview)
2. [TLS/HTTPS Configuration](#tlshttps-configuration)
3. [Authentication](#authentication)
4. [Password Security](#password-security)
5. [Rate Limiting](#rate-limiting)
6. [Secrets Management](#secrets-management)
7. [Nginx Reverse Proxy](#nginx-reverse-proxy)
8. [Security Headers](#security-headers)
9. [Audit Logging](#audit-logging)
10. [Best Practices](#best-practices)

---

## Overview

AegisDB v0.1.8+ includes production-ready security features:

| Feature | Implementation | Status |
|---------|---------------|--------|
| TLS/HTTPS | axum-server with rustls | ✅ Production Ready |
| Password Hashing | Argon2id | ✅ Production Ready |
| Rate Limiting | Token Bucket Algorithm | ✅ Production Ready |
| Secrets Management | HashiCorp Vault | ✅ Production Ready |
| Authentication | Local, LDAP, OAuth2 | ✅ Production Ready |
| MFA/2FA | TOTP (RFC 6238) | ✅ Production Ready |
| RBAC | 25+ Permissions | ✅ Production Ready |
| Audit Logging | Comprehensive Activity Log | ✅ Production Ready |

---

## TLS/HTTPS Configuration

### Native TLS Support

AegisDB supports native TLS via rustls:

```bash
# Start server with TLS
cargo run -p aegis-server -- \
  --tls \
  --tls-cert /path/to/server.crt \
  --tls-key /path/to/server.key
```

### Environment Variables

```bash
export AEGIS_TLS_CERT=/path/to/server.crt
export AEGIS_TLS_KEY=/path/to/server.key
cargo run -p aegis-server -- --tls
```

### Generate Self-Signed Certificates (Development)

```bash
# Create certificates directory
mkdir -p certs

# Generate self-signed certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout certs/server.key \
  -out certs/server.crt \
  -days 365 -nodes \
  -subj "/CN=localhost/O=Aegis-DB/C=US"

# Add Subject Alternative Names for local testing
openssl req -x509 -newkey rsa:4096 \
  -keyout certs/server.key \
  -out certs/server.crt \
  -days 365 -nodes \
  -subj "/CN=localhost/O=Aegis-DB/C=US" \
  -addext "subjectAltName=DNS:localhost,DNS:aegis.local,IP:127.0.0.1"
```

### Production Certificates

For production, use certificates from a trusted CA:

```bash
# Let's Encrypt (via certbot)
sudo certbot certonly --standalone -d aegis.example.com

# Use the generated certificates
cargo run -p aegis-server -- \
  --tls \
  --tls-cert /etc/letsencrypt/live/aegis.example.com/fullchain.pem \
  --tls-key /etc/letsencrypt/live/aegis.example.com/privkey.pem
```

### TLS Configuration

The server uses secure defaults:
- **Protocols**: TLSv1.2, TLSv1.3
- **Cipher Suites**: Modern ECDHE-based ciphers
- **Session Tickets**: Disabled for forward secrecy

---

## Authentication

### Supported Methods

| Method | Description | Configuration |
|--------|-------------|---------------|
| Local | Username/password stored in AegisDB | Default |
| LDAP | Active Directory / OpenLDAP | `auth.method = "ldap"` |
| OAuth2/OIDC | Okta, Auth0, Azure AD, etc. | `auth.method = "oauth2"` |

### Local Authentication

Users are authenticated against locally stored credentials:

```bash
# Login
curl -X POST https://localhost:9090/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "demo", "password": "demo"}'

# Response
{
  "token": "abc123...",
  "user": {
    "username": "demo",
    "role": "viewer",
    "mfa_enabled": false
  }
}
```

### MFA/2FA (TOTP)

Multi-factor authentication using TOTP (RFC 6238):

```bash
# Enable MFA for a user (returns secret)
POST /api/v1/auth/mfa/enable

# Verify MFA code after login
POST /api/v1/auth/mfa/verify
{
  "token": "temporary-token",
  "code": "123456"
}
```

---

## Password Security

### Argon2id Hashing

Passwords are hashed using Argon2id, the winner of the Password Hashing Competition:

| Parameter | Value | Purpose |
|-----------|-------|---------|
| Memory Cost | 19 MiB | Resist GPU attacks |
| Time Cost | 2 iterations | Increase computation time |
| Parallelism | 1 | Single-threaded |
| Output Length | 32 bytes | 256-bit hash |
| Salt | Random 16 bytes | Unique per password |

### Implementation

```rust
use argon2::{Argon2, Algorithm, Version, Params, PasswordHasher};

const ARGON2_MEMORY_COST: u32 = 19_456; // 19 MiB
const ARGON2_TIME_COST: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(ARGON2_MEMORY_COST, ARGON2_TIME_COST, ARGON2_PARALLELISM, None)
            .expect("Invalid Argon2 params")
    );
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}
```

---

## Rate Limiting

### Token Bucket Algorithm

AegisDB implements token bucket rate limiting to prevent brute force attacks:

| Endpoint Category | Limit | Window |
|-------------------|-------|--------|
| Login endpoints | 30 requests | per minute per IP |
| General API | 1000 requests | per minute per IP |

### Response Headers

Rate limit information is included in response headers:

```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1706288400
```

### Rate Limited Response

When rate limited, the API returns:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60

{
  "error": "Rate limit exceeded. Try again in 60 seconds."
}
```

### Configuration

```toml
[rate_limit]
requests_per_minute = 1000
login_requests_per_minute = 30
```

---

## Secrets Management

### HashiCorp Vault Integration

AegisDB integrates with HashiCorp Vault for enterprise secrets management.

### Environment Variables

```bash
# Vault server address
export VAULT_ADDR=https://vault.example.com:8200

# Authentication methods (choose one):

# 1. Token-based (simplest)
export VAULT_TOKEN=hvs.your-vault-token

# 2. AppRole (recommended for production)
export VAULT_ROLE_ID=your-role-id
export VAULT_SECRET_ID=your-secret-id

# 3. Kubernetes (for K8s deployments)
export VAULT_KUBERNETES_ROLE=aegis-db
```

### Vault Secret Paths

| Secret | Vault Path | Environment Fallback |
|--------|------------|---------------------|
| TLS Certificate Path | `secret/data/aegis/tls_cert_path` | `AEGIS_TLS_CERT` |
| TLS Key Path | `secret/data/aegis/tls_key_path` | `AEGIS_TLS_KEY` |
| Database Password | `secret/data/aegis/db_password` | `AEGIS_DB_PASSWORD` |

### Vault Policy

```hcl
# aegis-db-policy.hcl
path "secret/data/aegis/*" {
  capabilities = ["read"]
}

path "secret/metadata/aegis/*" {
  capabilities = ["list"]
}
```

### Provider Priority

Secrets are resolved in order:
1. HashiCorp Vault (if configured and available)
2. Environment variables
3. Default values (where applicable)

---

## Nginx Reverse Proxy

### Recommended Configuration

For production deployments, use Nginx as a reverse proxy:

```nginx
# /etc/nginx/sites-available/aegis-db

upstream aegis_backend {
    server 127.0.0.1:9090;
    keepalive 32;
}

server {
    listen 443 ssl http2;
    server_name aegis.example.com;

    # SSL Configuration
    ssl_certificate /etc/letsencrypt/live/aegis.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/aegis.example.com/privkey.pem;

    # Modern SSL settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;
    ssl_prefer_server_ciphers off;
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:50m;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

    # API proxy
    location /api {
        proxy_pass http://aegis_backend;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # Health check
    location = /health {
        proxy_pass http://aegis_backend/health;
    }

    # Dashboard static files
    location / {
        root /opt/Aegis-DB/crates/aegis-dashboard/dist;
        index index.html;
        try_files $uri $uri/ /index.html;
    }
}

# HTTP to HTTPS redirect
server {
    listen 80;
    server_name aegis.example.com;
    return 301 https://$server_name$request_uri;
}
```

---

## Security Headers

AegisDB (via Nginx) adds the following security headers:

| Header | Value | Purpose |
|--------|-------|---------|
| `X-Frame-Options` | `SAMEORIGIN` | Prevent clickjacking |
| `X-Content-Type-Options` | `nosniff` | Prevent MIME sniffing |
| `X-XSS-Protection` | `1; mode=block` | XSS filter |
| `Strict-Transport-Security` | `max-age=31536000` | Force HTTPS |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | Control referrer |

---

## Audit Logging

### Activity Log

All security-relevant actions are logged:

```json
{
  "timestamp": "2026-01-26T15:30:00Z",
  "event_type": "login",
  "user": "admin",
  "ip_address": "192.168.1.100",
  "success": true,
  "details": {
    "method": "local",
    "mfa_used": true
  }
}
```

### Logged Events

| Event | Description |
|-------|-------------|
| `login` | User authentication attempt |
| `logout` | User session termination |
| `mfa_enable` | MFA enabled for user |
| `mfa_verify` | MFA code verification |
| `password_change` | Password modification |
| `user_create` | New user created |
| `user_delete` | User deleted |
| `role_change` | User role modified |
| `query` | Database query executed |
| `config_change` | Configuration modified |

### Log Locations

| Log | Location |
|-----|----------|
| Activity Log | `/var/log/aegis/activity.log` |
| Server Log | `/var/log/aegis/server.log` |
| Audit Log | `/var/log/aegis/audit.log` |

---

## Best Practices

### Production Deployment Checklist

- [ ] **Enable TLS** - Never run without encryption in production
- [ ] **Use strong certificates** - Obtain from trusted CA (Let's Encrypt)
- [ ] **Configure Vault** - Use HashiCorp Vault for secrets
- [ ] **Change default passwords** - Never use `demo/demo` in production
- [ ] **Enable MFA** - Require 2FA for admin accounts
- [ ] **Use Nginx** - Deploy behind reverse proxy
- [ ] **Configure rate limiting** - Protect against brute force
- [ ] **Enable audit logging** - Track all security events
- [ ] **Regular updates** - Keep AegisDB and dependencies updated
- [ ] **Network segmentation** - Isolate database from public internet

### Password Requirements

For user accounts, enforce:
- Minimum 12 characters
- Mix of uppercase, lowercase, numbers, symbols
- No common passwords
- No username in password

### Session Security

- Session tokens are cryptographically random (256 bits)
- Tokens expire after configurable timeout (default: 30 minutes)
- Tokens are invalidated on logout
- Concurrent session limits can be configured

### Network Security

- Bind to `127.0.0.1` by default (localhost only)
- Use firewall rules to restrict access
- Deploy behind VPN for internal-only access
- Use private subnets in cloud deployments

---

## Reporting Security Issues

If you discover a security vulnerability in AegisDB, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email security concerns to: security@automatanexus.com
3. Include detailed steps to reproduce
4. Allow 90 days for patch before public disclosure

---

**Document Version:** 1.0.0
**Last Updated:** January 2026
**Maintainer:** AutomataNexus Security Team
