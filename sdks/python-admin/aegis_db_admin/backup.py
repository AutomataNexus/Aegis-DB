"""
Aegis-DB Admin SDK - Backup Service

Create, list, restore, and delete backups.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .types import Backup, RestoreResult

if TYPE_CHECKING:
    from .client import AegisAdmin


class BackupService:
    """Manage backups via the admin API."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    async def create_backup(
        self,
        *,
        label: Optional[str] = None,
        include_wal: bool = True,
    ) -> Backup:
        """Create a new backup.

        Args:
            label: Optional label for the backup.
            include_wal: Whether to include WAL in the backup.

        Returns:
            The created Backup record.
        """
        payload: Dict[str, Any] = {"include_wal": include_wal}
        if label is not None:
            payload["label"] = label

        data = await self._admin._request("POST", "/api/v1/admin/backup", payload)
        return _parse_backup(data)

    async def list_backups(self) -> List[Backup]:
        """List all backups.

        Returns:
            List of Backup objects.
        """
        data = await self._admin._request("GET", "/api/v1/admin/backups")
        backups_list = data if isinstance(data, list) else data.get("backups", [])
        return [_parse_backup(b) for b in backups_list]

    async def restore_backup(
        self,
        backup_id: str,
        *,
        target_node: Optional[str] = None,
    ) -> RestoreResult:
        """Restore from a backup.

        Args:
            backup_id: The backup identifier to restore from.
            target_node: Optional target node for the restore.

        Returns:
            RestoreResult with outcome details.
        """
        payload: Dict[str, Any] = {"backup_id": backup_id}
        if target_node is not None:
            payload["target_node"] = target_node

        data = await self._admin._request("POST", "/api/v1/admin/restore", payload)
        return RestoreResult(
            success=data.get("success", False),
            message=data.get("message"),
            restored_at=data.get("restored_at"),
            _raw=data,
        )

    async def delete_backup(self, backup_id: str) -> Dict[str, Any]:
        """Delete a backup.

        Args:
            backup_id: The backup identifier to delete.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "DELETE", f"/api/v1/admin/backup/{backup_id}"
        )


# =============================================================================
# Helpers
# =============================================================================

def _parse_backup(data: Dict[str, Any]) -> Backup:
    return Backup(
        id=data.get("id"),
        status=data.get("status"),
        created_at=data.get("created_at"),
        size_bytes=data.get("size_bytes", 0),
        path=data.get("path"),
        node=data.get("node"),
        _raw=data,
    )
