"""
Aegis-DB Python Admin SDK

Async-first admin client for privileged server-side operations.
Covers user/role management, cluster administration, backups, vault
secrets, security shield, and compliance (GDPR/CCPA).

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, Optional

try:
    import aiohttp
except ImportError:
    aiohttp = None  # type: ignore[assignment]

from .types import (
    AdminError,
    AuthenticationError,
    ConnectionError,
    NotFoundError,
    ConflictError,
    ValidationError,
)
from .auth import UserService, RoleService
from .cluster import ClusterService
from .backup import BackupService
from .vault import VaultService
from .shield import ShieldService
from .compliance import ComplianceService


class AegisAdmin:
    """
    Admin client for Aegis-DB privileged operations.

    Provides sub-services for every admin API surface:
      - ``admin.users``      -- user management
      - ``admin.roles``      -- role management
      - ``admin.cluster``    -- cluster, nodes, storage, stats, settings
      - ``admin.backups``    -- backup and restore
      - ``admin.vault``      -- secrets, transit encryption, audit
      - ``admin.shield``     -- security shield, IP blocking, policy
      - ``admin.compliance`` -- GDPR, consent, breach tracking

    Example::

        async with AegisAdmin("http://localhost:9090", username="admin", password="secret") as admin:
            users = await admin.users.list_users()
            status = await admin.vault.get_status()
            breaches = await admin.compliance.list_breaches()
    """

    def __init__(
        self,
        url: str,
        *,
        username: Optional[str] = None,
        password: Optional[str] = None,
        token: Optional[str] = None,
        api_key: Optional[str] = None,
        timeout: float = 30.0,
        max_connections: int = 10,
    ) -> None:
        """
        Initialize the admin client.

        Provide **either** ``username``/``password`` for credential-based auth
        **or** ``token`` for pre-authenticated access.

        Args:
            url: Aegis-DB server URL (e.g. ``http://localhost:9090``).
            username: Admin username.
            password: Admin password.
            token: Pre-existing bearer token.
            api_key: Optional API key (sent as ``X-API-Key`` header).
            timeout: HTTP request timeout in seconds.
            max_connections: Max concurrent HTTP connections.
        """
        if aiohttp is None:
            raise ImportError(
                "aiohttp is required. Install with: pip install aiohttp"
            )

        self._url: str = url.rstrip("/")
        self._username: Optional[str] = username
        self._password: Optional[str] = password
        self._token: Optional[str] = token
        self._api_key: Optional[str] = api_key
        self._timeout: float = timeout
        self._max_connections: int = max_connections
        self._session: Optional[aiohttp.ClientSession] = None

        # Sub-services
        self.users: UserService = UserService(self)
        self.roles: RoleService = RoleService(self)
        self.cluster: ClusterService = ClusterService(self)
        self.backups: BackupService = BackupService(self)
        self.vault: VaultService = VaultService(self)
        self.shield: ShieldService = ShieldService(self)
        self.compliance: ComplianceService = ComplianceService(self)

    # =========================================================================
    # Lifecycle
    # =========================================================================

    async def __aenter__(self) -> AegisAdmin:
        await self.connect()
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.close()

    async def connect(self) -> None:
        """Open the HTTP session and authenticate if credentials were provided."""
        if self._session is not None:
            return

        connector = aiohttp.TCPConnector(limit=self._max_connections)
        timeout = aiohttp.ClientTimeout(total=self._timeout)
        self._session = aiohttp.ClientSession(
            connector=connector,
            timeout=timeout,
        )

        if self._username and self._password:
            await self._authenticate()

    async def _authenticate(self) -> None:
        """Authenticate with username/password and store the bearer token."""
        payload = {
            "username": self._username,
            "password": self._password,
        }
        try:
            async with self._session.post(  # type: ignore[union-attr]
                f"{self._url}/api/v1/auth/login",
                json=payload,
            ) as resp:
                if resp.status != 200:
                    text = await resp.text()
                    raise AuthenticationError(
                        f"Authentication failed ({resp.status}): {text}",
                        status_code=resp.status,
                    )
                data = await resp.json()
                if data.get("error"):
                    raise AuthenticationError(data["error"])
                self._token = data.get("token")
        except aiohttp.ClientError as exc:
            raise ConnectionError(f"Connection failed during auth: {exc}")

    async def close(self) -> None:
        """Close the HTTP session."""
        if self._session is not None:
            await self._session.close()
            self._session = None

    # =========================================================================
    # Health
    # =========================================================================

    async def health(self) -> Dict[str, Any]:
        """Server health check (``GET /health``).

        Returns:
            Health status dict.
        """
        return await self._request("GET", "/health")

    # =========================================================================
    # Internal HTTP helpers
    # =========================================================================

    def _headers(self) -> Dict[str, str]:
        headers: Dict[str, str] = {"Content-Type": "application/json"}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        if self._api_key:
            headers["X-API-Key"] = self._api_key
        return headers

    async def _request(
        self,
        method: str,
        path: str,
        data: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Execute an HTTP request against the Aegis-DB server.

        Args:
            method: HTTP method (GET, POST, PUT, DELETE).
            path: URL path (e.g. ``/api/v1/admin/users``).
            data: Optional JSON body.

        Returns:
            Parsed JSON response.

        Raises:
            ConnectionError: If the session is not open or a network error occurs.
            AuthenticationError: On 401/403 responses.
            NotFoundError: On 404 responses.
            ConflictError: On 409 responses.
            ValidationError: On 422 responses.
            AdminError: On any other non-2xx response.
        """
        if self._session is None:
            raise ConnectionError("Client not connected. Call connect() first.")

        url = f"{self._url}{path}"
        headers = self._headers()

        try:
            if method == "GET":
                async with self._session.get(url, headers=headers) as resp:
                    return await self._handle_response(resp)
            elif method == "POST":
                async with self._session.post(url, headers=headers, json=data) as resp:
                    return await self._handle_response(resp)
            elif method == "PUT":
                async with self._session.put(url, headers=headers, json=data) as resp:
                    return await self._handle_response(resp)
            elif method == "DELETE":
                async with self._session.delete(url, headers=headers, json=data) as resp:
                    return await self._handle_response(resp)
            else:
                raise ValueError(f"Unsupported HTTP method: {method}")
        except aiohttp.ClientError as exc:
            raise ConnectionError(f"Request failed: {exc}")

    async def _handle_response(
        self, resp: aiohttp.ClientResponse
    ) -> Dict[str, Any]:
        """Parse the HTTP response, raising typed errors for non-2xx statuses."""
        if resp.status < 300:
            # Some endpoints may return empty bodies on success (204, etc.)
            text = await resp.text()
            if not text:
                return {"status": "ok"}
            try:
                return await resp.json(content_type=None)
            except Exception:
                return {"status": "ok", "body": text}

        body = await resp.text()
        status = resp.status

        if status == 401:
            raise AuthenticationError(f"Unauthorized: {body}", status_code=status)
        elif status == 403:
            raise AuthenticationError(f"Forbidden: {body}", status_code=status)
        elif status == 404:
            raise NotFoundError(f"Not found: {body}", status_code=status)
        elif status == 409:
            raise ConflictError(f"Conflict: {body}", status_code=status)
        elif status == 422:
            raise ValidationError(f"Validation error: {body}", status_code=status)
        else:
            raise AdminError(
                f"Request failed ({status}): {body}", status_code=status
            )
