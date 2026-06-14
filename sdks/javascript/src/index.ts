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
    method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
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

  async query(sql: string, params: unknown[] = []): Promise<QueryResult> {
    const response = await this.request<{
      success: boolean;
      data?: { columns: string[]; rows: unknown[][]; rows_affected: number };
      error?: string;
      execution_time_ms: number;
    }>('POST', '/api/v1/query', {
      sql,
      database: this.config.database,
      params,
    });

    const columns = response.data?.columns ?? [];
    const rawRows = response.data?.rows ?? [];
    const rows: Row[] = rawRows.map((row) => {
      const obj: Row = {};
      columns.forEach((col, i) => {
        obj[col] = row[i];
      });
      return obj;
    });

    return {
      columns,
      rows,
      rowsAffected: response.data?.rows_affected ?? 0,
      executionTimeMs: response.execution_time_ms,
    };
  }

  async execute(sql: string, params: unknown[] = []): Promise<number> {
    const result = await this.query(sql, params);
    return result.rowsAffected;
  }

  /** Prepare a statement; returns its id for repeated execution. */
  async prepare(sql: string): Promise<string> {
    const res = await this.request<{ statement_id: string }>('POST', '/api/v1/prepare', {
      sql,
      database: this.config.database,
    });
    return res.statement_id;
  }

  /** Execute a prepared statement with bound positional parameters. */
  async executePrepared(statementId: string, params: unknown[] = []): Promise<QueryResult> {
    const response = await this.request<{
      success: boolean;
      data?: { columns: string[]; rows: unknown[][]; rows_affected: number };
      execution_time_ms: number;
    }>('POST', '/api/v1/prepared/execute', { statement_id: statementId, params });
    const columns = response.data?.columns ?? [];
    const rawRows = response.data?.rows ?? [];
    const rows: Row[] = rawRows.map((row) => {
      const obj: Row = {};
      columns.forEach((col, i) => {
        obj[col] = row[i];
      });
      return obj;
    });
    return {
      columns,
      rows,
      rowsAffected: response.data?.rows_affected ?? 0,
      executionTimeMs: response.execution_time_ms,
    };
  }

  /** Deallocate a prepared statement. */
  async deallocate(statementId: string): Promise<void> {
    await this.request('DELETE', `/api/v1/prepared/${statementId}`);
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
      const entry = await this.request<{ value: unknown } | null>(
        'GET',
        `/api/v1/kv/keys/${key}`
      );
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

  /** Get many keys at once (missing keys are omitted). */
  async kvBatchGet(keys: string[]): Promise<KeyValueEntry[]> {
    const res = await this.request<{ entries: KeyValueEntry[] }>(
      'POST',
      '/api/v1/kv/batch/get',
      { keys }
    );
    return res.entries ?? [];
  }

  /** Set many keys at once. Returns the number written. */
  async kvBatchSet(
    entries: { key: string; value: unknown; ttl?: number }[]
  ): Promise<number> {
    const res = await this.request<{ count: number }>('POST', '/api/v1/kv/batch/set', {
      entries,
    });
    return res.count ?? 0;
  }

  /** Delete many keys at once. Returns the number deleted. */
  async kvBatchDelete(keys: string[]): Promise<number> {
    const res = await this.request<{ deleted: number }>('POST', '/api/v1/kv/batch/delete', {
      keys,
    });
    return res.deleted ?? 0;
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

  /** Insert many documents into a collection in one call. Returns the new ids. */
  async bulkInsert(collection: string, documents: unknown[]): Promise<string[]> {
    const res = await this.request<{ ids: string[] }>(
      'POST',
      `/api/v1/documents/collections/${collection}/batch-insert`,
      { documents }
    );
    return res.ids ?? [];
  }

  /** Delete many documents by id in one call. Returns the number deleted. */
  async bulkDelete(collection: string, ids: string[]): Promise<number> {
    const res = await this.request<{ deleted: number }>(
      'POST',
      `/api/v1/documents/collections/${collection}/batch-delete`,
      { ids }
    );
    return res.deleted ?? 0;
  }

  /** Create a document collection. */
  async createCollection(name: string): Promise<unknown> {
    return await this.request('POST', '/api/v1/documents/collections', { name });
  }

  /** Insert a single document, optionally with an explicit id. Returns the new id. */
  async insertDocument(
    collection: string,
    document: unknown,
    id?: string
  ): Promise<string> {
    const res = await this.request<{ id: string }>(
      'POST',
      `/api/v1/documents/collections/${collection}/documents`,
      { id, document }
    );
    return res.id;
  }

  /** Get a document by id, or `undefined` if absent. */
  async getDocument(collection: string, id: string): Promise<unknown | undefined> {
    try {
      return await this.request(
        'GET',
        `/api/v1/documents/collections/${collection}/documents/${id}`
      );
    } catch {
      return undefined;
    }
  }

  /** Replace a document (full update). */
  async updateDocument(collection: string, id: string, document: unknown): Promise<unknown> {
    return await this.request(
      'PUT',
      `/api/v1/documents/collections/${collection}/documents/${id}`,
      { document }
    );
  }

  /** Partially update (merge) a document. */
  async patchDocument(collection: string, id: string, partial: unknown): Promise<unknown> {
    return await this.request(
      'PATCH',
      `/api/v1/documents/collections/${collection}/documents/${id}`,
      { document: partial }
    );
  }

  /** Delete a document by id. */
  async deleteDocument(collection: string, id: string): Promise<boolean> {
    try {
      await this.request(
        'DELETE',
        `/api/v1/documents/collections/${collection}/documents/${id}`
      );
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Query documents with a MongoDB-style filter. Pass `options.cursor` (from a
   * prior response's `next_cursor`) for pagination; the response includes
   * `next_cursor` when more pages exist.
   */
  async queryDocuments(
    collection: string,
    filter: Record<string, unknown> = {},
    options: { limit?: number; skip?: number; cursor?: string } = {}
  ): Promise<unknown> {
    return await this.request(
      'POST',
      `/api/v1/documents/collections/${collection}/query`,
      { filter, limit: options.limit, skip: options.skip, cursor: options.cursor }
    );
  }

  // ==========================================================================
  // Time Series
  // ==========================================================================

  /** Register a metric (e.g. `counter`, `gauge`, `histogram`, `summary`). */
  async registerMetric(name: string, metricType = 'gauge'): Promise<unknown> {
    return await this.request('POST', '/api/v1/timeseries/metrics', {
      name,
      metric_type: metricType,
    });
  }

  /** Write a single time-series point. */
  async tsWrite(
    metric: string,
    value: number,
    options: { timestamp?: number; tags?: Record<string, string> } = {}
  ): Promise<void> {
    await this.request('POST', '/api/v1/timeseries/write', {
      metric,
      value,
      timestamp: options.timestamp,
      tags: options.tags ?? {},
    });
  }

  /** Query a time series within an optional `[start, end]` window. */
  async tsQuery(
    metric: string,
    options: { start?: number; end?: number; limit?: number; tags?: Record<string, string> } = {}
  ): Promise<unknown> {
    return await this.request('POST', '/api/v1/timeseries/query', {
      metric,
      tags: options.tags,
      start: options.start,
      end: options.end,
      limit: options.limit,
    });
  }

  // ==========================================================================
  // Graph Database
  // ==========================================================================

  async getGraphData(): Promise<GraphData> {
    return await this.request<GraphData>('GET', '/api/v1/graph/data');
  }

  /** Create a graph node. */
  async createNode(label: string, properties: Record<string, unknown> = {}): Promise<unknown> {
    return await this.request('POST', '/api/v1/graph/nodes', { label, properties });
  }

  /** Update a graph node (omit a field to leave it unchanged). */
  async updateNode(
    nodeId: string,
    update: { label?: string; properties?: Record<string, unknown> }
  ): Promise<unknown> {
    return await this.request('PUT', `/api/v1/graph/nodes/${nodeId}`, update);
  }

  /** Delete a graph node (and its edges). */
  async deleteNode(nodeId: string): Promise<boolean> {
    try {
      await this.request('DELETE', `/api/v1/graph/nodes/${nodeId}`);
      return true;
    } catch {
      return false;
    }
  }

  /** Create a graph edge. */
  async createEdge(source: string, target: string, relationship: string): Promise<unknown> {
    return await this.request('POST', '/api/v1/graph/edges', {
      source,
      target,
      relationship,
    });
  }

  /** Update a graph edge's relationship. */
  async updateEdge(edgeId: string, relationship: string): Promise<unknown> {
    return await this.request('PUT', `/api/v1/graph/edges/${edgeId}`, { relationship });
  }

  /** Delete a graph edge. */
  async deleteEdge(edgeId: string): Promise<boolean> {
    try {
      await this.request('DELETE', `/api/v1/graph/edges/${edgeId}`);
      return true;
    } catch {
      return false;
    }
  }

  // ==========================================================================
  // Streaming (Server-Sent Events)
  // ==========================================================================

  /**
   * Subscribe to a streaming channel as an async iterator of events (SSE).
   * The channel is created on the server if it does not exist. Iteration ends
   * when the response stream closes; `break` out of the loop to disconnect.
   *
   *   for await (const event of client.subscribeChannel('cdc')) { ... }
   */
  async *subscribeChannel(channel: string): AsyncGenerator<unknown> {
    const url = `${this.config.url}/api/v1/streaming/channels/${channel}/sse`;
    const headers: Record<string, string> = { Accept: 'text/event-stream' };
    if (this.token) headers['Authorization'] = `Bearer ${this.token}`;
    if (this.config.apiKey) headers['X-API-Key'] = this.config.apiKey;

    const response = await fetch(url, { headers });
    if (!response.ok || !response.body) {
      throw new QueryError(`Subscribe failed (${response.status})`);
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        let sep: number;
        // SSE frames are separated by a blank line.
        while ((sep = buffer.indexOf('\n\n')) !== -1) {
          const frame = buffer.slice(0, sep);
          buffer = buffer.slice(sep + 2);
          for (const line of frame.split('\n')) {
            if (line.startsWith('data:')) {
              const data = line.slice(5).trim();
              try {
                yield JSON.parse(data);
              } catch {
                yield data;
              }
            }
          }
        }
      }
    } finally {
      reader.cancel().catch(() => undefined);
    }
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
  // Positional ($1, $2, ...) parameter values in order, matching the server.
  private whereParamValues: unknown[] = [];
  private orderByCols: string[] = [];
  private groupByCols: string[] = [];
  private limitVal?: number;
  private offsetVal?: number;
  private joins: string[] = [];

  constructor(client: AegisClient, table: string) {
    this.client = client;
    this.table = table;
  }

  private nextPlaceholder(value: unknown): string {
    this.whereParamValues.push(value);
    return `$${this.whereParamValues.length}`;
  }

  select(...columns: string[]): this {
    this.selectCols = columns.length ? columns : ['*'];
    return this;
  }

  where(column: string, operator: string, value: unknown): this {
    this.whereClauses.push(`${column} ${operator} ${this.nextPlaceholder(value)}`);
    return this;
  }

  whereIn(column: string, values: unknown[]): this {
    const placeholders = values.map((val) => this.nextPlaceholder(val));
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

  build(): { sql: string; params: unknown[] } {
    const parts = [`SELECT ${this.selectCols.join(', ')} FROM ${this.table}`];

    if (this.joins.length) parts.push(...this.joins);
    if (this.whereClauses.length) parts.push(`WHERE ${this.whereClauses.join(' AND ')}`);
    if (this.groupByCols.length) parts.push(`GROUP BY ${this.groupByCols.join(', ')}`);
    if (this.orderByCols.length) parts.push(`ORDER BY ${this.orderByCols.join(', ')}`);
    if (this.limitVal !== undefined) parts.push(`LIMIT ${this.limitVal}`);
    if (this.offsetVal !== undefined) parts.push(`OFFSET ${this.offsetVal}`);

    return { sql: parts.join(' '), params: this.whereParamValues };
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

  async execute(sql: string, params: unknown[] = []): Promise<number> {
    if (!this.active) throw new Error('No active transaction');
    return await this.client.execute(sql, params);
  }

  async query(sql: string, params: unknown[] = []): Promise<QueryResult> {
    if (!this.active) throw new Error('No active transaction');
    return await this.client.query(sql, params);
  }
}

// Default export
export default AegisClient;
