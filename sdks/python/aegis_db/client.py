"""
Aegis Database Python Client

Async-first client with connection pooling and comprehensive API support.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

import asyncio
import json
from typing import Any, AsyncIterator, Dict, List, Optional, Union
from contextlib import asynccontextmanager
from dataclasses import dataclass

try:
    import aiohttp
except ImportError:
    aiohttp = None

from .types import (
    Row,
    QueryResult,
    TableInfo,
    ColumnInfo,
    DatabaseInfo,
    AegisError,
    ConnectionError,
    QueryError,
    AuthenticationError,
)
from .query import QueryBuilder
from .transaction import Transaction


@dataclass
class ClientConfig:
    """Configuration for Aegis client."""
    url: str
    database: str = "default"
    username: Optional[str] = None
    password: Optional[str] = None
    api_key: Optional[str] = None
    timeout: float = 30.0
    max_connections: int = 10
    retry_attempts: int = 3
    retry_delay: float = 1.0


class AegisClient:
    """
    Async client for Aegis Database.

    Example:
        async with AegisClient("http://localhost:8080") as client:
            result = await client.query("SELECT * FROM users")
            for row in result:
                print(row)
    """

    def __init__(
        self,
        url: str,
        *,
        database: str = "default",
        username: Optional[str] = None,
        password: Optional[str] = None,
        api_key: Optional[str] = None,
        timeout: float = 30.0,
        max_connections: int = 10,
    ):
        """
        Initialize Aegis client.

        Args:
            url: Aegis server URL (e.g., "http://localhost:8080")
            database: Default database name
            username: Optional username for authentication
            password: Optional password for authentication
            api_key: Optional API key for authentication
            timeout: Request timeout in seconds
            max_connections: Maximum concurrent connections
        """
        if aiohttp is None:
            raise ImportError("aiohttp is required. Install with: pip install aiohttp")

        self.config = ClientConfig(
            url=url.rstrip("/"),
            database=database,
            username=username,
            password=password,
            api_key=api_key,
            timeout=timeout,
            max_connections=max_connections,
        )
        self._session: Optional[aiohttp.ClientSession] = None
        self._token: Optional[str] = None

    async def __aenter__(self) -> "AegisClient":
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        """Async context manager exit."""
        await self.close()

    async def connect(self) -> None:
        """Establish connection to Aegis server."""
        if self._session is not None:
            return

        connector = aiohttp.TCPConnector(limit=self.config.max_connections)
        timeout = aiohttp.ClientTimeout(total=self.config.timeout)
        self._session = aiohttp.ClientSession(
            connector=connector,
            timeout=timeout,
        )

        # Authenticate if credentials provided
        if self.config.username and self.config.password:
            await self._authenticate()

    async def _authenticate(self) -> None:
        """Authenticate with username/password."""
        payload = {
            "username": self.config.username,
            "password": self.config.password,
        }

        try:
            async with self._session.post(
                f"{self.config.url}/api/v1/auth/login",
                json=payload,
            ) as resp:
                if resp.status != 200:
                    raise AuthenticationError(f"Authentication failed: {resp.status}")

                data = await resp.json()
                if data.get("error"):
                    raise AuthenticationError(data["error"])

                if data.get("requires_mfa"):
                    raise AuthenticationError("MFA required - use authenticate_mfa()")

                self._token = data.get("token")
        except aiohttp.ClientError as e:
            raise ConnectionError(f"Connection failed: {e}")

    async def authenticate_mfa(self, code: str, temp_token: str) -> None:
        """Complete MFA authentication."""
        payload = {"code": code, "token": temp_token}

        async with self._session.post(
            f"{self.config.url}/api/v1/auth/mfa/verify",
            json=payload,
        ) as resp:
            if resp.status != 200:
                raise AuthenticationError(f"MFA verification failed: {resp.status}")

            data = await resp.json()
            if data.get("error"):
                raise AuthenticationError(data["error"])

            self._token = data.get("token")

    async def close(self) -> None:
        """Close the client connection."""
        if self._session:
            await self._session.close()
            self._session = None
            self._token = None

    def _headers(self) -> Dict[str, str]:
        """Get request headers."""
        headers = {"Content-Type": "application/json"}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        if self.config.api_key:
            headers["X-API-Key"] = self.config.api_key
        return headers

    async def _request(
        self,
        method: str,
        path: str,
        data: Optional[Dict] = None,
    ) -> Dict[str, Any]:
        """Make an HTTP request."""
        if self._session is None:
            raise ConnectionError("Client not connected. Call connect() first.")

        url = f"{self.config.url}{path}"
        headers = self._headers()

        try:
            if method == "GET":
                async with self._session.get(url, headers=headers) as resp:
                    return await self._handle_response(resp)
            elif method == "POST":
                async with self._session.post(url, headers=headers, json=data) as resp:
                    return await self._handle_response(resp)
            elif method == "DELETE":
                async with self._session.delete(url, headers=headers) as resp:
                    return await self._handle_response(resp)
            else:
                raise ValueError(f"Unsupported method: {method}")
        except aiohttp.ClientError as e:
            raise ConnectionError(f"Request failed: {e}")

    async def _handle_response(self, resp: aiohttp.ClientResponse) -> Dict[str, Any]:
        """Handle HTTP response."""
        if resp.status >= 400:
            text = await resp.text()
            if resp.status == 401:
                raise AuthenticationError(f"Unauthorized: {text}")
            elif resp.status == 403:
                raise AuthenticationError(f"Forbidden: {text}")
            else:
                raise QueryError(f"Request failed ({resp.status}): {text}")

        return await resp.json()

    # =========================================================================
    # Query Methods
    # =========================================================================

    async def query(
        self,
        sql: str,
        params: Optional[Dict[str, Any]] = None,
    ) -> QueryResult:
        """
        Execute a SQL query.

        Args:
            sql: SQL query string
            params: Optional query parameters

        Returns:
            QueryResult with rows and metadata
        """
        payload = {
            "query": sql,
            "database": self.config.database,
            "params": params or {},
        }

        data = await self._request("POST", "/api/v1/query", payload)

        rows = [Row(dict(zip(data.get("columns", []), row))) for row in data.get("rows", [])]

        return QueryResult(
            columns=data.get("columns", []),
            rows=rows,
            rows_affected=data.get("rows_affected", 0),
            execution_time_ms=data.get("execution_time_ms", 0),
        )

    async def execute(
        self,
        sql: str,
        params: Optional[Dict[str, Any]] = None,
    ) -> int:
        """
        Execute a SQL statement (INSERT, UPDATE, DELETE).

        Args:
            sql: SQL statement
            params: Optional query parameters

        Returns:
            Number of rows affected
        """
        result = await self.query(sql, params)
        return result.rows_affected

    def query_builder(self, table: str) -> QueryBuilder:
        """
        Create a type-safe query builder.

        Args:
            table: Table name

        Returns:
            QueryBuilder instance
        """
        return QueryBuilder(self, table)

    async def stream_query(
        self,
        sql: str,
        batch_size: int = 1000,
    ) -> AsyncIterator[Row]:
        """
        Stream query results in batches.

        Args:
            sql: SQL query string
            batch_size: Number of rows per batch

        Yields:
            Row objects
        """
        offset = 0
        while True:
            paginated_sql = f"{sql} LIMIT {batch_size} OFFSET {offset}"
            result = await self.query(paginated_sql)

            if not result.rows:
                break

            for row in result.rows:
                yield row

            if len(result.rows) < batch_size:
                break

            offset += batch_size

    # =========================================================================
    # Transaction Methods
    # =========================================================================

    @asynccontextmanager
    async def transaction(self) -> AsyncIterator[Transaction]:
        """
        Start a transaction.

        Example:
            async with client.transaction() as tx:
                await tx.execute("INSERT INTO users (name) VALUES (?)", {"name": "Alice"})
                await tx.execute("INSERT INTO logs (msg) VALUES (?)", {"msg": "User created"})
        """
        tx = Transaction(self)
        try:
            await tx.begin()
            yield tx
            await tx.commit()
        except Exception:
            await tx.rollback()
            raise

    # =========================================================================
    # Schema Methods
    # =========================================================================

    async def list_tables(self) -> List[TableInfo]:
        """List all tables in the database."""
        data = await self._request("GET", "/api/v1/tables")
        return [TableInfo(**t) for t in data.get("tables", [])]

    async def get_table(self, name: str) -> TableInfo:
        """Get information about a specific table."""
        data = await self._request("GET", f"/api/v1/tables/{name}")
        return TableInfo(**data)

    async def list_databases(self) -> List[DatabaseInfo]:
        """List all databases."""
        data = await self._request("GET", "/api/v1/databases")
        return [DatabaseInfo(**d) for d in data.get("databases", [])]

    # =========================================================================
    # Key-Value Methods
    # =========================================================================

    async def kv_get(self, key: str) -> Optional[Any]:
        """Get a value from the KV store."""
        try:
            data = await self._request("GET", f"/api/v1/kv/keys/{key}")
            return data.get("value")
        except QueryError:
            return None

    async def kv_set(self, key: str, value: Any, ttl: Optional[int] = None) -> None:
        """Set a value in the KV store."""
        payload = {"key": key, "value": value}
        if ttl:
            payload["ttl"] = ttl
        await self._request("POST", "/api/v1/kv/keys", payload)

    async def kv_delete(self, key: str) -> bool:
        """Delete a key from the KV store."""
        try:
            await self._request("DELETE", f"/api/v1/kv/keys/{key}")
            return True
        except QueryError:
            return False

    async def kv_list(self) -> List[str]:
        """List all keys in the KV store."""
        data = await self._request("GET", "/api/v1/kv/keys")
        return [entry["key"] for entry in data]

    # =========================================================================
    # Health and Metrics
    # =========================================================================

    async def health(self) -> Dict[str, Any]:
        """Check server health."""
        return await self._request("GET", "/health")

    async def metrics(self) -> Dict[str, Any]:
        """Get server metrics."""
        return await self._request("GET", "/api/v1/metrics")

    # =========================================================================
    # Document Store Methods
    # =========================================================================

    async def list_collections(self) -> List[Dict[str, Any]]:
        """List all document collections."""
        data = await self._request("GET", "/api/v1/documents/collections")
        return data

    async def get_collection(self, name: str) -> List[Dict[str, Any]]:
        """Get documents from a collection."""
        data = await self._request("GET", f"/api/v1/documents/collections/{name}")
        return data

    # =========================================================================
    # Graph Methods
    # =========================================================================

    async def get_graph_data(self) -> Dict[str, Any]:
        """Get graph nodes and edges."""
        return await self._request("GET", "/api/v1/graph/data")
