"""
Aegis-DB Admin SDK - Compliance Service

GDPR data subject operations, consent management, breach tracking,
and security event access.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .types import (
    DeletionCertificate,
    ConsentRecord,
    ConsentStats,
    Breach,
    BreachStats,
    SecurityEvent,
)

if TYPE_CHECKING:
    from .client import AegisAdmin


class ComplianceService:
    """GDPR, consent, breach, and compliance operations."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    # =========================================================================
    # GDPR - Data Subject
    # =========================================================================

    async def delete_data_subject(self, subject_id: str) -> Dict[str, Any]:
        """Delete all data for a data subject (GDPR right to erasure).

        Args:
            subject_id: The data subject identifier.

        Returns:
            Server response dict (typically includes a deletion certificate id).
        """
        return await self._admin._request(
            "DELETE", f"/api/v1/compliance/data-subject/{subject_id}"
        )

    async def export_data_subject(
        self,
        subject_id: str,
        *,
        format: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Export all data for a data subject (GDPR right to data portability).

        Args:
            subject_id: The data subject identifier.
            format: Export format (e.g. 'json', 'csv').

        Returns:
            Dict containing the exported data or download reference.
        """
        payload: Dict[str, Any] = {"subject_id": subject_id}
        if format is not None:
            payload["format"] = format
        return await self._admin._request(
            "POST", "/api/v1/compliance/export", payload
        )

    # =========================================================================
    # Deletion Certificates
    # =========================================================================

    async def list_deletion_certificates(self) -> List[DeletionCertificate]:
        """List all deletion certificates.

        Returns:
            List of DeletionCertificate objects.
        """
        data = await self._admin._request("GET", "/api/v1/compliance/certificates")
        certs = data if isinstance(data, list) else data.get("certificates", [])
        return [_parse_certificate(c) for c in certs]

    async def get_deletion_certificate(self, cert_id: str) -> DeletionCertificate:
        """Get a specific deletion certificate.

        Args:
            cert_id: The certificate identifier.

        Returns:
            DeletionCertificate object.
        """
        data = await self._admin._request(
            "GET", f"/api/v1/compliance/certificates/{cert_id}"
        )
        return _parse_certificate(data)

    async def verify_deletion_certificate(self, cert_id: str) -> Dict[str, Any]:
        """Verify a deletion certificate.

        Args:
            cert_id: The certificate identifier.

        Returns:
            Verification result dict.
        """
        return await self._admin._request(
            "GET", f"/api/v1/compliance/certificates/{cert_id}/verify"
        )

    # =========================================================================
    # Deletion Audit
    # =========================================================================

    async def get_deletion_audit(self, subject_id: str) -> Dict[str, Any]:
        """Get the deletion audit trail for a data subject.

        Args:
            subject_id: The data subject identifier.

        Returns:
            Audit trail dict.
        """
        return await self._admin._request(
            "GET", f"/api/v1/compliance/audit/{subject_id}"
        )

    async def verify_audit_integrity(self) -> Dict[str, Any]:
        """Verify audit log integrity.

        Returns:
            Verification result dict.
        """
        return await self._admin._request("GET", "/api/v1/compliance/audit/verify")

    # =========================================================================
    # Consent Management
    # =========================================================================

    async def record_consent(
        self,
        subject_id: str,
        purpose: str,
        *,
        granted: bool = True,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Record a consent decision.

        Args:
            subject_id: The data subject identifier.
            purpose: The purpose of consent.
            granted: Whether consent was granted.
            metadata: Optional additional metadata.

        Returns:
            Server response dict.
        """
        payload: Dict[str, Any] = {
            "subject_id": subject_id,
            "purpose": purpose,
            "granted": granted,
        }
        if metadata is not None:
            payload["metadata"] = metadata
        return await self._admin._request(
            "POST", "/api/v1/compliance/consent", payload
        )

    async def get_consent_stats(self) -> ConsentStats:
        """Get consent statistics.

        Returns:
            ConsentStats object.
        """
        data = await self._admin._request("GET", "/api/v1/compliance/consent/stats")
        return ConsentStats(
            total_subjects=data.get("total_subjects", 0),
            total_consents=data.get("total_consents", 0),
            _raw=data,
        )

    async def get_consent_status(self, subject_id: str) -> Dict[str, Any]:
        """Get consent status for a data subject.

        Args:
            subject_id: The data subject identifier.

        Returns:
            Dict with consent status per purpose.
        """
        return await self._admin._request(
            "GET", f"/api/v1/compliance/consent/{subject_id}"
        )

    async def delete_consent_data(self, subject_id: str) -> Dict[str, Any]:
        """Delete all consent records for a data subject.

        Args:
            subject_id: The data subject identifier.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "DELETE", f"/api/v1/compliance/consent/{subject_id}"
        )

    async def get_consent_history(self, subject_id: str) -> List[ConsentRecord]:
        """Get consent history for a data subject.

        Args:
            subject_id: The data subject identifier.

        Returns:
            List of ConsentRecord objects.
        """
        data = await self._admin._request(
            "GET", f"/api/v1/compliance/consent/{subject_id}/history"
        )
        records = data if isinstance(data, list) else data.get("history", [])
        return [_parse_consent(r) for r in records]

    async def export_consent(self, subject_id: str) -> Dict[str, Any]:
        """Export consent data for a data subject.

        Args:
            subject_id: The data subject identifier.

        Returns:
            Dict containing exported consent data.
        """
        return await self._admin._request(
            "GET", f"/api/v1/compliance/consent/{subject_id}/export"
        )

    async def check_consent(self, subject_id: str, purpose: str) -> Dict[str, Any]:
        """Check whether a data subject has consented to a specific purpose.

        Args:
            subject_id: The data subject identifier.
            purpose: The purpose to check.

        Returns:
            Dict with consent check result (typically includes 'granted' bool).
        """
        return await self._admin._request(
            "GET", f"/api/v1/compliance/consent/{subject_id}/check/{purpose}"
        )

    async def withdraw_consent(self, subject_id: str, purpose: str) -> Dict[str, Any]:
        """Withdraw consent for a specific purpose.

        Args:
            subject_id: The data subject identifier.
            purpose: The purpose to withdraw consent for.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "DELETE", f"/api/v1/compliance/consent/{subject_id}/{purpose}"
        )

    # =========================================================================
    # CCPA
    # =========================================================================

    async def get_do_not_sell_list(self) -> Dict[str, Any]:
        """Get the CCPA do-not-sell list.

        Returns:
            Dict containing the do-not-sell entries.
        """
        return await self._admin._request("GET", "/api/v1/compliance/do-not-sell")

    # =========================================================================
    # Breaches
    # =========================================================================

    async def list_breaches(self) -> List[Breach]:
        """List all breach records.

        Returns:
            List of Breach objects.
        """
        data = await self._admin._request("GET", "/api/v1/compliance/breaches")
        breaches = data if isinstance(data, list) else data.get("breaches", [])
        return [_parse_breach(b) for b in breaches]

    async def get_breach_stats(self) -> BreachStats:
        """Get breach statistics.

        Returns:
            BreachStats object.
        """
        data = await self._admin._request("GET", "/api/v1/compliance/breaches/stats")
        return BreachStats(
            total=data.get("total", 0),
            open=data.get("open", 0),
            acknowledged=data.get("acknowledged", 0),
            resolved=data.get("resolved", 0),
            _raw=data,
        )

    async def cleanup_breaches(self) -> Dict[str, Any]:
        """Run breach cleanup.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "POST", "/api/v1/compliance/breaches/cleanup"
        )

    async def get_breach(self, breach_id: str) -> Breach:
        """Get a specific breach record.

        Args:
            breach_id: The breach identifier.

        Returns:
            Breach object.
        """
        data = await self._admin._request(
            "GET", f"/api/v1/compliance/breaches/{breach_id}"
        )
        return _parse_breach(data)

    async def acknowledge_breach(self, breach_id: str) -> Dict[str, Any]:
        """Acknowledge a breach.

        Args:
            breach_id: The breach identifier.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "POST", f"/api/v1/compliance/breaches/{breach_id}/acknowledge"
        )

    async def resolve_breach(
        self,
        breach_id: str,
        *,
        resolution: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Resolve a breach.

        Args:
            breach_id: The breach identifier.
            resolution: Optional resolution description.

        Returns:
            Server response dict.
        """
        payload: Dict[str, Any] = {}
        if resolution is not None:
            payload["resolution"] = resolution
        return await self._admin._request(
            "POST", f"/api/v1/compliance/breaches/{breach_id}/resolve", payload
        )

    async def get_breach_report(self, breach_id: str) -> Dict[str, Any]:
        """Get a breach report.

        Args:
            breach_id: The breach identifier.

        Returns:
            Breach report dict.
        """
        return await self._admin._request(
            "GET", f"/api/v1/compliance/breaches/{breach_id}/report"
        )

    # =========================================================================
    # Security Events
    # =========================================================================

    async def get_security_events(self) -> List[SecurityEvent]:
        """Get compliance security events.

        Returns:
            List of SecurityEvent objects.
        """
        data = await self._admin._request(
            "GET", "/api/v1/compliance/security-events"
        )
        events = data if isinstance(data, list) else data.get("events", [])
        return [
            SecurityEvent(
                id=e.get("id"),
                event_type=e.get("event_type"),
                severity=e.get("severity"),
                source_ip=e.get("source_ip"),
                message=e.get("message"),
                timestamp=e.get("timestamp"),
                _raw=e,
            )
            for e in events
        ]


# =============================================================================
# Helpers
# =============================================================================

def _parse_certificate(data: Dict[str, Any]) -> DeletionCertificate:
    return DeletionCertificate(
        id=data.get("id"),
        subject_id=data.get("subject_id"),
        deleted_at=data.get("deleted_at"),
        verified=data.get("verified"),
        _raw=data,
    )


def _parse_consent(data: Dict[str, Any]) -> ConsentRecord:
    return ConsentRecord(
        subject_id=data.get("subject_id"),
        purpose=data.get("purpose"),
        granted=data.get("granted", False),
        timestamp=data.get("timestamp"),
        _raw=data,
    )


def _parse_breach(data: Dict[str, Any]) -> Breach:
    return Breach(
        id=data.get("id"),
        severity=data.get("severity"),
        description=data.get("description"),
        status=data.get("status"),
        detected_at=data.get("detected_at"),
        acknowledged_at=data.get("acknowledged_at"),
        resolved_at=data.get("resolved_at"),
        _raw=data,
    )
