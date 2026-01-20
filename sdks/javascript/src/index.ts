/**
 * Aegis Database JavaScript/TypeScript SDK
 *
 * Official client library for Aegis Database Platform.
 *
 * @example
 * ```typescript
 * import { AegisClient } from '@aegis-db/client';
 *
 * const client = new AegisClient('http://localhost:8080');
 * await client.connect();
 *
 * const result = await client.query('SELECT * FROM users LIMIT 10');
 * console.log(result.rows);
 *
 * await client.close();
 * ```
 *
 * @version 1.0.0
 * @author AutomataNexus Development Team
 */

// ============================================================================
// Types
// ============================================================================

export interface AegisClientConfig {
  url: string;
  database?: string;
  username?: string;
  password?: string;
  apiKey?: string;
  timeout?: number;
  retryAttempts?: number;
  retryDelay?: number;
}

export interface Row {
  [key: string]: unknown;
}

export interface QueryResult {
  columns: string[];
  rows: Row[];
  rowsAffected: number;
  executionTimeMs: number;
}

export interface TableInfo {
  name: string;
  columns: ColumnInfo[];
  rowCount: number;
  sizeBytes: number;
  indexes: string[];
}

export interface ColumnInfo {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
  default?: unknown;
}

export interface KeyValueEntry {
  key: string;
  value: unknown;
  sizeBytes: number;
  createdAt: string;
  updatedAt: string;
  ttl?: number;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface GraphNode {
  id: string;
  label: string;
  properties: Record<string, unknown>;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  label: string;
  properties: Record<string, unknown>;
}

export class AegisError extends Error {
  constructor(message: string, public code?: string) {
    super(message);
    this.name = 'AegisError';
  }
}

export class ConnectionError extends AegisError {
  constructor(message: string) {
    super(message, 'CONNECTION_ERROR');
    this.name = 'ConnectionError';
  }
}

export class QueryError extends AegisError {
  constructor(message: string) {
    super(message, 'QUERY_ERROR');
    this.name = 'QueryError';
  }
}

export class AuthenticationError extends AegisError {
  constructor(message: string) {
    super(message, 'AUTH_ERROR');
    this.name = 'AuthenticationError';
  }
}

// ============================================================================
// Client
// ============================================================================

export class AegisClient {
  private config: Required<AegisClientConfig>;
  private token?: string;
  private connected = false;

  constructor(urlOrConfig: string | AegisClientConfig) {
    const defaultConfig: Omit<Required<AegisClientConfig>, 'url'> = {
      database: 'default',
      username: '',
      password: '',
      apiKey: '',
      timeout: 30000,
      retryAttempts: 3,
      retryDelay: 1000,
    };

    if (typeof urlOrConfig === 'string') {
      this.config = { ...defaultConfig, url: urlOrConfig };
    } else {
      this.config = { ...defaultConfig, ...urlOrConfig };
    }

    // Remove trailing slash
    this.config.url = this.config.url.replace(/\/$/, '');
  }

  // ==========================================================================
  // Connection Management
  // ==========================================================================

  async connect(): Promise<void> {
    if (this.connected) return;

    // Test connection
    await this.health();

    // Authenticate if credentials provided
    if (this.config.username && this.config.password) {
      await this.authenticate();
    }

    this.connected = true;
  }

  async close(): Promise<void> {
    if (this.token) {
      try {
        await this.request('POST', '/api/v1/auth/logout');
      } catch {
        // Ignore logout errors
      }
    }
    this.token = undefined;
    this.connected = false;
  }

  private async authenticate(): Promise<void> {
    const response = await this.request<{
      token?: string;
      requires_mfa?: boolean;
      error?: string;
    }>('POST', '/api/v1/auth/login', {
      username: this.config.username,
      password: this.config.password,
    });

    if (response.error) {
      throw new AuthenticationError(response.error);
    }

    if (response.requires_mfa) {
      throw new AuthenticationError('MFA required - use authenticateMfa()');
    }

    this.token = response.token;
  }

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
  }

  // ==========================================================================
  // HTTP Request Helper
  // ==========================================================================

  private async request<T>(
    method: 'GET' | 'POST' | 'DELETE',
    path: string,
    body?: unknown
  ): Promise<T> {
    const url = `${this.config.url}${path}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    if (this.config.apiKey) {
      headers['X-API-Key'] = this.config.apiKey;
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.config.timeout);

    try {
      const response = await fetch(url, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        const text = await response.text();
        if (response.status === 401 || response.status === 403) {
          throw new AuthenticationError(text);
        }
        throw new QueryError(`Request failed (${response.status}): ${text}`);
      }

      return await response.json();
    } catch (error) {
      clearTimeout(timeoutId);
      if (error instanceof AegisError) throw error;
      if (error instanceof Error) {
        if (error.name === 'AbortError') {
          throw new ConnectionError('Request timeout');
        }
        throw new ConnectionError(error.message);
      }
      throw new ConnectionError('Unknown error');
    }
  }

  // ==========================================================================
  // Query Methods
  // ==========================================================================

  async query(sql: string, params?: Record<string, unknown>): Promise<QueryResult> {
    const response = await this.request<{
      columns: string[];
      rows: unknown[][];
      rows_affected: number;
      execution_time_ms: number;
    }>('POST', '/api/v1/query', {
      query: sql,
      database: this.config.database,
      params: params || {},
    });

    const rows: Row[] = response.rows.map((row) => {
      const obj: Row = {};
      response.columns.forEach((col, i) => {
        obj[col] = row[i];
      });
      return obj;
    });

    return {
      columns: response.columns,
      rows,
      rowsAffected: response.rows_affected,
      executionTimeMs: response.execution_time_ms,
    };
  }

  async execute(sql: string, params?: Record<string, unknown>): Promise<number> {
    const result = await this.query(sql, params);
    return result.rowsAffected;
  }

  queryBuilder(table: string): QueryBuilder {
    return new QueryBuilder(this, table);
  }

  async *streamQuery(sql: string, batchSize = 1000): AsyncGenerator<Row> {
    let offset = 0;
    while (true) {
      const result = await this.query(`${sql} LIMIT ${batchSize} OFFSET ${offset}`);

      if (result.rows.length === 0) break;

      for (const row of result.rows) {
        yield row;
      }

      if (result.rows.length < batchSize) break;
      offset += batchSize;
    }
  }

  // ==========================================================================
  // Transaction Support
  // ==========================================================================

  async transaction<T>(fn: (tx: Transaction) => Promise<T>): Promise<T> {
    const tx = new Transaction(this);
    try {
      await tx.begin();
      const result = await fn(tx);
      await tx.commit();
      return result;
    } catch (error) {
      await tx.rollback();
      throw error;
    }
  }

  // ==========================================================================
  // Schema Methods
  // ==========================================================================

  async listTables(): Promise<TableInfo[]> {
    const response = await this.request<{ tables: TableInfo[] }>('GET', '/api/v1/tables');
    return response.tables;
  }

  async getTable(name: string): Promise<TableInfo> {
    return await this.request<TableInfo>('GET', `/api/v1/tables/${name}`);
  }

  // ==========================================================================
  // Key-Value Store
  // ==========================================================================

  async kvGet(key: string): Promise<unknown | undefined> {
    try {
      const entries = await this.request<KeyValueEntry[]>('GET', '/api/v1/kv/keys');
      const entry = entries.find((e) => e.key === key);
      return entry?.value;
    } catch {
      return undefined;
    }
  }

  async kvSet(key: string, value: unknown, ttl?: number): Promise<void> {
    await this.request('POST', '/api/v1/kv/keys', { key, value, ttl });
  }

  async kvDelete(key: string): Promise<boolean> {
    try {
      await this.request('DELETE', `/api/v1/kv/keys/${key}`);
      return true;
    } catch {
      return false;
    }
  }

  async kvList(): Promise<KeyValueEntry[]> {
    return await this.request<KeyValueEntry[]>('GET', '/api/v1/kv/keys');
  }

  // ==========================================================================
  // Document Store
  // ==========================================================================

  async listCollections(): Promise<{ name: string; documentCount: number }[]> {
    return await this.request('GET', '/api/v1/documents/collections');
  }

  async getCollection(name: string): Promise<{ id: string; data: unknown }[]> {
    return await this.request('GET', `/api/v1/documents/collections/${name}`);
  }

  // ==========================================================================
  // Graph Database
  // ==========================================================================

  async getGraphData(): Promise<GraphData> {
    return await this.request<GraphData>('GET', '/api/v1/graph/data');
  }

  // ==========================================================================
  // Health and Metrics
  // ==========================================================================

  async health(): Promise<{ status: string }> {
    return await this.request<{ status: string }>('GET', '/health');
  }

  async metrics(): Promise<Record<string, unknown>> {
    return await this.request('GET', '/api/v1/metrics');
  }
}

// ============================================================================
// Query Builder
// ============================================================================

export class QueryBuilder {
  private client: AegisClient;
  private table: string;
  private selectCols: string[] = ['*'];
  private whereClauses: string[] = [];
  private whereParams: Record<string, unknown> = {};
  private orderByCols: string[] = [];
  private groupByCols: string[] = [];
  private limitVal?: number;
  private offsetVal?: number;
  private joins: string[] = [];
  private paramCounter = 0;

  constructor(client: AegisClient, table: string) {
    this.client = client;
    this.table = table;
  }

  private nextParam(): string {
    this.paramCounter++;
    return `p${this.paramCounter}`;
  }

  select(...columns: string[]): this {
    this.selectCols = columns.length ? columns : ['*'];
    return this;
  }

  where(column: string, operator: string, value: unknown): this {
    const param = this.nextParam();
    this.whereClauses.push(`${column} ${operator} :${param}`);
    this.whereParams[param] = value;
    return this;
  }

  whereIn(column: string, values: unknown[]): this {
    const placeholders = values.map((val) => {
      const param = this.nextParam();
      this.whereParams[param] = val;
      return `:${param}`;
    });
    this.whereClauses.push(`${column} IN (${placeholders.join(', ')})`);
    return this;
  }

  whereNull(column: string): this {
    this.whereClauses.push(`${column} IS NULL`);
    return this;
  }

  whereNotNull(column: string): this {
    this.whereClauses.push(`${column} IS NOT NULL`);
    return this;
  }

  join(table: string, on: string, type: 'INNER' | 'LEFT' | 'RIGHT' = 'INNER'): this {
    this.joins.push(`${type} JOIN ${table} ON ${on}`);
    return this;
  }

  leftJoin(table: string, on: string): this {
    return this.join(table, on, 'LEFT');
  }

  orderBy(column: string, direction: 'ASC' | 'DESC' = 'ASC'): this {
    this.orderByCols.push(`${column} ${direction}`);
    return this;
  }

  groupBy(...columns: string[]): this {
    this.groupByCols.push(...columns);
    return this;
  }

  limit(count: number): this {
    this.limitVal = count;
    return this;
  }

  offset(count: number): this {
    this.offsetVal = count;
    return this;
  }

  build(): { sql: string; params: Record<string, unknown> } {
    const parts = [`SELECT ${this.selectCols.join(', ')} FROM ${this.table}`];

    if (this.joins.length) parts.push(...this.joins);
    if (this.whereClauses.length) parts.push(`WHERE ${this.whereClauses.join(' AND ')}`);
    if (this.groupByCols.length) parts.push(`GROUP BY ${this.groupByCols.join(', ')}`);
    if (this.orderByCols.length) parts.push(`ORDER BY ${this.orderByCols.join(', ')}`);
    if (this.limitVal !== undefined) parts.push(`LIMIT ${this.limitVal}`);
    if (this.offsetVal !== undefined) parts.push(`OFFSET ${this.offsetVal}`);

    return { sql: parts.join(' '), params: this.whereParams };
  }

  async execute(): Promise<QueryResult> {
    const { sql, params } = this.build();
    return await this.client.query(sql, params);
  }

  async first(): Promise<Row | undefined> {
    this.limitVal = 1;
    const result = await this.execute();
    return result.rows[0];
  }

  async count(): Promise<number> {
    this.selectCols = ['COUNT(*) as count'];
    const result = await this.execute();
    return (result.rows[0]?.count as number) || 0;
  }

  async exists(): Promise<boolean> {
    return (await this.count()) > 0;
  }
}

// ============================================================================
// Transaction
// ============================================================================

export class Transaction {
  private client: AegisClient;
  private active = false;
  private savepoints: string[] = [];

  constructor(client: AegisClient) {
    this.client = client;
  }

  get isActive(): boolean {
    return this.active;
  }

  async begin(): Promise<void> {
    if (this.active) throw new Error('Transaction already active');
    await this.client.query('BEGIN TRANSACTION');
    this.active = true;
  }

  async commit(): Promise<void> {
    if (!this.active) throw new Error('No active transaction');
    await this.client.query('COMMIT');
    this.active = false;
    this.savepoints = [];
  }

  async rollback(): Promise<void> {
    if (!this.active) throw new Error('No active transaction');
    await this.client.query('ROLLBACK');
    this.active = false;
    this.savepoints = [];
  }

  async savepoint(name: string): Promise<string> {
    if (!this.active) throw new Error('No active transaction');
    await this.client.query(`SAVEPOINT ${name}`);
    this.savepoints.push(name);
    return name;
  }

  async rollbackTo(savepoint: string): Promise<void> {
    if (!this.active) throw new Error('No active transaction');
    if (!this.savepoints.includes(savepoint)) {
      throw new Error(`Unknown savepoint: ${savepoint}`);
    }
    await this.client.query(`ROLLBACK TO SAVEPOINT ${savepoint}`);
    const idx = this.savepoints.indexOf(savepoint);
    this.savepoints = this.savepoints.slice(0, idx + 1);
  }

  async execute(sql: string, params?: Record<string, unknown>): Promise<number> {
    if (!this.active) throw new Error('No active transaction');
    return await this.client.execute(sql, params);
  }

  async query(sql: string, params?: Record<string, unknown>): Promise<QueryResult> {
    if (!this.active) throw new Error('No active transaction');
    return await this.client.query(sql, params);
  }
}

// Default export
export default AegisClient;
