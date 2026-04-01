"""
Aegis-DB Admin SDK Type Definitions

Dataclass response types for all admin API endpoints.

@version 1.0.0
@author AutomataNexus Development Team
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


# =============================================================================
# Errors
# =============================================================================

class AdminError(Exception):
    """Base exception for Aegis admin errors."""

    def __init__(self, message: str, status_code: Optional[int] = None):
        super().__init__(message)
        self.status_code = status_code


class AuthenticationError(AdminError):
    """Authentication or authorization failure."""
    pass


class ConnectionError(AdminError):
    """Connection-related errors."""
    pass


class NotFoundError(AdminError):
    """Resource not found."""
    pass


class ConflictError(AdminError):
    """Resource conflict (e.g., duplicate)."""
    pass


class ValidationError(AdminError):
    """Request validation failure."""
    pass


# =============================================================================
# Auth / Users
# =============================================================================

@dataclass
class User:
    """A user account."""
    username: str
    email: Optional[str] = None
    role: Optional[str] = None
    enabled: bool = True
    created_at: Optional[str] = None
    updated_at: Optional[str] = None
    last_login: Optional[str] = None
    mfa_enabled: bool = False


@dataclass
class Role:
    """A role definition."""
    name: str
    description: Optional[str] = None
    permissions: List[str] = field(default_factory=list)
    created_at: Optional[str] = None


# =============================================================================
# Cluster
# =============================================================================

@dataclass
class ClusterInfo:
    """Cluster overview."""
    cluster_name: Optional[str] = None
    node_count: int = 0
    leader: Optional[str] = None
    status: Optional[str] = None
    version: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class NodeInfo:
    """A cluster node."""
    id: Optional[str] = None
    name: Optional[str] = None
    address: Optional[str] = None
    role: Optional[str] = None
    status: Optional[str] = None
    uptime: Optional[str] = None
    version: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class StorageInfo:
    """Storage statistics."""
    total_bytes: int = 0
    used_bytes: int = 0
    free_bytes: int = 0
    backend: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class QueryStats:
    """Query statistics."""
    total_queries: int = 0
    queries_per_second: float = 0.0
    avg_latency_ms: float = 0.0
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class DatabaseStats:
    """Database statistics."""
    table_count: int = 0
    total_rows: int = 0
    total_size_bytes: int = 0
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class Alert:
    """An alert entry."""
    id: Optional[str] = None
    severity: Optional[str] = None
    message: Optional[str] = None
    timestamp: Optional[str] = None
    acknowledged: bool = False
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class Activity:
    """An activity log entry."""
    id: Optional[str] = None
    action: Optional[str] = None
    user: Optional[str] = None
    timestamp: Optional[str] = None
    details: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


# =============================================================================
# Backups
# =============================================================================

@dataclass
class Backup:
    """A backup record."""
    id: Optional[str] = None
    status: Optional[str] = None
    created_at: Optional[str] = None
    size_bytes: int = 0
    path: Optional[str] = None
    node: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class RestoreResult:
    """Result of a restore operation."""
    success: bool = False
    message: Optional[str] = None
    restored_at: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


# =============================================================================
# Vault
# =============================================================================

@dataclass
class VaultStatus:
    """Vault status."""
    sealed: bool = True
    initialized: bool = False
    version: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class Secret:
    """A stored secret."""
    key: str
    value: Optional[Any] = None
    version: Optional[int] = None
    created_at: Optional[str] = None
    updated_at: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class TransitKey:
    """A transit encryption key."""
    name: str
    algorithm: Optional[str] = None
    version: Optional[int] = None
    created_at: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class TransitResult:
    """Result of transit encrypt/decrypt."""
    ciphertext: Optional[str] = None
    plaintext: Optional[str] = None
    key_version: Optional[int] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class AuditEntry:
    """A vault audit log entry."""
    id: Optional[str] = None
    operation: Optional[str] = None
    path: Optional[str] = None
    user: Optional[str] = None
    timestamp: Optional[str] = None
    status: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


# =============================================================================
# Shield
# =============================================================================

@dataclass
class ShieldStatus:
    """Shield status."""
    enabled: bool = False
    mode: Optional[str] = None
    rules_count: int = 0
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class ShieldStats:
    """Shield statistics."""
    total_requests: int = 0
    blocked_requests: int = 0
    threats_detected: int = 0
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class SecurityEvent:
    """A security event."""
    id: Optional[str] = None
    event_type: Optional[str] = None
    severity: Optional[str] = None
    source_ip: Optional[str] = None
    message: Optional[str] = None
    timestamp: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class BlockedIP:
    """A blocked IP entry."""
    ip: Optional[str] = None
    reason: Optional[str] = None
    blocked_at: Optional[str] = None
    expires_at: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class AllowlistEntry:
    """An allowlist entry."""
    ip: Optional[str] = None
    added_at: Optional[str] = None
    description: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class ShieldPolicy:
    """Shield policy configuration."""
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)

    def __getitem__(self, key: str) -> Any:
        return self._raw[key]

    def get(self, key: str, default: Any = None) -> Any:
        return self._raw.get(key, default)

    def to_dict(self) -> Dict[str, Any]:
        return self._raw.copy()


@dataclass
class IPReputation:
    """IP reputation info."""
    ip: Optional[str] = None
    score: Optional[float] = None
    classification: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class ThreatFeedEntry:
    """Threat feed entry."""
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)

    def __getitem__(self, key: str) -> Any:
        return self._raw[key]

    def get(self, key: str, default: Any = None) -> Any:
        return self._raw.get(key, default)

    def to_dict(self) -> Dict[str, Any]:
        return self._raw.copy()


# =============================================================================
# Compliance
# =============================================================================

@dataclass
class DeletionCertificate:
    """GDPR deletion certificate."""
    id: Optional[str] = None
    subject_id: Optional[str] = None
    deleted_at: Optional[str] = None
    verified: Optional[bool] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class ConsentRecord:
    """A consent record."""
    subject_id: Optional[str] = None
    purpose: Optional[str] = None
    granted: bool = False
    timestamp: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class ConsentStats:
    """Consent statistics."""
    total_subjects: int = 0
    total_consents: int = 0
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class Breach:
    """A breach record."""
    id: Optional[str] = None
    severity: Optional[str] = None
    description: Optional[str] = None
    status: Optional[str] = None
    detected_at: Optional[str] = None
    acknowledged_at: Optional[str] = None
    resolved_at: Optional[str] = None
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)


@dataclass
class BreachStats:
    """Breach statistics."""
    total: int = 0
    open: int = 0
    acknowledged: int = 0
    resolved: int = 0
    _raw: Dict[str, Any] = field(default_factory=dict, repr=False)
