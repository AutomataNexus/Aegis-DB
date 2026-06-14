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
            elif method == "PUT":
                async with self._session.put(url, headers=headers, json=data) as resp:
                    return await self._handle_response(resp)
            elif method == "PATCH":
                async with self._session.patch(url, headers=headers, json=data) as resp:
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
        params: Optional[List[Any]] = None,
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
            "sql": sql,
            "database": self.config.database,
            "params": params or [],
        }

        resp = await self._request("POST", "/api/v1/query", payload)
        data = resp.get("data") or {}

        rows = [Row(dict(zip(data.get("columns", []), row))) for row in data.get("rows", [])]

        return QueryResult(
            columns=data.get("columns", []),
            rows=rows,
            rows_affected=data.get("rows_affected", 0),
            execution_time_ms=resp.get("execution_time_ms", 0),
        )

    async def execute(
        self,
        sql: str,
        params: Optional[List[Any]] = None,
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

    async def prepare(self, sql: str) -> str:
        """Prepare a statement; returns its id for repeated execution."""
        resp = await self._request(
            "POST",
            "/api/v1/prepare",
            {"sql": sql, "database": self.config.database},
        )
        return resp.get("statement_id", "")

    async def execute_prepared(
        self, statement_id: str, params: Optional[List[Any]] = None
    ) -> QueryResult:
        """Execute a prepared statement with bound positional parameters."""
        resp = await self._request(
            "POST",
            "/api/v1/prepared/execute",
            {"statement_id": statement_id, "params": params or []},
        )
        data = resp.get("data") or {}
        rows = [Row(dict(zip(data.get("columns", []), row))) for row in data.get("rows", [])]
        return QueryResult(
            columns=data.get("columns", []),
            rows=rows,
            rows_affected=data.get("rows_affected", 0),
            execution_time_ms=resp.get("execution_time_ms", 0),
        )

    async def deallocate(self, statement_id: str) -> bool:
        """Deallocate a prepared statement."""
        try:
            await self._request("DELETE", f"/api/v1/prepared/{statement_id}")
            return True
        except QueryError:
            return False

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

    async def kv_batch_get(self, keys: List[str]) -> List[Dict[str, Any]]:
        """Get many keys at once (missing keys are omitted)."""
        data = await self._request("POST", "/api/v1/kv/batch/get", {"keys": keys})
        return data.get("entries", [])

    async def kv_batch_set(self, entries: List[Dict[str, Any]]) -> int:
        """Set many keys at once.

        Each entry is a dict ``{"key": ..., "value": ..., "ttl": optional}``.
        Returns the number of keys written.
        """
        data = await self._request("POST", "/api/v1/kv/batch/set", {"entries": entries})
        return data.get("count", 0)

    async def kv_batch_delete(self, keys: List[str]) -> int:
        """Delete many keys at once. Returns the number deleted."""
        data = await self._request("POST", "/api/v1/kv/batch/delete", {"keys": keys})
        return data.get("deleted", 0)

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

    async def bulk_insert(self, collection: str, documents: List[Any]) -> List[str]:
        """Insert many documents into a collection in one call. Returns new ids."""
        data = await self._request(
            "POST",
            f"/api/v1/documents/collections/{collection}/batch-insert",
            {"documents": documents},
        )
        return data.get("ids", [])

    async def bulk_delete(self, collection: str, ids: List[str]) -> int:
        """Delete many documents by id in one call. Returns the number deleted."""
        data = await self._request(
            "POST",
            f"/api/v1/documents/collections/{collection}/batch-delete",
            {"ids": ids},
        )
        return data.get("deleted", 0)

    async def create_collection(self, name: str) -> Dict[str, Any]:
        """Create a document collection."""
        return await self._request(
            "POST", "/api/v1/documents/collections", {"name": name}
        )

    async def insert_document(
        self, collection: str, document: Any, doc_id: Optional[str] = None
    ) -> str:
        """Insert a single document, optionally with an explicit id. Returns the new id."""
        data = await self._request(
            "POST",
            f"/api/v1/documents/collections/{collection}/documents",
            {"id": doc_id, "document": document},
        )
        return data.get("id", "")

    async def get_document(self, collection: str, doc_id: str) -> Optional[Any]:
        """Get a document by id, or ``None`` if absent."""
        try:
            return await self._request(
                "GET",
                f"/api/v1/documents/collections/{collection}/documents/{doc_id}",
            )
        except QueryError:
            return None

    async def update_document(
        self, collection: str, doc_id: str, document: Any
    ) -> Dict[str, Any]:
        """Replace a document (full update)."""
        return await self._request(
            "PUT",
            f"/api/v1/documents/collections/{collection}/documents/{doc_id}",
            {"document": document},
        )

    async def patch_document(
        self, collection: str, doc_id: str, partial: Any
    ) -> Dict[str, Any]:
        """Partially update (merge) a document."""
        return await self._request(
            "PATCH",
            f"/api/v1/documents/collections/{collection}/documents/{doc_id}",
            {"document": partial},
        )

    async def delete_document(self, collection: str, doc_id: str) -> bool:
        """Delete a document by id."""
        try:
            await self._request(
                "DELETE",
                f"/api/v1/documents/collections/{collection}/documents/{doc_id}",
            )
            return True
        except QueryError:
            return False

    async def query_documents(
        self,
        collection: str,
        filter: Optional[Dict[str, Any]] = None,
        limit: Optional[int] = None,
        skip: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Query documents with a MongoDB-style filter.

        Pass ``cursor`` (from a prior response's ``next_cursor``) for pagination;
        the response includes ``next_cursor`` when more pages exist.
        """
        return await self._request(
            "POST",
            f"/api/v1/documents/collections/{collection}/query",
            {"filter": filter or {}, "limit": limit, "skip": skip, "cursor": cursor},
        )

    # =========================================================================
    # Time Series Methods
    # =========================================================================

    async def register_metric(self, name: str, metric_type: str = "gauge") -> Dict[str, Any]:
        """Register a metric (counter / gauge / histogram / summary)."""
        return await self._request(
            "POST",
            "/api/v1/timeseries/metrics",
            {"name": name, "metric_type": metric_type},
        )

    async def ts_write(
        self,
        metric: str,
        value: float,
        timestamp: Optional[int] = None,
        tags: Optional[Dict[str, str]] = None,
    ) -> None:
        """Write a single time-series point."""
        await self._request(
            "POST",
            "/api/v1/timeseries/write",
            {"metric": metric, "value": value, "timestamp": timestamp, "tags": tags or {}},
        )

    async def ts_query(
        self,
        metric: str,
        start: Optional[int] = None,
        end: Optional[int] = None,
        limit: Optional[int] = None,
        tags: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """Query a time series within an optional ``[start, end]`` window."""
        return await self._request(
            "POST",
            "/api/v1/timeseries/query",
            {"metric": metric, "tags": tags, "start": start, "end": end, "limit": limit},
        )

    # =========================================================================
    # Graph Methods
    # =========================================================================

    async def get_graph_data(self) -> Dict[str, Any]:
        """Get graph nodes and edges."""
        return await self._request("GET", "/api/v1/graph/data")

    async def create_node(
        self, label: str, properties: Optional[Dict[str, Any]] = None
    ) -> Dict[str, Any]:
        """Create a graph node."""
        return await self._request(
            "POST", "/api/v1/graph/nodes", {"label": label, "properties": properties or {}}
        )

    async def update_node(
        self,
        node_id: str,
        label: Optional[str] = None,
        properties: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Update a graph node (omit a field to leave it unchanged)."""
        return await self._request(
            "PUT",
            f"/api/v1/graph/nodes/{node_id}",
            {"label": label, "properties": properties},
        )

    async def delete_node(self, node_id: str) -> bool:
        """Delete a graph node (and its edges)."""
        try:
            await self._request("DELETE", f"/api/v1/graph/nodes/{node_id}")
            return True
        except QueryError:
            return False

    async def create_edge(
        self, source: str, target: str, relationship: str
    ) -> Dict[str, Any]:
        """Create a graph edge."""
        return await self._request(
            "POST",
            "/api/v1/graph/edges",
            {"source": source, "target": target, "relationship": relationship},
        )

    async def update_edge(self, edge_id: str, relationship: str) -> Dict[str, Any]:
        """Update a graph edge's relationship."""
        return await self._request(
            "PUT", f"/api/v1/graph/edges/{edge_id}", {"relationship": relationship}
        )

    async def delete_edge(self, edge_id: str) -> bool:
        """Delete a graph edge."""
        try:
            await self._request("DELETE", f"/api/v1/graph/edges/{edge_id}")
            return True
        except QueryError:
            return False

    # =========================================================================
    # Streaming (Server-Sent Events)
    # =========================================================================

    async def subscribe_channel(self, channel: str) -> AsyncIterator[Any]:
        """Subscribe to a streaming channel as an async iterator of events (SSE).

        The channel is created on the server if it does not exist. Iteration
        ends when the response stream closes; ``break`` out of the loop to
        disconnect::

            async for event in client.subscribe_channel("cdc"):
                ...
        """
        if self._session is None:
            raise ConnectionError("Client not connected. Call connect() first.")

        url = f"{self.config.url}/api/v1/streaming/channels/{channel}/sse"
        headers = self._headers()
        headers["Accept"] = "text/event-stream"

        async with self._session.get(url, headers=headers) as resp:
            if resp.status != 200:
                raise QueryError(f"Subscribe failed ({resp.status})")
            async for raw in resp.content:
                line = raw.decode("utf-8").rstrip("\r\n")
                if line.startswith("data:"):
                    data = line[5:].strip()
                    try:
                        yield json.loads(data)
                    except (ValueError, TypeError):
                        yield data

    # =========================================================================
    # Vector / KNN Methods
    # =========================================================================

    async def create_vector_collection(
        self, name: str, dim: int, metric: str = "cosine"
    ) -> Dict[str, Any]:
        """Create a vector collection (``metric``: cosine / l2 / dot)."""
        return await self._request(
            "POST",
            "/api/v1/vector/collections",
            {"name": name, "dim": dim, "metric": metric},
        )

    async def list_vector_collections(self) -> List[str]:
        """List vector collections."""
        data = await self._request("GET", "/api/v1/vector/collections")
        return data.get("collections", [])

    async def vector_collection_stats(self, name: str) -> Dict[str, Any]:
        """Stats for a vector collection (dim, metric, count)."""
        return await self._request("GET", f"/api/v1/vector/collections/{name}")

    async def drop_vector_collection(self, name: str) -> bool:
        """Drop a vector collection."""
        try:
            await self._request("DELETE", f"/api/v1/vector/collections/{name}")
            return True
        except QueryError:
            return False

    async def vector_upsert(
        self,
        collection: str,
        id: str,
        vector: List[float],
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Upsert a single vector with optional metadata."""
        await self._request(
            "POST",
            f"/api/v1/vector/collections/{collection}/upsert",
            {"id": id, "vector": vector, "metadata": metadata or {}},
        )

    async def vector_upsert_batch(
        self, collection: str, vectors: List[Dict[str, Any]]
    ) -> int:
        """Batch-upsert vectors (each ``{"id", "vector", "metadata"?}``). Returns count."""
        data = await self._request(
            "POST",
            f"/api/v1/vector/collections/{collection}/batch",
            {"vectors": vectors},
        )
        return data.get("count", 0)

    async def get_vector(self, collection: str, id: str) -> Optional[Dict[str, Any]]:
        """Get a stored vector by id, or ``None`` if absent."""
        try:
            return await self._request(
                "GET", f"/api/v1/vector/collections/{collection}/vectors/{id}"
            )
        except QueryError:
            return None

    async def delete_vector(self, collection: str, id: str) -> bool:
        """Delete a vector by id."""
        try:
            await self._request(
                "DELETE", f"/api/v1/vector/collections/{collection}/vectors/{id}"
            )
            return True
        except QueryError:
            return False

    async def vector_search(
        self,
        collection: str,
        query: List[float],
        k: int = 10,
        ef: Optional[int] = None,
        filter: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """KNN search; returns ``{"hits": [...], "count": n}`` ranked by score."""
        return await self._request(
            "POST",
            f"/api/v1/vector/collections/{collection}/search",
            {"vector": query, "k": k, "ef": ef, "filter": filter or {}},
        )

    # =========================================================================
    # Full-Text Search (BM25)
    # =========================================================================

    async def create_fts_index(self, name: str) -> Dict[str, Any]:
        """Create a full-text (BM25) index."""
        return await self._request("POST", "/api/v1/fts/indexes", {"name": name})

    async def list_fts_indexes(self) -> List[str]:
        """List full-text indexes."""
        data = await self._request("GET", "/api/v1/fts/indexes")
        return data.get("indexes", [])

    async def fts_index_stats(self, name: str) -> Dict[str, Any]:
        """Full-text index stats."""
        return await self._request("GET", f"/api/v1/fts/indexes/{name}")

    async def drop_fts_index(self, name: str) -> bool:
        """Drop a full-text index."""
        try:
            await self._request("DELETE", f"/api/v1/fts/indexes/{name}")
            return True
        except QueryError:
            return False

    async def fts_index_document(
        self,
        index: str,
        id: str,
        text: str,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Index (insert or replace) a document with optional metadata."""
        await self._request(
            "POST",
            f"/api/v1/fts/indexes/{index}/documents",
            {"id": id, "text": text, "metadata": metadata or {}},
        )

    async def fts_get_document(self, index: str, id: str) -> Optional[Dict[str, Any]]:
        """Get an indexed document by id, or ``None`` if absent."""
        try:
            return await self._request(
                "GET", f"/api/v1/fts/indexes/{index}/documents/{id}"
            )
        except QueryError:
            return None

    async def fts_delete_document(self, index: str, id: str) -> bool:
        """Delete a document from a full-text index."""
        try:
            await self._request(
                "DELETE", f"/api/v1/fts/indexes/{index}/documents/{id}"
            )
            return True
        except QueryError:
            return False

    async def fts_search(
        self,
        index: str,
        query: str,
        k: int = 10,
        filter: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """BM25 search; returns ``{"hits": [...], "count": n}`` ranked by score."""
        return await self._request(
            "POST",
            f"/api/v1/fts/indexes/{index}/search",
            {"query": query, "k": k, "filter": filter or {}},
        )

    # =========================================================================
    # Geospatial (grid index + Haversine)
    # =========================================================================

    async def create_geo_collection(self, name: str) -> Dict[str, Any]:
        """Create a geo collection."""
        return await self._request("POST", "/api/v1/geo/collections", {"name": name})

    async def list_geo_collections(self) -> List[str]:
        """List geo collections."""
        data = await self._request("GET", "/api/v1/geo/collections")
        return data.get("collections", [])

    async def geo_collection_stats(self, name: str) -> Dict[str, Any]:
        """Geo collection stats."""
        return await self._request("GET", f"/api/v1/geo/collections/{name}")

    async def drop_geo_collection(self, name: str) -> bool:
        """Drop a geo collection."""
        try:
            await self._request("DELETE", f"/api/v1/geo/collections/{name}")
            return True
        except QueryError:
            return False

    async def geo_upsert_feature(
        self,
        collection: str,
        id: str,
        lat: float,
        lon: float,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Upsert a feature ``(id, lat, lon)`` with optional metadata."""
        await self._request(
            "POST",
            f"/api/v1/geo/collections/{collection}/features",
            {"id": id, "lat": lat, "lon": lon, "metadata": metadata or {}},
        )

    async def geo_get_feature(
        self, collection: str, id: str
    ) -> Optional[Dict[str, Any]]:
        """Get a feature by id, or ``None`` if absent."""
        try:
            return await self._request(
                "GET", f"/api/v1/geo/collections/{collection}/features/{id}"
            )
        except QueryError:
            return None

    async def geo_delete_feature(self, collection: str, id: str) -> bool:
        """Delete a feature by id."""
        try:
            await self._request(
                "DELETE", f"/api/v1/geo/collections/{collection}/features/{id}"
            )
            return True
        except QueryError:
            return False

    async def geo_radius(
        self,
        collection: str,
        lat: float,
        lon: float,
        radius_m: float,
        filter: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Features within ``radius_m`` metres of ``(lat, lon)``, nearest first."""
        return await self._request(
            "POST",
            f"/api/v1/geo/collections/{collection}/radius",
            {"lat": lat, "lon": lon, "radius_m": radius_m, "filter": filter or {}},
        )

    async def geo_bbox(
        self,
        collection: str,
        min_lat: float,
        min_lon: float,
        max_lat: float,
        max_lon: float,
        filter: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Features inside a bounding box."""
        return await self._request(
            "POST",
            f"/api/v1/geo/collections/{collection}/bbox",
            {
                "min_lat": min_lat,
                "min_lon": min_lon,
                "max_lat": max_lat,
                "max_lon": max_lon,
                "filter": filter or {},
            },
        )

    async def geo_nearest(
        self,
        collection: str,
        lat: float,
        lon: float,
        k: int = 10,
        filter: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """The ``k`` nearest features to ``(lat, lon)``."""
        return await self._request(
            "POST",
            f"/api/v1/geo/collections/{collection}/nearest",
            {"lat": lat, "lon": lon, "k": k, "filter": filter or {}},
        )

    # =========================================================================
    # Columnar / OLAP (column-major store + group-by aggregation)
    # =========================================================================

    async def create_columnar_table(
        self, name: str, columns: List[Dict[str, str]]
    ) -> Dict[str, Any]:
        """Create a columnar table. ``columns`` is ``[{"name", "type"}]`` where
        type is one of ``int`` / ``float`` / ``text`` / ``bool``."""
        return await self._request(
            "POST", "/api/v1/columnar/tables", {"name": name, "columns": columns}
        )

    async def list_columnar_tables(self) -> List[str]:
        """List columnar tables."""
        data = await self._request("GET", "/api/v1/columnar/tables")
        return data.get("tables", [])

    async def columnar_table_stats(self, name: str) -> Dict[str, Any]:
        """Columnar table stats (row count + schema)."""
        return await self._request("GET", f"/api/v1/columnar/tables/{name}")

    async def drop_columnar_table(self, name: str) -> bool:
        """Drop a columnar table."""
        try:
            await self._request("DELETE", f"/api/v1/columnar/tables/{name}")
            return True
        except QueryError:
            return False

    async def columnar_insert(
        self, table: str, rows: List[Dict[str, Any]]
    ) -> Dict[str, Any]:
        """Insert many rows into a columnar table."""
        return await self._request(
            "POST", f"/api/v1/columnar/tables/{table}/rows", {"rows": rows}
        )

    async def columnar_scan(
        self,
        table: str,
        columns: Optional[List[str]] = None,
        filter: Optional[List[Dict[str, Any]]] = None,
        limit: Optional[int] = None,
    ) -> Dict[str, Any]:
        """Scan rows. ``filter`` is a list of ``{"column", "op", "value"}``
        conditions (ANDed); ``op`` is one of eq/ne/lt/lte/gt/gte."""
        return await self._request(
            "POST",
            f"/api/v1/columnar/tables/{table}/scan",
            {"columns": columns or [], "filter": filter or [], "limit": limit},
        )

    async def columnar_aggregate(
        self,
        table: str,
        aggregates: List[Dict[str, str]],
        group_by: Optional[List[str]] = None,
        filter: Optional[List[Dict[str, Any]]] = None,
    ) -> Dict[str, Any]:
        """Group-by aggregation. ``aggregates`` is ``[{"func", "column"}]`` where
        func is one of count/sum/min/max/avg (column ``"*"`` allowed for count)."""
        return await self._request(
            "POST",
            f"/api/v1/columnar/tables/{table}/aggregate",
            {
                "group_by": group_by or [],
                "aggregates": aggregates,
                "filter": filter or [],
            },
        )

    async def columnar_distinct(self, table: str, column: str) -> Dict[str, Any]:
        """Distinct non-null values of a column."""
        return await self._request(
            "GET", f"/api/v1/columnar/tables/{table}/distinct/{column}"
        )

    # =========================================================================
    # Object / Blob Store (S3-style buckets + content-addressed ETags)
    # =========================================================================

    async def create_bucket(self, name: str) -> Dict[str, Any]:
        """Create an object bucket."""
        return await self._request("POST", "/api/v1/objects/buckets", {"name": name})

    async def list_buckets(self) -> List[str]:
        """List object buckets."""
        data = await self._request("GET", "/api/v1/objects/buckets")
        return data.get("buckets", [])

    async def bucket_stats(self, name: str) -> Dict[str, Any]:
        """Bucket stats (object count + total bytes)."""
        return await self._request("GET", f"/api/v1/objects/buckets/{name}")

    async def drop_bucket(self, name: str) -> bool:
        """Drop an object bucket."""
        try:
            await self._request("DELETE", f"/api/v1/objects/buckets/{name}")
            return True
        except QueryError:
            return False

    async def list_objects(
        self,
        bucket: str,
        prefix: Optional[str] = None,
        limit: Optional[int] = None,
    ) -> Dict[str, Any]:
        """List object metadata in a bucket (optional key prefix + limit)."""
        params = []
        if prefix:
            params.append(f"prefix={prefix}")
        if limit is not None:
            params.append(f"limit={limit}")
        qs = ("?" + "&".join(params)) if params else ""
        return await self._request(
            "GET", f"/api/v1/objects/buckets/{bucket}/objects{qs}"
        )

    async def put_object(
        self,
        bucket: str,
        key: str,
        data: bytes,
        content_type: str = "application/octet-stream",
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """Store (or replace) an object from raw bytes; returns its metadata."""
        if self._session is None:
            raise ConnectionError("Client not connected. Call connect() first.")
        headers = {"Content-Type": content_type}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        if self.config.api_key:
            headers["X-API-Key"] = self.config.api_key
        if metadata is not None:
            headers["X-Aegis-Meta"] = json.dumps(metadata)
        url = f"{self.config.url}/api/v1/objects/buckets/{bucket}/object/{key}"
        async with self._session.put(url, headers=headers, data=data) as resp:
            return await self._handle_response(resp)

    async def get_object(self, bucket: str, key: str) -> Optional[bytes]:
        """Fetch an object's raw bytes, or ``None`` if absent."""
        if self._session is None:
            raise ConnectionError("Client not connected. Call connect() first.")
        headers = {}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        if self.config.api_key:
            headers["X-API-Key"] = self.config.api_key
        url = f"{self.config.url}/api/v1/objects/buckets/{bucket}/object/{key}"
        async with self._session.get(url, headers=headers) as resp:
            if resp.status == 404:
                return None
            if resp.status >= 400:
                raise QueryError(f"get_object failed ({resp.status})")
            return await resp.read()

    async def head_object(self, bucket: str, key: str) -> Optional[Dict[str, Any]]:
        """Fetch an object's metadata only, or ``None`` if absent."""
        try:
            return await self._request(
                "GET", f"/api/v1/objects/buckets/{bucket}/object/{key}?meta=1"
            )
        except QueryError:
            return None

    async def delete_object(self, bucket: str, key: str) -> bool:
        """Delete an object."""
        try:
            await self._request(
                "DELETE", f"/api/v1/objects/buckets/{bucket}/object/{key}"
            )
            return True
        except QueryError:
            return False

    # =========================================================================
    # Wide-Column (row-keyed sparse columns, per-cell timestamps, LWW)
    # =========================================================================

    async def create_wide_table(self, name: str) -> Dict[str, Any]:
        """Create a wide-column table."""
        return await self._request(
            "POST", "/api/v1/widecolumn/tables", {"name": name}
        )

    async def list_wide_tables(self) -> List[str]:
        """List wide-column tables."""
        data = await self._request("GET", "/api/v1/widecolumn/tables")
        return data.get("tables", [])

    async def wide_table_stats(self, name: str) -> Dict[str, Any]:
        """Wide-column table stats (rows + cells)."""
        return await self._request("GET", f"/api/v1/widecolumn/tables/{name}")

    async def drop_wide_table(self, name: str) -> bool:
        """Drop a wide-column table."""
        try:
            await self._request("DELETE", f"/api/v1/widecolumn/tables/{name}")
            return True
        except QueryError:
            return False

    async def wide_put_row(
        self,
        table: str,
        row: str,
        columns: Dict[str, Any],
        timestamp: Optional[int] = None,
    ) -> Dict[str, Any]:
        """Set columns on a row (last-write-wins; optional explicit timestamp)."""
        return await self._request(
            "PUT",
            f"/api/v1/widecolumn/tables/{table}/rows/{row}",
            {"columns": columns, "timestamp": timestamp},
        )

    async def wide_get_row(
        self,
        table: str,
        row: str,
        columns: Optional[List[str]] = None,
    ) -> Optional[Dict[str, Any]]:
        """Get a row, optionally projecting a subset of columns; ``None`` if absent."""
        qs = f"?columns={','.join(columns)}" if columns else ""
        try:
            return await self._request(
                "GET", f"/api/v1/widecolumn/tables/{table}/rows/{row}{qs}"
            )
        except QueryError:
            return None

    async def wide_delete_row(self, table: str, row: str) -> bool:
        """Delete a row."""
        try:
            await self._request(
                "DELETE", f"/api/v1/widecolumn/tables/{table}/rows/{row}"
            )
            return True
        except QueryError:
            return False

    async def wide_delete_cell(self, table: str, row: str, column: str) -> bool:
        """Delete a single column (cell) from a row."""
        try:
            await self._request(
                "DELETE",
                f"/api/v1/widecolumn/tables/{table}/rows/{row}/columns/{column}",
            )
            return True
        except QueryError:
            return False

    async def wide_scan(
        self,
        table: str,
        start: Optional[str] = None,
        end: Optional[str] = None,
        prefix: Optional[str] = None,
        columns: Optional[List[str]] = None,
        limit: Optional[int] = None,
    ) -> Dict[str, Any]:
        """Scan rows in key order (range / prefix / projection / limit)."""
        return await self._request(
            "POST",
            f"/api/v1/widecolumn/tables/{table}/scan",
            {
                "start": start,
                "end": end,
                "prefix": prefix,
                "columns": columns or [],
                "limit": limit,
            },
        )
