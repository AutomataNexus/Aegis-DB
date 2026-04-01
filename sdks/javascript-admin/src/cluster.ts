/**
 * Cluster service for managing nodes, storage, settings, and monitoring.
 */

import type { RequestFn } from './index';

// ============================================================================
// Types
// ============================================================================

export interface ClusterInfo {
  cluster_name: string;
  node_count: number;
  leader_id?: string;
  status: string;
  nodes: ClusterNode[];
}

export interface ClusterNode {
  id: string;
  name: string;
  address: string;
  port: number;
  role: string;
  status: string;
  last_heartbeat?: string;
  uptime_seconds?: number;
}

export interface NodeLogs {
  node_id: string;
  logs: LogEntry[];
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  target?: string;
}

export interface StorageInfo {
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  tables: StorageTableInfo[];
}

export interface StorageTableInfo {
  name: string;
  size_bytes: number;
  row_count: number;
}

export interface DatabaseStats {
  total_queries: number;
  active_connections: number;
  uptime_seconds: number;
  tables_count: number;
  total_rows: number;
  cache_hit_ratio?: number;
  avg_query_time_ms?: number;
}

export interface ServerStats {
  cpu_usage?: number;
  memory_usage_bytes?: number;
  memory_total_bytes?: number;
  disk_usage_bytes?: number;
  disk_total_bytes?: number;
  open_connections: number;
  total_requests: number;
  requests_per_second?: number;
}

export interface Alert {
  id: string;
  severity: string;
  message: string;
  source: string;
  timestamp: string;
  acknowledged: boolean;
}

export interface Activity {
  id: string;
  action: string;
  user?: string;
  resource?: string;
  timestamp: string;
  details?: Record<string, unknown>;
}

export interface Settings {
  [key: string]: unknown;
}

// ============================================================================
// Service
// ============================================================================

export class ClusterService {
  constructor(private request: RequestFn) {}

  /** Get cluster overview. */
  async getCluster(): Promise<ClusterInfo> {
    return this.request<ClusterInfo>('GET', '/api/v1/admin/cluster');
  }

  /** List all nodes. */
  async listNodes(): Promise<ClusterNode[]> {
    return this.request<ClusterNode[]>('GET', '/api/v1/admin/nodes');
  }

  /** Restart a node. */
  async restartNode(nodeId: string): Promise<{ message: string }> {
    return this.request<{ message: string }>('POST', `/api/v1/admin/nodes/${encodeURIComponent(nodeId)}/restart`);
  }

  /** Drain a node (stop accepting new connections and migrate work). */
  async drainNode(nodeId: string): Promise<{ message: string }> {
    return this.request<{ message: string }>('POST', `/api/v1/admin/nodes/${encodeURIComponent(nodeId)}/drain`);
  }

  /** Get logs for a specific node. */
  async getNodeLogs(nodeId: string): Promise<NodeLogs> {
    return this.request<NodeLogs>('GET', `/api/v1/admin/nodes/${encodeURIComponent(nodeId)}/logs`);
  }

  /** Remove a node from the cluster. */
  async removeNode(nodeId: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/admin/nodes/${encodeURIComponent(nodeId)}`);
  }

  /** Get storage information. */
  async getStorage(): Promise<StorageInfo> {
    return this.request<StorageInfo>('GET', '/api/v1/admin/storage');
  }

  /** Get server stats (CPU, memory, connections). */
  async getStats(): Promise<ServerStats> {
    return this.request<ServerStats>('GET', '/api/v1/admin/stats');
  }

  /** Get database stats (queries, tables, rows). */
  async getDatabase(): Promise<DatabaseStats> {
    return this.request<DatabaseStats>('GET', '/api/v1/admin/database');
  }

  /** Get active alerts. */
  async getAlerts(): Promise<Alert[]> {
    return this.request<Alert[]>('GET', '/api/v1/admin/alerts');
  }

  /** Get recent activity log. */
  async getActivities(): Promise<Activity[]> {
    return this.request<Activity[]>('GET', '/api/v1/admin/activities');
  }

  /** Get server settings. */
  async getSettings(): Promise<Settings> {
    return this.request<Settings>('GET', '/api/v1/admin/settings');
  }

  /** Update server settings. */
  async updateSettings(settings: Settings): Promise<Settings> {
    return this.request<Settings>('PUT', '/api/v1/admin/settings', settings);
  }
}
