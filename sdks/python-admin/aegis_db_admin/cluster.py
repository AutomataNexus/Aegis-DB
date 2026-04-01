"""
Aegis-DB Admin SDK - Cluster Service

Cluster, node, storage, stats, alerts, activities, and settings management.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .types import (
    ClusterInfo,
    NodeInfo,
    StorageInfo,
    QueryStats,
    DatabaseStats,
    Alert,
    Activity,
)

if TYPE_CHECKING:
    from .client import AegisAdmin


class ClusterService:
    """Manage cluster, nodes, settings, and observability."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    # =========================================================================
    # Cluster
    # =========================================================================

    async def get_cluster_info(self) -> ClusterInfo:
        """Get cluster information.

        Returns:
            ClusterInfo object.
        """
        data = await self._admin._request("GET", "/api/v1/admin/cluster")
        return ClusterInfo(
            cluster_name=data.get("cluster_name"),
            node_count=data.get("node_count", 0),
            leader=data.get("leader"),
            status=data.get("status"),
            version=data.get("version"),
            _raw=data,
        )

    # =========================================================================
    # Nodes
    # =========================================================================

    async def list_nodes(self) -> List[NodeInfo]:
        """List all cluster nodes.

        Returns:
            List of NodeInfo objects.
        """
        data = await self._admin._request("GET", "/api/v1/admin/nodes")
        nodes_list = data if isinstance(data, list) else data.get("nodes", [])
        return [_parse_node(n) for n in nodes_list]

    async def restart_node(self, node_id: str) -> Dict[str, Any]:
        """Restart a cluster node.

        Args:
            node_id: The node identifier.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "POST", f"/api/v1/admin/nodes/{node_id}/restart"
        )

    async def drain_node(self, node_id: str) -> Dict[str, Any]:
        """Drain a cluster node (stop accepting new work).

        Args:
            node_id: The node identifier.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "POST", f"/api/v1/admin/nodes/{node_id}/drain"
        )

    async def get_node_logs(
        self,
        node_id: str,
        *,
        lines: Optional[int] = None,
        level: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Get logs from a node.

        Args:
            node_id: The node identifier.
            lines: Number of log lines to retrieve.
            level: Log level filter (e.g. 'error', 'warn', 'info').

        Returns:
            Dict containing log entries.
        """
        params: List[str] = []
        if lines is not None:
            params.append(f"lines={lines}")
        if level is not None:
            params.append(f"level={level}")
        qs = f"?{'&'.join(params)}" if params else ""
        return await self._admin._request(
            "GET", f"/api/v1/admin/nodes/{node_id}/logs{qs}"
        )

    async def remove_node(self, node_id: str) -> Dict[str, Any]:
        """Remove a node from the cluster.

        Args:
            node_id: The node identifier.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "DELETE", f"/api/v1/admin/nodes/{node_id}"
        )

    # =========================================================================
    # Storage / Stats / Database
    # =========================================================================

    async def get_storage_info(self) -> StorageInfo:
        """Get storage information.

        Returns:
            StorageInfo object.
        """
        data = await self._admin._request("GET", "/api/v1/admin/storage")
        return StorageInfo(
            total_bytes=data.get("total_bytes", 0),
            used_bytes=data.get("used_bytes", 0),
            free_bytes=data.get("free_bytes", 0),
            backend=data.get("backend"),
            _raw=data,
        )

    async def get_query_stats(self) -> QueryStats:
        """Get query statistics.

        Returns:
            QueryStats object.
        """
        data = await self._admin._request("GET", "/api/v1/admin/stats")
        return QueryStats(
            total_queries=data.get("total_queries", 0),
            queries_per_second=data.get("queries_per_second", 0.0),
            avg_latency_ms=data.get("avg_latency_ms", 0.0),
            _raw=data,
        )

    async def get_database_stats(self) -> DatabaseStats:
        """Get database statistics.

        Returns:
            DatabaseStats object.
        """
        data = await self._admin._request("GET", "/api/v1/admin/database")
        return DatabaseStats(
            table_count=data.get("table_count", 0),
            total_rows=data.get("total_rows", 0),
            total_size_bytes=data.get("total_size_bytes", 0),
            _raw=data,
        )

    # =========================================================================
    # Alerts / Activities
    # =========================================================================

    async def get_alerts(self) -> List[Alert]:
        """Get active alerts.

        Returns:
            List of Alert objects.
        """
        data = await self._admin._request("GET", "/api/v1/admin/alerts")
        alerts_list = data if isinstance(data, list) else data.get("alerts", [])
        return [
            Alert(
                id=a.get("id"),
                severity=a.get("severity"),
                message=a.get("message"),
                timestamp=a.get("timestamp"),
                acknowledged=a.get("acknowledged", False),
                _raw=a,
            )
            for a in alerts_list
        ]

    async def get_activities(self) -> List[Activity]:
        """Get activity log.

        Returns:
            List of Activity objects.
        """
        data = await self._admin._request("GET", "/api/v1/admin/activities")
        acts_list = data if isinstance(data, list) else data.get("activities", [])
        return [
            Activity(
                id=a.get("id"),
                action=a.get("action"),
                user=a.get("user"),
                timestamp=a.get("timestamp"),
                details=a.get("details"),
                _raw=a,
            )
            for a in acts_list
        ]

    # =========================================================================
    # Settings
    # =========================================================================

    async def get_settings(self) -> Dict[str, Any]:
        """Get current server settings.

        Returns:
            Settings dict.
        """
        return await self._admin._request("GET", "/api/v1/admin/settings")

    async def update_settings(self, settings: Dict[str, Any]) -> Dict[str, Any]:
        """Update server settings.

        Args:
            settings: Dict of settings to update.

        Returns:
            Updated settings dict.
        """
        return await self._admin._request("PUT", "/api/v1/admin/settings", settings)


# =============================================================================
# Helpers
# =============================================================================

def _parse_node(data: Dict[str, Any]) -> NodeInfo:
    return NodeInfo(
        id=data.get("id"),
        name=data.get("name"),
        address=data.get("address"),
        role=data.get("role"),
        status=data.get("status"),
        uptime=data.get("uptime"),
        version=data.get("version"),
        _raw=data,
    )
