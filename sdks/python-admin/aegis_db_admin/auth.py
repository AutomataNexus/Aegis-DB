"""
Aegis-DB Admin SDK - Auth Services

User and role management.

@version 1.0.0
@author AutomataNexus Development Team
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .types import User, Role

if TYPE_CHECKING:
    from .client import AegisAdmin


class UserService:
    """Manage users via the admin API."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    async def list_users(self) -> List[User]:
        """List all users.

        Returns:
            List of User objects.
        """
        data = await self._admin._request("GET", "/api/v1/admin/users")
        users_list = data if isinstance(data, list) else data.get("users", [])
        return [_parse_user(u) for u in users_list]

    async def create_user(
        self,
        username: str,
        password: str,
        *,
        email: Optional[str] = None,
        role: Optional[str] = None,
    ) -> User:
        """Create a new user.

        Args:
            username: The username.
            password: The password.
            email: Optional email address.
            role: Optional role name to assign.

        Returns:
            The created User.
        """
        payload: Dict[str, Any] = {
            "username": username,
            "password": password,
        }
        if email is not None:
            payload["email"] = email
        if role is not None:
            payload["role"] = role

        data = await self._admin._request("POST", "/api/v1/admin/users", payload)
        return _parse_user(data)

    async def update_user(
        self,
        username: str,
        *,
        email: Optional[str] = None,
        role: Optional[str] = None,
        enabled: Optional[bool] = None,
        password: Optional[str] = None,
    ) -> User:
        """Update an existing user.

        Args:
            username: The username to update.
            email: New email address.
            role: New role.
            enabled: Enable or disable the account.
            password: New password.

        Returns:
            The updated User.
        """
        payload: Dict[str, Any] = {}
        if email is not None:
            payload["email"] = email
        if role is not None:
            payload["role"] = role
        if enabled is not None:
            payload["enabled"] = enabled
        if password is not None:
            payload["password"] = password

        data = await self._admin._request(
            "PUT", f"/api/v1/admin/users/{username}", payload
        )
        return _parse_user(data)

    async def delete_user(self, username: str) -> Dict[str, Any]:
        """Delete a user.

        Args:
            username: The username to delete.

        Returns:
            Server response dict.
        """
        return await self._admin._request(
            "DELETE", f"/api/v1/admin/users/{username}"
        )


class RoleService:
    """Manage roles via the admin API."""

    def __init__(self, admin: AegisAdmin) -> None:
        self._admin = admin

    async def list_roles(self) -> List[Role]:
        """List all roles.

        Returns:
            List of Role objects.
        """
        data = await self._admin._request("GET", "/api/v1/admin/roles")
        roles_list = data if isinstance(data, list) else data.get("roles", [])
        return [_parse_role(r) for r in roles_list]

    async def create_role(
        self,
        name: str,
        *,
        description: Optional[str] = None,
        permissions: Optional[List[str]] = None,
    ) -> Role:
        """Create a new role.

        Args:
            name: Role name.
            description: Role description.
            permissions: List of permission strings.

        Returns:
            The created Role.
        """
        payload: Dict[str, Any] = {"name": name}
        if description is not None:
            payload["description"] = description
        if permissions is not None:
            payload["permissions"] = permissions

        data = await self._admin._request("POST", "/api/v1/admin/roles", payload)
        return _parse_role(data)

    async def delete_role(self, name: str) -> Dict[str, Any]:
        """Delete a role.

        Args:
            name: Role name to delete.

        Returns:
            Server response dict.
        """
        return await self._admin._request("DELETE", f"/api/v1/admin/roles/{name}")


# =============================================================================
# Helpers
# =============================================================================

def _parse_user(data: Dict[str, Any]) -> User:
    return User(
        username=data.get("username", ""),
        email=data.get("email"),
        role=data.get("role"),
        enabled=data.get("enabled", True),
        created_at=data.get("created_at"),
        updated_at=data.get("updated_at"),
        last_login=data.get("last_login"),
        mfa_enabled=data.get("mfa_enabled", False),
    )


def _parse_role(data: Dict[str, Any]) -> Role:
    return Role(
        name=data.get("name", ""),
        description=data.get("description"),
        permissions=data.get("permissions", []),
        created_at=data.get("created_at"),
    )
