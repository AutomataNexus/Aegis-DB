/**
 * Auth service for managing users and roles.
 */

import type { RequestFn } from './index';

// ============================================================================
// Types
// ============================================================================

export interface User {
  username: string;
  email?: string;
  roles: string[];
  enabled: boolean;
  created_at: string;
  updated_at: string;
  last_login?: string;
  mfa_enabled?: boolean;
}

export interface CreateUserRequest {
  username: string;
  password: string;
  email?: string;
  roles?: string[];
  enabled?: boolean;
}

export interface UpdateUserRequest {
  email?: string;
  password?: string;
  roles?: string[];
  enabled?: boolean;
  mfa_enabled?: boolean;
}

export interface Role {
  name: string;
  permissions: string[];
  description?: string;
  created_at: string;
}

export interface CreateRoleRequest {
  name: string;
  permissions: string[];
  description?: string;
}

export interface UserListResponse {
  users: User[];
  total: number;
}

export interface RoleListResponse {
  roles: Role[];
}

// ============================================================================
// Service
// ============================================================================

export class AuthService {
  constructor(private request: RequestFn) {}

  /** List all users. */
  async listUsers(): Promise<UserListResponse> {
    return this.request<UserListResponse>('GET', '/api/v1/admin/users');
  }

  /** Create a new user. */
  async createUser(user: CreateUserRequest): Promise<User> {
    return this.request<User>('POST', '/api/v1/admin/users', user);
  }

  /** Update an existing user. */
  async updateUser(username: string, updates: UpdateUserRequest): Promise<User> {
    return this.request<User>('PUT', `/api/v1/admin/users/${encodeURIComponent(username)}`, updates);
  }

  /** Delete a user. */
  async deleteUser(username: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/admin/users/${encodeURIComponent(username)}`);
  }

  /** List all roles. */
  async listRoles(): Promise<RoleListResponse> {
    return this.request<RoleListResponse>('GET', '/api/v1/admin/roles');
  }

  /** Create a new role. */
  async createRole(role: CreateRoleRequest): Promise<Role> {
    return this.request<Role>('POST', '/api/v1/admin/roles', role);
  }

  /** Delete a role. */
  async deleteRole(name: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/admin/roles/${encodeURIComponent(name)}`);
  }
}
