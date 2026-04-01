"""
Aegis-DB Python Admin SDK

Server-side admin client for privileged Aegis-DB operations.

Example::

    import asyncio
    from aegis_db_admin import AegisAdmin

    async def main():
        async with AegisAdmin("http://localhost:9090", username="admin", password="secret") as admin:
            # User management
            users = await admin.users.list_users()
            await admin.users.create_user("alice", "p@ssw0rd", email="alice@example.com", role="editor")

            # Vault secrets
            await admin.vault.set_secret("db-password", "s3cret")
            secret = await admin.vault.get_secret("db-password")

            # Shield
            status = await admin.shield.get_status()
            await admin.shield.block_ip("10.0.0.99", reason="abuse")

            # Compliance
            await admin.compliance.record_consent("user-123", "marketing", granted=True)
            breaches = await admin.compliance.list_breaches()

    asyncio.run(main())

@version 1.0.0
@author AutomataNexus Development Team
"""

from .client import AegisAdmin
from .auth import UserService, RoleService
from .cluster import ClusterService
from .backup import BackupService
from .vault import VaultService
from .shield import ShieldService
from .compliance import ComplianceService
from .types import (
    AdminError,
    AuthenticationError,
    ConnectionError,
    NotFoundError,
    ConflictError,
    ValidationError,
    User,
    Role,
    ClusterInfo,
    NodeInfo,
    StorageInfo,
    QueryStats,
    DatabaseStats,
    Alert,
    Activity,
    Backup,
    RestoreResult,
    VaultStatus,
    Secret,
    TransitKey,
    TransitResult,
    AuditEntry,
    ShieldStatus,
    ShieldStats,
    SecurityEvent,
    BlockedIP,
    AllowlistEntry,
    ShieldPolicy,
    IPReputation,
    ThreatFeedEntry,
    DeletionCertificate,
    ConsentRecord,
    ConsentStats,
    Breach,
    BreachStats,
)

__version__ = "1.0.0"
__all__ = [
    # Main client
    "AegisAdmin",
    # Services
    "UserService",
    "RoleService",
    "ClusterService",
    "BackupService",
    "VaultService",
    "ShieldService",
    "ComplianceService",
    # Errors
    "AdminError",
    "AuthenticationError",
    "ConnectionError",
    "NotFoundError",
    "ConflictError",
    "ValidationError",
    # Auth types
    "User",
    "Role",
    # Cluster types
    "ClusterInfo",
    "NodeInfo",
    "StorageInfo",
    "QueryStats",
    "DatabaseStats",
    "Alert",
    "Activity",
    # Backup types
    "Backup",
    "RestoreResult",
    # Vault types
    "VaultStatus",
    "Secret",
    "TransitKey",
    "TransitResult",
    "AuditEntry",
    # Shield types
    "ShieldStatus",
    "ShieldStats",
    "SecurityEvent",
    "BlockedIP",
    "AllowlistEntry",
    "ShieldPolicy",
    "IPReputation",
    "ThreatFeedEntry",
    # Compliance types
    "DeletionCertificate",
    "ConsentRecord",
    "ConsentStats",
    "Breach",
    "BreachStats",
]
