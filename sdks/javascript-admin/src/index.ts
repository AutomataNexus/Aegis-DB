/**
 * Aegis-DB Admin SDK
 *
 * Server-side admin SDK for privileged operations on Aegis Database Platform.
 * Covers user management, cluster administration, backups, vault secrets,
 * shield security, and GDPR/compliance operations.
 *
 * @example
 * ```typescript
 * import { AegisAdmin } from '@aegis-db/admin';
 *
 * const admin = new AegisAdmin({
 *   url: 'http://localhost:9090',
 *   username: 'admin',
 *   password: 'secret',
 * });
 *
 * await admin.connect();
 *
 * // User management
 * const { users } = await admin.auth.listUsers();
 *
 * // Vault secrets
 * await admin.vault.putSecret('api-key', 's3cret');
 * const secret = await admin.vault.getSecret('api-key');
 *
 * // Shield security
 * await admin.shield.blockIP('10.0.0.1', 'suspicious activity');
 *
 * // GDPR compliance
 * await admin.compliance.deleteDataSubject('user-123');
 * ```
 *
 * @version 1.0.0
 * @author AutomataNexus Development Team
 */

// ============================================================================
// Re-exports
// ============================================================================

export { AuthService } from './auth';
export type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
  Role,
  CreateRoleRequest,
  UserListResponse,
  RoleListResponse,
} from './auth';

export { ClusterService } from './cluster';
export type {
  ClusterInfo,
  ClusterNode,
  NodeLogs,
  LogEntry,
  StorageInfo,
  StorageTableInfo,
  DatabaseStats,
  ServerStats,
  Alert,
  Activity,
  Settings,
} from './cluster';

export { BackupService } from './backup';
export type {
  Backup,
  CreateBackupRequest,
  RestoreRequest,
  RestoreResult,
  BackupListResponse,
} from './backup';

export { VaultService } from './vault';
export type {
  VaultStatus,
  Secret,
  SecretListResponse,
  SecretEntry,
  PutSecretRequest,
  UnsealRequest,
  UnsealResponse,
  TransitEncryptRequest,
  TransitEncryptResponse,
  TransitDecryptRequest,
  TransitDecryptResponse,
  TransitKeyRequest,
  TransitKey,
  AuditEntry,
  AuditLogResponse,
} from './vault';

export { ShieldService } from './shield';
export type {
  ShieldStatus,
  ShieldStats,
  ShieldEvent,
  BlockedIP,
  AllowlistEntry,
  BlockIPRequest,
  AllowlistRequest,
  ShieldPolicy,
  IPReputation,
  ThreatFeed,
  ThreatFeedEntry,
} from './shield';

export { ComplianceService } from './compliance';
export type {
  DataDeletionResult,
  DataExportRequest,
  DataExportResult,
  ComplianceCertificate,
  CertificateVerification,
  ComplianceAuditEntry,
  AuditVerification,
  ConsentRecord,
  ConsentRequest,
  ConsentStats,
  ConsentHistory,
  ConsentExport,
  ConsentCheck,
  DoNotSellList,
  DoNotSellEntry,
  Breach,
  BreachStats,
  BreachCleanupResult,
  BreachReport,
  BreachTimelineEntry,
  SecurityEvent,
} from './compliance';

// ============================================================================
// Shared types
// ============================================================================

/** HTTP method type. */
export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

/** Signature of the internal request function passed to sub-services. */
export type RequestFn = <T>(method: HttpMethod, path: string, body?: unknown) => Promise<T>;

// ============================================================================
// Configuration
// ============================================================================

export interface AegisAdminConfig {
  /** Server URL (e.g. http://localhost:9090). */
  url: string;
  /** Admin username for authentication. */
  username?: string;
  /** Admin password for authentication. */
  password?: string;
  /** API key for authentication (alternative to username/password). */
  apiKey?: string;
  /** Pre-existing bearer token (skips login). */
  token?: string;
  /** Request timeout in milliseconds (default: 30000). */
  timeout?: number;
  /** Number of retry attempts for transient failures (default: 3). */
  retryAttempts?: number;
  /** Delay between retries in milliseconds (default: 1000). */
  retryDelay?: number;
}

// ============================================================================
// Error classes
// ============================================================================

export class AdminError extends Error {
  public readonly statusCode?: number;

  constructor(message: string, public code?: string, statusCode?: number) {
    super(message);
    this.name = 'AdminError';
    this.statusCode = statusCode;
  }
}

export class AuthenticationError extends AdminError {
  constructor(message: string) {
    super(message, 'AUTH_ERROR', 401);
    this.name = 'AuthenticationError';
  }
}

export class ConnectionError extends AdminError {
  constructor(message: string) {
    super(message, 'CONNECTION_ERROR');
    this.name = 'ConnectionError';
  }
}

export class NotFoundError extends AdminError {
  constructor(message: string) {
    super(message, 'NOT_FOUND', 404);
    this.name = 'NotFoundError';
  }
}

export class ForbiddenError extends AdminError {
  constructor(message: string) {
    super(message, 'FORBIDDEN', 403);
    this.name = 'ForbiddenError';
  }
}

// ============================================================================
// Main Admin client
// ============================================================================

import { AuthService } from './auth';
import { ClusterService } from './cluster';
import { BackupService } from './backup';
import { VaultService } from './vault';
import { ShieldService } from './shield';
import { ComplianceService } from './compliance';

export class AegisAdmin {
  private readonly url: string;
  private readonly username?: string;
  private readonly password?: string;
  private readonly apiKey?: string;
  private readonly timeout: number;
  private readonly retryAttempts: number;
  private readonly retryDelay: number;
  private token?: string;
  private connected = false;

  /** User and role management. */
  public readonly auth: AuthService;
  /** Cluster, nodes, storage, settings, and monitoring. */
  public readonly cluster: ClusterService;
  /** Backup and restore operations. */
  public readonly backup: BackupService;
  /** Secrets management and transit encryption. */
  public readonly vault: VaultService;
  /** Security shield, IP blocking, and threat management. */
  public readonly shield: ShieldService;
  /** GDPR compliance, consent, breach tracking, and auditing. */
  public readonly compliance: ComplianceService;

  constructor(config: AegisAdminConfig) {
    this.url = config.url.replace(/\/$/, '');
    this.username = config.username;
    this.password = config.password;
    this.apiKey = config.apiKey;
    this.token = config.token;
    this.timeout = config.timeout ?? 30000;
    this.retryAttempts = config.retryAttempts ?? 3;
    this.retryDelay = config.retryDelay ?? 1000;

    // Bind the request method so sub-services can call it.
    const boundRequest: RequestFn = this.request.bind(this);

    this.auth = new AuthService(boundRequest);
    this.cluster = new ClusterService(boundRequest);
    this.backup = new BackupService(boundRequest);
    this.vault = new VaultService(boundRequest);
    this.shield = new ShieldService(boundRequest);
    this.compliance = new ComplianceService(boundRequest);
  }

  // ==========================================================================
  // Connection lifecycle
  // ==========================================================================

  /** Connect to the server: verify reachability and authenticate. */
  async connect(): Promise<void> {
    if (this.connected) return;

    // Verify the server is reachable.
    await this.health();

    // Authenticate if credentials provided and no token yet.
    if (!this.token && this.username && this.password) {
      await this.authenticate();
    }

    this.connected = true;
  }

  /** Disconnect and invalidate the session token. */
  async disconnect(): Promise<void> {
    if (this.token) {
      try {
        await this.request('POST', '/api/v1/auth/logout');
      } catch {
        // Ignore logout errors.
      }
    }
    this.token = undefined;
    this.connected = false;
  }

  /** Check server health. */
  async health(): Promise<{ status: string }> {
    return this.request<{ status: string }>('GET', '/health');
  }

  /** Whether the client is currently connected. */
  get isConnected(): boolean {
    return this.connected;
  }

  // ==========================================================================
  // Authentication
  // ==========================================================================

  private async authenticate(): Promise<void> {
    const response = await this.request<{
      token?: string;
      requires_mfa?: boolean;
      error?: string;
    }>('POST', '/api/v1/auth/login', {
      username: this.username,
      password: this.password,
    });

    if (response.error) {
      throw new AuthenticationError(response.error);
    }

    if (response.requires_mfa) {
      throw new AuthenticationError('MFA required - call authenticateMfa() with the TOTP code');
    }

    this.token = response.token;
  }

  /** Complete MFA authentication after connect() raises an MFA-required error. */
  async authenticateMfa(code: string, tempToken: string): Promise<void> {
    const response = await this.request<{
      token?: string;
      error?: string;
    }>('POST', '/api/v1/auth/mfa/verify', {
      code,
      token: tempToken,
    });

    if (response.error) {
      throw new AuthenticationError(response.error);
    }

    this.token = response.token;
    this.connected = true;
  }

  // ==========================================================================
  // HTTP request engine
  // ==========================================================================

  /** Internal HTTP request with auth headers, timeout, and retry logic. */
  async request<T>(method: HttpMethod, path: string, body?: unknown): Promise<T> {
    const url = `${this.url}${path}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    if (this.apiKey) {
      headers['X-API-Key'] = this.apiKey;
    }

    let lastError: Error | undefined;

    for (let attempt = 0; attempt <= this.retryAttempts; attempt++) {
      if (attempt > 0) {
        await this.sleep(this.retryDelay * attempt);
      }

      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), this.timeout);

      try {
        const response = await fetch(url, {
          method,
          headers,
          body: body !== undefined ? JSON.stringify(body) : undefined,
          signal: controller.signal,
        });

        clearTimeout(timeoutId);

        if (!response.ok) {
          const text = await response.text();

          // Non-retryable errors: throw immediately.
          if (response.status === 401) {
            throw new AuthenticationError(text || 'Unauthorized');
          }
          if (response.status === 403) {
            throw new ForbiddenError(text || 'Forbidden');
          }
          if (response.status === 404) {
            throw new NotFoundError(text || 'Not found');
          }

          // Retryable server errors (5xx).
          if (response.status >= 500) {
            lastError = new AdminError(
              `Server error (${response.status}): ${text}`,
              'SERVER_ERROR',
              response.status
            );
            continue;
          }

          // Other client errors: not retryable.
          throw new AdminError(
            `Request failed (${response.status}): ${text}`,
            'REQUEST_ERROR',
            response.status
          );
        }

        // Some endpoints return 204 No Content.
        const contentType = response.headers.get('content-type');
        if (response.status === 204 || !contentType?.includes('application/json')) {
          const text = await response.text();
          if (!text) return undefined as T;
          try {
            return JSON.parse(text) as T;
          } catch {
            return undefined as T;
          }
        }

        return (await response.json()) as T;
      } catch (error) {
        clearTimeout(timeoutId);

        // Don't retry auth/client errors.
        if (error instanceof AdminError) {
          if (error.statusCode && error.statusCode < 500) {
            throw error;
          }
          lastError = error;
          continue;
        }

        if (error instanceof Error) {
          if (error.name === 'AbortError') {
            lastError = new ConnectionError('Request timeout');
            continue;
          }
          lastError = new ConnectionError(error.message);
          continue;
        }

        throw new ConnectionError('Unknown error');
      }
    }

    throw lastError ?? new ConnectionError('Request failed after retries');
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

export default AegisAdmin;
