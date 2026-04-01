/**
 * Compliance service for GDPR, consent management, breach tracking, and auditing.
 */

import type { RequestFn } from './index';

// ============================================================================
// Types
// ============================================================================

// --- Data Subject ---

export interface DataDeletionResult {
  subject_id: string;
  records_deleted: number;
  tables_affected: string[];
  completed_at: string;
}

export interface DataExportRequest {
  subject_id: string;
  format?: string;
  include_metadata?: boolean;
}

export interface DataExportResult {
  subject_id: string;
  format: string;
  data: Record<string, unknown>;
  generated_at: string;
  size_bytes?: number;
}

// --- Certificates ---

export interface ComplianceCertificate {
  id: string;
  type: string;
  standard: string;
  status: string;
  issued_at: string;
  expires_at?: string;
  issuer?: string;
  details?: Record<string, unknown>;
}

export interface CertificateVerification {
  id: string;
  valid: boolean;
  verified_at: string;
  details?: Record<string, unknown>;
}

// --- Audit ---

export interface ComplianceAuditEntry {
  id: string;
  subject_id: string;
  action: string;
  timestamp: string;
  details?: Record<string, unknown>;
  actor?: string;
}

export interface AuditVerification {
  valid: boolean;
  entries_checked: number;
  tampered_entries: number;
  verified_at: string;
}

// --- Consent ---

export interface ConsentRecord {
  subject_id: string;
  purpose: string;
  granted: boolean;
  granted_at?: string;
  revoked_at?: string;
  expires_at?: string;
  source?: string;
  metadata?: Record<string, unknown>;
}

export interface ConsentRequest {
  subject_id: string;
  purpose: string;
  granted: boolean;
  expires_at?: string;
  source?: string;
  metadata?: Record<string, unknown>;
}

export interface ConsentStats {
  total_subjects: number;
  total_consents: number;
  granted: number;
  revoked: number;
  expired: number;
  by_purpose: Record<string, number>;
}

export interface ConsentHistory {
  subject_id: string;
  history: ConsentRecord[];
}

export interface ConsentExport {
  subject_id: string;
  consents: ConsentRecord[];
  exported_at: string;
}

export interface ConsentCheck {
  subject_id: string;
  purpose: string;
  granted: boolean;
  expires_at?: string;
}

// --- Do Not Sell ---

export interface DoNotSellList {
  subjects: DoNotSellEntry[];
  total: number;
}

export interface DoNotSellEntry {
  subject_id: string;
  requested_at: string;
  status: string;
}

// --- Breaches ---

export interface Breach {
  id: string;
  title: string;
  description: string;
  severity: string;
  status: string;
  detected_at: string;
  acknowledged_at?: string;
  resolved_at?: string;
  affected_subjects?: number;
  affected_data_types?: string[];
  reporter?: string;
}

export interface BreachStats {
  total: number;
  open: number;
  acknowledged: number;
  resolved: number;
  by_severity: Record<string, number>;
}

export interface BreachCleanupResult {
  cleaned: number;
  message: string;
}

export interface BreachReport {
  breach: Breach;
  timeline: BreachTimelineEntry[];
  affected_subjects: number;
  notification_status?: string;
  generated_at: string;
}

export interface BreachTimelineEntry {
  timestamp: string;
  action: string;
  actor?: string;
  details?: string;
}

// --- Security Events ---

export interface SecurityEvent {
  id: string;
  timestamp: string;
  event_type: string;
  severity: string;
  source: string;
  description: string;
  details?: Record<string, unknown>;
}

// ============================================================================
// Service
// ============================================================================

export class ComplianceService {
  constructor(private request: RequestFn) {}

  // --- Data Subject Rights ---

  /** Delete all data for a data subject (GDPR right to erasure). */
  async deleteDataSubject(subjectId: string): Promise<DataDeletionResult> {
    return this.request<DataDeletionResult>(
      'DELETE',
      `/api/v1/compliance/data-subject/${encodeURIComponent(subjectId)}`
    );
  }

  /** Export all data for a data subject (GDPR right to portability). */
  async exportData(options: DataExportRequest): Promise<DataExportResult> {
    return this.request<DataExportResult>('POST', '/api/v1/compliance/export', options);
  }

  // --- Certificates ---

  /** List compliance certificates. */
  async listCertificates(): Promise<ComplianceCertificate[]> {
    return this.request<ComplianceCertificate[]>('GET', '/api/v1/compliance/certificates');
  }

  /** Get a specific compliance certificate. */
  async getCertificate(id: string): Promise<ComplianceCertificate> {
    return this.request<ComplianceCertificate>(
      'GET',
      `/api/v1/compliance/certificates/${encodeURIComponent(id)}`
    );
  }

  /** Verify a compliance certificate. */
  async verifyCertificate(id: string): Promise<CertificateVerification> {
    return this.request<CertificateVerification>(
      'GET',
      `/api/v1/compliance/certificates/${encodeURIComponent(id)}/verify`
    );
  }

  // --- Audit ---

  /** Get compliance audit trail for a data subject. */
  async getAuditTrail(subjectId: string): Promise<ComplianceAuditEntry[]> {
    return this.request<ComplianceAuditEntry[]>(
      'GET',
      `/api/v1/compliance/audit/${encodeURIComponent(subjectId)}`
    );
  }

  /** Verify audit log integrity. */
  async verifyAuditLog(): Promise<AuditVerification> {
    return this.request<AuditVerification>('GET', '/api/v1/compliance/audit/verify');
  }

  // --- Consent Management ---

  /** Record or update consent. */
  async recordConsent(consent: ConsentRequest): Promise<ConsentRecord> {
    return this.request<ConsentRecord>('POST', '/api/v1/compliance/consent', consent);
  }

  /** Get consent statistics. */
  async getConsentStats(): Promise<ConsentStats> {
    return this.request<ConsentStats>('GET', '/api/v1/compliance/consent/stats');
  }

  /** Get all consent records for a subject. */
  async getSubjectConsent(subjectId: string): Promise<ConsentRecord[]> {
    return this.request<ConsentRecord[]>(
      'GET',
      `/api/v1/compliance/consent/${encodeURIComponent(subjectId)}`
    );
  }

  /** Delete all consent records for a subject. */
  async deleteSubjectConsent(subjectId: string): Promise<void> {
    await this.request<void>(
      'DELETE',
      `/api/v1/compliance/consent/${encodeURIComponent(subjectId)}`
    );
  }

  /** Get consent history for a subject. */
  async getConsentHistory(subjectId: string): Promise<ConsentHistory> {
    return this.request<ConsentHistory>(
      'GET',
      `/api/v1/compliance/consent/${encodeURIComponent(subjectId)}/history`
    );
  }

  /** Export consent records for a subject. */
  async exportConsent(subjectId: string): Promise<ConsentExport> {
    return this.request<ConsentExport>(
      'GET',
      `/api/v1/compliance/consent/${encodeURIComponent(subjectId)}/export`
    );
  }

  /** Check if a subject has granted consent for a specific purpose. */
  async checkConsent(subjectId: string, purpose: string): Promise<ConsentCheck> {
    return this.request<ConsentCheck>(
      'GET',
      `/api/v1/compliance/consent/${encodeURIComponent(subjectId)}/check/${encodeURIComponent(purpose)}`
    );
  }

  /** Revoke consent for a specific purpose. */
  async revokeConsent(subjectId: string, purpose: string): Promise<void> {
    await this.request<void>(
      'DELETE',
      `/api/v1/compliance/consent/${encodeURIComponent(subjectId)}/${encodeURIComponent(purpose)}`
    );
  }

  // --- Do Not Sell ---

  /** Get the do-not-sell list (CCPA). */
  async getDoNotSellList(): Promise<DoNotSellList> {
    return this.request<DoNotSellList>('GET', '/api/v1/compliance/do-not-sell');
  }

  // --- Breach Management ---

  /** List all breaches. */
  async listBreaches(): Promise<Breach[]> {
    return this.request<Breach[]>('GET', '/api/v1/compliance/breaches');
  }

  /** Get breach statistics. */
  async getBreachStats(): Promise<BreachStats> {
    return this.request<BreachStats>('GET', '/api/v1/compliance/breaches/stats');
  }

  /** Clean up resolved breaches. */
  async cleanupBreaches(): Promise<BreachCleanupResult> {
    return this.request<BreachCleanupResult>('POST', '/api/v1/compliance/breaches/cleanup');
  }

  /** Get a specific breach by ID. */
  async getBreach(id: string): Promise<Breach> {
    return this.request<Breach>(
      'GET',
      `/api/v1/compliance/breaches/${encodeURIComponent(id)}`
    );
  }

  /** Acknowledge a breach. */
  async acknowledgeBreach(id: string): Promise<Breach> {
    return this.request<Breach>(
      'POST',
      `/api/v1/compliance/breaches/${encodeURIComponent(id)}/acknowledge`
    );
  }

  /** Mark a breach as resolved. */
  async resolveBreach(id: string): Promise<Breach> {
    return this.request<Breach>(
      'POST',
      `/api/v1/compliance/breaches/${encodeURIComponent(id)}/resolve`
    );
  }

  /** Generate a formal breach report. */
  async getBreachReport(id: string): Promise<BreachReport> {
    return this.request<BreachReport>(
      'GET',
      `/api/v1/compliance/breaches/${encodeURIComponent(id)}/report`
    );
  }

  // --- Security Events ---

  /** Get security events. */
  async getSecurityEvents(): Promise<SecurityEvent[]> {
    return this.request<SecurityEvent[]>('GET', '/api/v1/compliance/security-events');
  }
}
