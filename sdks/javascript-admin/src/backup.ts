/**
 * Backup service for creating, listing, restoring, and deleting backups.
 */

import type { RequestFn } from './index';

// ============================================================================
// Types
// ============================================================================

export interface Backup {
  id: string;
  name?: string;
  status: string;
  size_bytes?: number;
  created_at: string;
  completed_at?: string;
  tables?: string[];
  node_id?: string;
}

export interface CreateBackupRequest {
  name?: string;
  tables?: string[];
  compression?: string;
}

export interface RestoreRequest {
  backup_id: string;
  target_database?: string;
  tables?: string[];
}

export interface RestoreResult {
  message: string;
  backup_id: string;
  restored_tables?: string[];
  duration_ms?: number;
}

export interface BackupListResponse {
  backups: Backup[];
  total: number;
}

// ============================================================================
// Service
// ============================================================================

export class BackupService {
  constructor(private request: RequestFn) {}

  /** Create a new backup. */
  async create(options?: CreateBackupRequest): Promise<Backup> {
    return this.request<Backup>('POST', '/api/v1/admin/backup', options ?? {});
  }

  /** List all backups. */
  async list(): Promise<BackupListResponse> {
    return this.request<BackupListResponse>('GET', '/api/v1/admin/backups');
  }

  /** Restore from a backup. */
  async restore(options: RestoreRequest): Promise<RestoreResult> {
    return this.request<RestoreResult>('POST', '/api/v1/admin/restore', options);
  }

  /** Delete a backup. */
  async delete(backupId: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/admin/backup/${encodeURIComponent(backupId)}`);
  }
}
