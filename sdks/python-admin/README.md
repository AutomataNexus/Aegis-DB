# Aegis-DB Admin SDK for Python

Server-side admin SDK for [Aegis-DB](https://github.com/aegisdb/aegis-db). Covers all privileged operations: user/role management, cluster administration, backups, vault secrets, security shield, and compliance (GDPR/CCPA).

## Installation

```bash
pip install aegis-db-admin
```

## Quick Start

```python
import asyncio
from aegis_db_admin import AegisAdmin

async def main():
    async with AegisAdmin(
        "http://localhost:9090",
        username="admin",
        password="secret",
    ) as admin:
        # List users
        users = await admin.users.list_users()

        # Create a user
        user = await admin.users.create_user(
            "alice", "p@ssw0rd", email="alice@example.com", role="editor"
        )

        # Cluster info
        info = await admin.cluster.get_cluster_info()

        # Create a backup
        backup = await admin.backups.create_backup(label="nightly")

        # Vault secrets
        await admin.vault.set_secret("db-password", "s3cret")
        secret = await admin.vault.get_secret("db-password")

        # Shield - block an IP
        await admin.shield.block_ip("10.0.0.99", reason="abuse")

        # Compliance - record consent
        await admin.compliance.record_consent("user-123", "marketing", granted=True)

asyncio.run(main())
```

## Authentication

```python
# Username / password (recommended for scripts)
admin = AegisAdmin("http://localhost:9090", username="admin", password="secret")

# Pre-existing token
admin = AegisAdmin("http://localhost:9090", token="eyJ...")

# API key
admin = AegisAdmin("http://localhost:9090", api_key="ak_...")
```

## Sub-services

| Property | Service | Endpoints |
|---|---|---|
| `admin.users` | UserService | `/api/v1/admin/users` |
| `admin.roles` | RoleService | `/api/v1/admin/roles` |
| `admin.cluster` | ClusterService | `/api/v1/admin/cluster`, `/nodes`, `/storage`, `/stats`, `/settings` |
| `admin.backups` | BackupService | `/api/v1/admin/backup`, `/backups`, `/restore` |
| `admin.vault` | VaultService | `/api/v1/vault/*` |
| `admin.shield` | ShieldService | `/api/v1/shield/*` |
| `admin.compliance` | ComplianceService | `/api/v1/compliance/*` |

## Error Handling

```python
from aegis_db_admin import (
    AdminError,
    AuthenticationError,
    ConnectionError,
    NotFoundError,
    ConflictError,
    ValidationError,
)

try:
    await admin.users.delete_user("nonexistent")
except NotFoundError:
    print("User does not exist")
except AuthenticationError:
    print("Not authorized")
except AdminError as e:
    print(f"Error ({e.status_code}): {e}")
```

## License

MIT
