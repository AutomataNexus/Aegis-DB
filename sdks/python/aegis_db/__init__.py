"""
Aegis Database Python SDK

A Python client library for Aegis Database Platform.

Features:
- Async-first design with aiohttp
- Connection pooling
- Type-safe query builder
- Transaction support
- Streaming results

Example:
    >>> import asyncio
    >>> from aegis_db import AegisClient
    >>>
    >>> async def main():
    ...     async with AegisClient("http://localhost:8080") as client:
    ...         result = await client.query("SELECT * FROM users LIMIT 10")
    ...         for row in result:
    ...             print(row)
    >>>
    >>> asyncio.run(main())

@version 1.0.0
@author AutomataNexus Development Team
"""

from .client import AegisClient
from .query import QueryBuilder
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
from .transaction import Transaction

__version__ = "1.0.0"
__all__ = [
    "AegisClient",
    "QueryBuilder",
    "Transaction",
    "Row",
    "QueryResult",
    "TableInfo",
    "ColumnInfo",
    "DatabaseInfo",
    "AegisError",
    "ConnectionError",
    "QueryError",
    "AuthenticationError",
]
