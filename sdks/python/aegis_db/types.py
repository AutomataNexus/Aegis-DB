"""
Aegis Database Type Definitions

@version 1.0.0
@author AutomataNexus Development Team
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


class AegisError(Exception):
    """Base exception for Aegis client errors."""
    pass


class ConnectionError(AegisError):
    """Connection-related errors."""
    pass


class QueryError(AegisError):
    """Query execution errors."""
    pass


class AuthenticationError(AegisError):
    """Authentication-related errors."""
    pass


@dataclass
class Row:
    """
    A single row from a query result.

    Supports both dict-like and attribute access.
    """
    _data: Dict[str, Any]

    def __getitem__(self, key: str) -> Any:
        return self._data[key]

    def __getattr__(self, name: str) -> Any:
        if name.startswith("_"):
            raise AttributeError(name)
        return self._data.get(name)

    def __iter__(self):
        return iter(self._data.values())

    def keys(self):
        return self._data.keys()

    def values(self):
        return self._data.values()

    def items(self):
        return self._data.items()

    def to_dict(self) -> Dict[str, Any]:
        return self._data.copy()

    def __repr__(self) -> str:
        return f"Row({self._data})"


@dataclass
class QueryResult:
    """Result of a query execution."""
    columns: List[str]
    rows: List[Row]
    rows_affected: int = 0
    execution_time_ms: int = 0

    def __iter__(self):
        return iter(self.rows)

    def __len__(self):
        return len(self.rows)

    def __getitem__(self, index: int) -> Row:
        return self.rows[index]

    def to_dicts(self) -> List[Dict[str, Any]]:
        """Convert all rows to dictionaries."""
        return [row.to_dict() for row in self.rows]

    def to_dataframe(self):
        """Convert to pandas DataFrame (requires pandas)."""
        try:
            import pandas as pd
            return pd.DataFrame(self.to_dicts())
        except ImportError:
            raise ImportError("pandas is required for to_dataframe()")


@dataclass
class ColumnInfo:
    """Information about a table column."""
    name: str
    data_type: str
    nullable: bool = True
    primary_key: bool = False
    default: Optional[Any] = None


@dataclass
class TableInfo:
    """Information about a database table."""
    name: str
    columns: List[ColumnInfo] = field(default_factory=list)
    row_count: int = 0
    size_bytes: int = 0
    indexes: List[str] = field(default_factory=list)

    def __post_init__(self):
        if self.columns and isinstance(self.columns[0], dict):
            self.columns = [ColumnInfo(**c) for c in self.columns]


@dataclass
class DatabaseInfo:
    """Information about a database."""
    name: str
    tables: List[str] = field(default_factory=list)
    size_bytes: int = 0
    created_at: Optional[str] = None
