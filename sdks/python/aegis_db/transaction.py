"""
Aegis Database Transaction Support

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .client import AegisClient
    from .types import QueryResult


class Transaction:
    """
    Database transaction with savepoint support.

    Example:
        async with client.transaction() as tx:
            await tx.execute("INSERT INTO users (name) VALUES (?)", {"name": "Alice"})
            savepoint = await tx.savepoint("sp1")
            try:
                await tx.execute("INSERT INTO logs (msg) VALUES (?)", {"msg": "test"})
            except Exception:
                await tx.rollback_to(savepoint)
    """

    def __init__(self, client: "AegisClient"):
        self._client = client
        self._active = False
        self._tx_id: Optional[str] = None
        self._savepoints: List[str] = []

    @property
    def is_active(self) -> bool:
        """Check if transaction is active."""
        return self._active

    async def begin(self) -> None:
        """Start the transaction."""
        if self._active:
            raise RuntimeError("Transaction already active")

        result = await self._client.query("BEGIN TRANSACTION")
        self._active = True
        # In a real implementation, we'd get a transaction ID from the server

    async def commit(self) -> None:
        """Commit the transaction."""
        if not self._active:
            raise RuntimeError("No active transaction")

        await self._client.query("COMMIT")
        self._active = False
        self._savepoints.clear()

    async def rollback(self) -> None:
        """Rollback the transaction."""
        if not self._active:
            raise RuntimeError("No active transaction")

        await self._client.query("ROLLBACK")
        self._active = False
        self._savepoints.clear()

    async def savepoint(self, name: str) -> str:
        """Create a savepoint."""
        if not self._active:
            raise RuntimeError("No active transaction")

        await self._client.query(f"SAVEPOINT {name}")
        self._savepoints.append(name)
        return name

    async def rollback_to(self, savepoint: str) -> None:
        """Rollback to a savepoint."""
        if not self._active:
            raise RuntimeError("No active transaction")

        if savepoint not in self._savepoints:
            raise ValueError(f"Unknown savepoint: {savepoint}")

        await self._client.query(f"ROLLBACK TO SAVEPOINT {savepoint}")

        # Remove savepoints created after this one
        idx = self._savepoints.index(savepoint)
        self._savepoints = self._savepoints[:idx + 1]

    async def release_savepoint(self, savepoint: str) -> None:
        """Release a savepoint."""
        if not self._active:
            raise RuntimeError("No active transaction")

        if savepoint not in self._savepoints:
            raise ValueError(f"Unknown savepoint: {savepoint}")

        await self._client.query(f"RELEASE SAVEPOINT {savepoint}")
        self._savepoints.remove(savepoint)

    async def execute(
        self,
        sql: str,
        params: Optional[Dict[str, Any]] = None,
    ) -> int:
        """Execute a statement within the transaction."""
        if not self._active:
            raise RuntimeError("No active transaction")

        result = await self._client.query(sql, params)
        return result.rows_affected

    async def query(
        self,
        sql: str,
        params: Optional[Dict[str, Any]] = None,
    ) -> "QueryResult":
        """Execute a query within the transaction."""
        if not self._active:
            raise RuntimeError("No active transaction")

        return await self._client.query(sql, params)
