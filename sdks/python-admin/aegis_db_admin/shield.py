"""
Aegis-DB Admin SDK - Shield Service

Security shield management: status, events, IP blocking, allowlisting,
policy, IP reputation, and threat feeds.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .types import (
    ShieldStatus,
    ShieldStats,
    SecurityEvent,
    BlockedIP,
    AllowlistEntry,
    ShieldPolicy,
    IPReputation,
    ThreatFeedEntry,
)

if TYPE_CHECKING:
    from .client import AegisAdmin


class ShieldService:
    """Manage the security shield."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    # =========================================================================
    # Status / Stats
    # =========================================================================

    async def get_status(self) -> ShieldStatus:
        """Get shield status.

        Returns:
            ShieldStatus object.
        """
        data = await self._admin._request("GET", "/api/v1/shield/status")
        return ShieldStatus(
            enabled=data.get("enabled", False),
            mode=data.get("mode"),
            rules_count=data.get("rules_count", 0),
            _raw=data,
        )

    async def get_stats(self) -> ShieldStats:
        """Get shield statistics.

        Returns:
            ShieldStats object.
        """
        data = await self._admin._request("GET", "/api/v1/shield/stats")
        return ShieldStats(
            total_requests=data.get("total_requests", 0),
            blocked_requests=data.get("blocked_requests", 0),
            threats_detected=data.get("threats_detected", 0),
            _raw=data,
        )

    # =========================================================================
    # Security Events
    # =========================================================================

    async def get_events(self) -> List[SecurityEvent]:
        """Get security events.

        Returns:
            List of SecurityEvent objects.
        """
        data = await self._admin._request("GET", "/api/v1/shield/events")
        events_list = data if isinstance(data, list) else data.get("events", [])
        return [_parse_security_event(e) for e in events_list]

    # =========================================================================
    # Blocked IPs
    # =========================================================================

    async def list_blocked(self) -> List[BlockedIP]:
        """List blocked IPs.

        Returns:
            List of BlockedIP objects.
        """
        data = await self._admin._request("GET", "/api/v1/shield/blocked")
        blocked_list = data if isinstance(data, list) else data.get("blocked", [])
        return [
            BlockedIP(
                ip=b.get("ip"),
                reason=b.get("reason"),
                blocked_at=b.get("blocked_at"),
                expires_at=b.get("expires_at"),
                _raw=b,
            )
            for b in blocked_list
        ]

    async def block_ip(
        self,
        ip: str,
        *,
        reason: Optional[str] = None,
        duration: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Block an IP address.

        Args:
            ip: The IP address to block.
            reason: Optional reason for blocking.
            duration: Optional duration (e.g. '24h', '7d').

        Returns:
            Server response dict.
        """
        payload: Dict[str, Any] = {"ip": ip}
        if reason is not None:
            payload["reason"] = reason
        if duration is not None:
            payload["duration"] = duration
        return await self._admin._request("POST", "/api/v1/shield/blocked", payload)

    async def unblock_ip(self, ip: str) -> Dict[str, Any]:
        """Unblock an IP address.

        Args:
            ip: The IP address to unblock.

        Returns:
            Server response dict.
        """
        return await self._admin._request("DELETE", f"/api/v1/shield/blocked/{ip}")

    # =========================================================================
    # Allowlist
    # =========================================================================

    async def get_allowlist(self) -> List[AllowlistEntry]:
        """Get the IP allowlist.

        Returns:
            List of AllowlistEntry objects.
        """
        data = await self._admin._request("GET", "/api/v1/shield/allowlist")
        allow_list = data if isinstance(data, list) else data.get("allowlist", [])
        return [
            AllowlistEntry(
                ip=a.get("ip"),
                added_at=a.get("added_at"),
                description=a.get("description"),
                _raw=a,
            )
            for a in allow_list
        ]

    async def add_to_allowlist(
        self,
        ip: str,
        *,
        description: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Add an IP to the allowlist.

        Args:
            ip: The IP address to allow.
            description: Optional description.

        Returns:
            Server response dict.
        """
        payload: Dict[str, Any] = {"ip": ip}
        if description is not None:
            payload["description"] = description
        return await self._admin._request("POST", "/api/v1/shield/allowlist", payload)

    async def remove_from_allowlist(self, ip: str) -> Dict[str, Any]:
        """Remove an IP from the allowlist.

        Args:
            ip: The IP address to remove.

        Returns:
            Server response dict.
        """
        return await self._admin._request("DELETE", f"/api/v1/shield/allowlist/{ip}")

    # =========================================================================
    # Policy
    # =========================================================================

    async def get_policy(self) -> ShieldPolicy:
        """Get the shield security policy.

        Returns:
            ShieldPolicy object.
        """
        data = await self._admin._request("GET", "/api/v1/shield/policy")
        return ShieldPolicy(_raw=data)

    async def update_policy(self, policy: Dict[str, Any]) -> ShieldPolicy:
        """Update the shield security policy.

        Args:
            policy: Policy configuration dict.

        Returns:
            Updated ShieldPolicy object.
        """
        data = await self._admin._request("PUT", "/api/v1/shield/policy", policy)
        return ShieldPolicy(_raw=data)

    # =========================================================================
    # IP Reputation
    # =========================================================================

    async def get_ip_reputation(self, ip: str) -> IPReputation:
        """Get reputation information for an IP address.

        Args:
            ip: The IP address to look up.

        Returns:
            IPReputation object.
        """
        data = await self._admin._request("GET", f"/api/v1/shield/ip/{ip}")
        return IPReputation(
            ip=data.get("ip", ip),
            score=data.get("score"),
            classification=data.get("classification"),
            _raw=data,
        )

    # =========================================================================
    # Threat Feed
    # =========================================================================

    async def get_threat_feed(self) -> List[ThreatFeedEntry]:
        """Get the threat feed.

        Returns:
            List of ThreatFeedEntry objects.
        """
        data = await self._admin._request("GET", "/api/v1/shield/feed")
        feed_list = data if isinstance(data, list) else data.get("feed", [])
        return [ThreatFeedEntry(_raw=f) for f in feed_list]


# =============================================================================
# Helpers
# =============================================================================

def _parse_security_event(data: Dict[str, Any]) -> SecurityEvent:
    return SecurityEvent(
        id=data.get("id"),
        event_type=data.get("event_type"),
        severity=data.get("severity"),
        source_ip=data.get("source_ip"),
        message=data.get("message"),
        timestamp=data.get("timestamp"),
        _raw=data,
    )
