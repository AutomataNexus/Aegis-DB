"""
Aegis-DB Admin SDK - Vault Service

Secrets management, transit encryption, and audit logging.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .types import VaultStatus, Secret, TransitKey, TransitResult, AuditEntry

if TYPE_CHECKING:
    from .client import AegisAdmin


class VaultService:
    """Manage the integrated secrets vault."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    # =========================================================================
    # Status / Seal
    # =========================================================================

    async def get_status(self) -> VaultStatus:
        """Get vault status.

        Returns:
            VaultStatus object.
        """
        data = await self._admin._request("GET", "/api/v1/vault/status")
        return VaultStatus(
            sealed=data.get("sealed", True),
            initialized=data.get("initialized", False),
            version=data.get("version"),
            _raw=data,
        )

    async def seal(self) -> Dict[str, Any]:
        """Seal the vault.

        Returns:
            Server response dict.
        """
        return await self._admin._request("POST", "/api/v1/vault/seal")

    async def unseal(self, key: Optional[str] = None) -> Dict[str, Any]:
        """Unseal the vault.

        Args:
            key: Unseal key. May be omitted if the vault uses auto-unseal.

        Returns:
            Server response dict.
        """
        payload: Dict[str, Any] = {}
        if key is not None:
            payload["key"] = key
        return await self._admin._request("POST", "/api/v1/vault/unseal", payload)

    # =========================================================================
    # Secrets
    # =========================================================================

    async def list_secrets(self) -> List[Secret]:
        """List all secrets (keys only, values redacted).

        Returns:
            List of Secret objects (values may be None).
        """
        data = await self._admin._request("GET", "/api/v1/vault/secrets")
        secrets_list = data if isinstance(data, list) else data.get("secrets", [])
        return [_parse_secret(s) for s in secrets_list]

    async def get_secret(self, key: str) -> Secret:
        """Get a secret by key.

        Args:
            key: The secret key.

        Returns:
            Secret object with value.
        """
        data = await self._admin._request("GET", f"/api/v1/vault/secrets/{key}")
        return _parse_secret(data)

    async def set_secret(self, key: str, value: Any) -> Secret:
        """Create or update a secret.

        Args:
            key: The secret key.
            value: The secret value.

        Returns:
            The stored Secret.
        """
        payload: Dict[str, Any] = {"value": value}
        data = await self._admin._request(
            "PUT", f"/api/v1/vault/secrets/{key}", payload
        )
        return _parse_secret(data)

    async def delete_secret(self, key: str) -> Dict[str, Any]:
        """Delete a secret.

        Args:
            key: The secret key.

        Returns:
            Server response dict.
        """
        return await self._admin._request("DELETE", f"/api/v1/vault/secrets/{key}")

    # =========================================================================
    # Transit encryption
    # =========================================================================

    async def transit_encrypt(
        self,
        plaintext: str,
        *,
        key_name: Optional[str] = None,
    ) -> TransitResult:
        """Encrypt data using transit encryption.

        Args:
            plaintext: The plaintext data to encrypt.
            key_name: Optional transit key name to use.

        Returns:
            TransitResult with ciphertext.
        """
        payload: Dict[str, Any] = {"plaintext": plaintext}
        if key_name is not None:
            payload["key_name"] = key_name

        data = await self._admin._request(
            "POST", "/api/v1/vault/transit/encrypt", payload
        )
        return TransitResult(
            ciphertext=data.get("ciphertext"),
            plaintext=None,
            key_version=data.get("key_version"),
            _raw=data,
        )

    async def transit_decrypt(
        self,
        ciphertext: str,
        *,
        key_name: Optional[str] = None,
    ) -> TransitResult:
        """Decrypt data using transit encryption.

        Args:
            ciphertext: The ciphertext to decrypt.
            key_name: Optional transit key name to use.

        Returns:
            TransitResult with plaintext.
        """
        payload: Dict[str, Any] = {"ciphertext": ciphertext}
        if key_name is not None:
            payload["key_name"] = key_name

        data = await self._admin._request(
            "POST", "/api/v1/vault/transit/decrypt", payload
        )
        return TransitResult(
            ciphertext=None,
            plaintext=data.get("plaintext"),
            key_version=data.get("key_version"),
            _raw=data,
        )

    async def create_transit_key(
        self,
        name: str,
        *,
        algorithm: Optional[str] = None,
    ) -> TransitKey:
        """Create a new transit encryption key.

        Args:
            name: Key name.
            algorithm: Encryption algorithm (e.g. 'aes256-gcm').

        Returns:
            The created TransitKey.
        """
        payload: Dict[str, Any] = {"name": name}
        if algorithm is not None:
            payload["algorithm"] = algorithm

        data = await self._admin._request(
            "POST", "/api/v1/vault/transit/keys", payload
        )
        return _parse_transit_key(data)

    async def list_transit_keys(self) -> List[TransitKey]:
        """List transit encryption keys.

        Returns:
            List of TransitKey objects.
        """
        data = await self._admin._request("GET", "/api/v1/vault/transit/keys")
        keys_list = data if isinstance(data, list) else data.get("keys", [])
        return [_parse_transit_key(k) for k in keys_list]

    # =========================================================================
    # Audit
    # =========================================================================

    async def get_audit_log(self) -> List[AuditEntry]:
        """Get the vault audit log.

        Returns:
            List of AuditEntry objects.
        """
        data = await self._admin._request("GET", "/api/v1/vault/audit")
        entries = data if isinstance(data, list) else data.get("entries", [])
        return [
            AuditEntry(
                id=e.get("id"),
                operation=e.get("operation"),
                path=e.get("path"),
                user=e.get("user"),
                timestamp=e.get("timestamp"),
                status=e.get("status"),
                _raw=e,
            )
            for e in entries
        ]


# =============================================================================
# Helpers
# =============================================================================

def _parse_secret(data: Dict[str, Any]) -> Secret:
    return Secret(
        key=data.get("key", ""),
        value=data.get("value"),
        version=data.get("version"),
        created_at=data.get("created_at"),
        updated_at=data.get("updated_at"),
        _raw=data,
    )


def _parse_transit_key(data: Dict[str, Any]) -> TransitKey:
    return TransitKey(
        name=data.get("name", ""),
        algorithm=data.get("algorithm"),
        version=data.get("version"),
        created_at=data.get("created_at"),
        _raw=data,
    )
