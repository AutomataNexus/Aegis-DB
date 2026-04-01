/**
 * Shield service for security monitoring, IP blocking, and threat management.
 */

import type { RequestFn } from './index';

// ============================================================================
// Types
// ============================================================================

export interface ShieldStatus {
  enabled: boolean;
  mode: string;
  patterns_loaded: number;
  blocked_ips: number;
  allowlisted_ips: number;
  events_total: number;
}

export interface ShieldStats {
  total_requests: number;
  blocked_requests: number;
  sql_injection_attempts: number;
  xss_attempts: number;
  rate_limited: number;
  suspicious_ips: number;
  threats_detected: number;
  period?: string;
}

export interface ShieldEvent {
  id: string;
  timestamp: string;
  event_type: string;
  severity: string;
  source_ip: string;
  description: string;
  details?: Record<string, unknown>;
  blocked: boolean;
}

export interface BlockedIP {
  ip: string;
  reason: string;
  blocked_at: string;
  expires_at?: string;
  auto_blocked: boolean;
}

export interface AllowlistEntry {
  ip: string;
  description?: string;
  added_at: string;
  added_by?: string;
}

export interface BlockIPRequest {
  ip: string;
  reason: string;
  duration_seconds?: number;
}

export interface AllowlistRequest {
  ip: string;
  description?: string;
}

export interface ShieldPolicy {
  sql_injection_detection: boolean;
  xss_detection: boolean;
  rate_limiting: boolean;
  ip_reputation: boolean;
  auto_block: boolean;
  auto_block_threshold: number;
  max_request_size_bytes?: number;
  request_rate_limit?: number;
  login_rate_limit?: number;
  [key: string]: unknown;
}

export interface IPReputation {
  ip: string;
  score: number;
  risk_level: string;
  total_requests: number;
  blocked_requests: number;
  last_seen: string;
  flags: string[];
}

export interface ThreatFeed {
  entries: ThreatFeedEntry[];
  last_updated: string;
  source?: string;
}

export interface ThreatFeedEntry {
  ip: string;
  threat_type: string;
  confidence: number;
  first_seen: string;
  last_seen: string;
}

// ============================================================================
// Service
// ============================================================================

export class ShieldService {
  constructor(private request: RequestFn) {}

  /** Get shield status. */
  async getStatus(): Promise<ShieldStatus> {
    return this.request<ShieldStatus>('GET', '/api/v1/shield/status');
  }

  /** Get shield statistics. */
  async getStats(): Promise<ShieldStats> {
    return this.request<ShieldStats>('GET', '/api/v1/shield/stats');
  }

  /** Get security events. */
  async getEvents(): Promise<ShieldEvent[]> {
    return this.request<ShieldEvent[]>('GET', '/api/v1/shield/events');
  }

  /** List blocked IPs. */
  async listBlocked(): Promise<BlockedIP[]> {
    return this.request<BlockedIP[]>('GET', '/api/v1/shield/blocked');
  }

  /** Block an IP address. */
  async blockIP(ip: string, reason: string, durationSeconds?: number): Promise<BlockedIP> {
    return this.request<BlockedIP>('POST', '/api/v1/shield/blocked', {
      ip,
      reason,
      duration_seconds: durationSeconds,
    });
  }

  /** Unblock an IP address. */
  async unblockIP(ip: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/shield/blocked/${encodeURIComponent(ip)}`);
  }

  /** List allowlisted IPs. */
  async listAllowlist(): Promise<AllowlistEntry[]> {
    return this.request<AllowlistEntry[]>('GET', '/api/v1/shield/allowlist');
  }

  /** Add an IP to the allowlist. */
  async addToAllowlist(ip: string, description?: string): Promise<AllowlistEntry> {
    return this.request<AllowlistEntry>('POST', '/api/v1/shield/allowlist', {
      ip,
      description,
    });
  }

  /** Remove an IP from the allowlist. */
  async removeFromAllowlist(ip: string): Promise<void> {
    await this.request<void>('DELETE', `/api/v1/shield/allowlist/${encodeURIComponent(ip)}`);
  }

  /** Get the current shield security policy. */
  async getPolicy(): Promise<ShieldPolicy> {
    return this.request<ShieldPolicy>('GET', '/api/v1/shield/policy');
  }

  /** Update the shield security policy. */
  async updatePolicy(policy: Partial<ShieldPolicy>): Promise<ShieldPolicy> {
    return this.request<ShieldPolicy>('PUT', '/api/v1/shield/policy', policy);
  }

  /** Get IP reputation details. */
  async getIPReputation(ip: string): Promise<IPReputation> {
    return this.request<IPReputation>('GET', `/api/v1/shield/ip/${encodeURIComponent(ip)}`);
  }

  /** Get the threat feed. */
  async getThreatFeed(): Promise<ThreatFeed> {
    return this.request<ThreatFeed>('GET', '/api/v1/shield/feed');
  }
}
