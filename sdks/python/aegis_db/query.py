"""
Aegis Database Query Builder

Type-safe, fluent query builder for constructing SQL queries.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING, Union

if TYPE_CHECKING:
    from .client import AegisClient
    from .types import QueryResult


class QueryBuilder:
    """
    Fluent query builder for Aegis Database.

    Example:
        result = await client.query_builder("users") \\
            .select("id", "name", "email") \\
            .where("active", "=", True) \\
            .where("age", ">=", 18) \\
            .order_by("name") \\
            .limit(10) \\
            .execute()
    """

    def __init__(self, client: "AegisClient", table: str):
        self._client = client
        self._table = table
        self._select_cols: List[str] = ["*"]
        self._where_clauses: List[str] = []
        # Positional ($1, $2, ...) parameter values in order, matching the server.
        self._params: List[Any] = []
        self._order_by_cols: List[str] = []
        self._group_by_cols: List[str] = []
        self._having_clauses: List[str] = []
        self._limit_val: Optional[int] = None
        self._offset_val: Optional[int] = None
        self._joins: List[str] = []

    def _placeholder(self, value: Any) -> str:
        """Append a positional parameter value and return its ``$N`` placeholder."""
        self._params.append(value)
        return f"${len(self._params)}"

    def select(self, *columns: str) -> "QueryBuilder":
        """Select specific columns."""
        self._select_cols = list(columns) if columns else ["*"]
        return self

    def where(
        self,
        column: str,
        operator: str,
        value: Any,
    ) -> "QueryBuilder":
        """Add a WHERE condition."""
        self._where_clauses.append(f"{column} {operator} {self._placeholder(value)}")
        return self

    def where_in(self, column: str, values: List[Any]) -> "QueryBuilder":
        """Add a WHERE IN condition."""
        placeholders = [self._placeholder(val) for val in values]
        self._where_clauses.append(f"{column} IN ({', '.join(placeholders)})")
        return self

    def where_null(self, column: str) -> "QueryBuilder":
        """Add a WHERE IS NULL condition."""
        self._where_clauses.append(f"{column} IS NULL")
        return self

    def where_not_null(self, column: str) -> "QueryBuilder":
        """Add a WHERE IS NOT NULL condition."""
        self._where_clauses.append(f"{column} IS NOT NULL")
        return self

    def where_between(
        self,
        column: str,
        low: Any,
        high: Any,
    ) -> "QueryBuilder":
        """Add a WHERE BETWEEN condition."""
        self._where_clauses.append(
            f"{column} BETWEEN {self._placeholder(low)} AND {self._placeholder(high)}"
        )
        return self

    def where_like(self, column: str, pattern: str) -> "QueryBuilder":
        """Add a WHERE LIKE condition."""
        self._where_clauses.append(f"{column} LIKE {self._placeholder(pattern)}")
        return self

    def or_where(
        self,
        column: str,
        operator: str,
        value: Any,
    ) -> "QueryBuilder":
        """Add an OR WHERE condition."""
        placeholder = self._placeholder(value)
        if self._where_clauses:
            last = self._where_clauses.pop()
            self._where_clauses.append(f"({last} OR {column} {operator} {placeholder})")
        else:
            self._where_clauses.append(f"{column} {operator} {placeholder}")
        return self

    def join(
        self,
        table: str,
        on: str,
        join_type: str = "INNER",
    ) -> "QueryBuilder":
        """Add a JOIN clause."""
        self._joins.append(f"{join_type} JOIN {table} ON {on}")
        return self

    def left_join(self, table: str, on: str) -> "QueryBuilder":
        """Add a LEFT JOIN clause."""
        return self.join(table, on, "LEFT")

    def right_join(self, table: str, on: str) -> "QueryBuilder":
        """Add a RIGHT JOIN clause."""
        return self.join(table, on, "RIGHT")

    def order_by(self, column: str, direction: str = "ASC") -> "QueryBuilder":
        """Add an ORDER BY clause."""
        self._order_by_cols.append(f"{column} {direction.upper()}")
        return self

    def group_by(self, *columns: str) -> "QueryBuilder":
        """Add a GROUP BY clause."""
        self._group_by_cols.extend(columns)
        return self

    def having(
        self,
        column: str,
        operator: str,
        value: Any,
    ) -> "QueryBuilder":
        """Add a HAVING clause."""
        self._having_clauses.append(f"{column} {operator} {self._placeholder(value)}")
        return self

    def limit(self, count: int) -> "QueryBuilder":
        """Set the LIMIT."""
        self._limit_val = count
        return self

    def offset(self, count: int) -> "QueryBuilder":
        """Set the OFFSET."""
        self._offset_val = count
        return self

    def build(self) -> tuple[str, List[Any]]:
        """Build the SQL query and positional parameters."""
        parts = [f"SELECT {', '.join(self._select_cols)} FROM {self._table}"]

        if self._joins:
            parts.extend(self._joins)

        if self._where_clauses:
            parts.append(f"WHERE {' AND '.join(self._where_clauses)}")

        if self._group_by_cols:
            parts.append(f"GROUP BY {', '.join(self._group_by_cols)}")

        if self._having_clauses:
            parts.append(f"HAVING {' AND '.join(self._having_clauses)}")

        if self._order_by_cols:
            parts.append(f"ORDER BY {', '.join(self._order_by_cols)}")

        if self._limit_val is not None:
            parts.append(f"LIMIT {self._limit_val}")

        if self._offset_val is not None:
            parts.append(f"OFFSET {self._offset_val}")

        return " ".join(parts), self._params

    async def execute(self) -> "QueryResult":
        """Execute the built query."""
        sql, params = self.build()
        return await self._client.query(sql, params)

    async def first(self) -> Optional[Any]:
        """Execute and return the first row."""
        self._limit_val = 1
        result = await self.execute()
        return result.rows[0] if result.rows else None

    async def count(self) -> int:
        """Execute a COUNT query."""
        self._select_cols = ["COUNT(*) as count"]
        result = await self.execute()
        if result.rows:
            return result.rows[0]["count"]
        return 0

    async def exists(self) -> bool:
        """Check if any rows match."""
        return await self.count() > 0

    async def pluck(self, column: str) -> List[Any]:
        """Get a list of values from a single column."""
        self._select_cols = [column]
        result = await self.execute()
        return [row[column] for row in result.rows]

    def __repr__(self) -> str:
        sql, params = self.build()
        return f"QueryBuilder({sql!r}, params={params})"
