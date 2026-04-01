/**
 * Vault service for secrets management and transit encryption.
 */

import type { RequestFn } from './index';

// ============================================================================
// Types
// ============================================================================

export interface VaultStatus {
  initialized: boolean;
  sealed: boolean;
  version?: string;
  cluster_name?: string;
  secret_count?: number;
}

export interface Secret {
  key: string;
  value: string;
  version: number;
  created_at: string;
  updated_at: string;
  metadata?: Record<string, string>;
}

export interface SecretListResponse {
  secrets: SecretEntry[];
}

export interface SecretEntry {
  key: string;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface PutSecretRequest {
  value: string;
  metadata?: Record<string, string>;
}

export interface UnsealRequest {
  key: string;
}

export interface UnsealResponse {
  sealed: boolean;
  progress: number;
  threshold: number;
}

export interface TransitEncryptRequest {
  plaintext: string;
  key_name: string;
}

export interface TransitEncryptResponse {
  ciphertext: string;
  key_version: number;
}

export interface TransitDecryptRequest {
  ciphertext: string;
  key_name: string;
}

export interface TransitDecryptResponse {
  plaintext: string;
}

export interface TransitKeyRequest {
  name: string;
  type?: string;
  exportable?: boolean;
}

export interface TransitKey {
  name: string;
  type: string;
  latest_version: number;
  min_decryption_version: number;
  exportable: boolean;
  created_at: string;
}

export interface AuditEntry {
  id: string;
  timestamp: string;
  operation: string;
  path: string;
  user?: string;
  source_ip?: string;
  success: boolean;
}

export interface AuditLogResponse {
  entries: AuditEntry[];
}

// ============================================================================
// Service
// ============================================================================

export class VaultService {
  constructor(private request: RequestFn) {}

  /** Get vault status (sealed/unsealed, initialized). */
  async getStatus(): Promise<VaultStatus> {
    return this.request<VaultStatus>('GET', '/api/v1/vault/status');
  }

  /** Seal the vault. */
  async seal(): Promise<{ message: string }> {
    return this.request<{ message: string }>('POST', '/api/v1/vault/seal');
  }

  /** Unseal the vault with a key share. */
  async unseal(key: string): Promise<UnsealResponse> {
    return this.request<UnsealResponse>('POST', '/api/v1/vault/unseal', { key });
  }

  /** List all secrets (keys only, no values). */
  async listSecrets(): Promise<SecretListResponse> {
    return this.request<SecretListResponse>('GET', '/api/v1/vault/secrets');
  }

  /** Get a secret by key. */
  async getSecret(key: string): Promise<Secret> {
    return this.request<Secret>('GET', `/api/v1/vault/secrets/${encodeURIComponent(key)}`);
  }

  /** Create or update a secret. */
  async putSecret(key: string, value: string, metadata?: Record<string, string>): Promise<Secret> {
    return this.request<Secret>('PUT', `/api/v1/vault/secrets/${encodeURIComponent(key)}`, {
      value,
      metadata,
    });
  }

  /** Delete a secret. */
  async deleteSecret(key: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/vault/secrets/${encodeURIComponent(key)}`);
  }

  /** Encrypt data using a transit key. */
  async transitEncrypt(keyName: string, plaintext: string): Promise<TransitEncryptResponse> {
    return this.request<TransitEncryptResponse>('POST', '/api/v1/vault/transit/encrypt', {
      key_name: keyName,
      plaintext,
    });
  }

  /** Decrypt data using a transit key. */
  async transitDecrypt(keyName: string, ciphertext: string): Promise<TransitDecryptResponse> {
    return this.request<TransitDecryptResponse>('POST', '/api/v1/vault/transit/decrypt', {
      key_name: keyName,
      ciphertext,
    });
  }

  /** Create a new transit encryption key. */
  async createTransitKey(name: string, type?: string, exportable?: boolean): Promise<TransitKey> {
    return this.request<TransitKey>('POST', '/api/v1/vault/transit/keys', {
      name,
      type,
      exportable,
    });
  }

  /** List all transit encryption keys. */
  async listTransitKeys(): Promise<TransitKey[]> {
    return this.request<TransitKey[]>('GET', '/api/v1/vault/transit/keys');
  }

  /** Get vault audit log. */
  async getAuditLog(): Promise<AuditLogResponse> {
    return this.request<AuditLogResponse>('GET', '/api/v1/vault/audit');
  }
}
