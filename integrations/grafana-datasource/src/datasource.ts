/**
 * Aegis DB Grafana Data Source
 *
 * Provides integration between Grafana and Aegis Database for:
 * - SQL queries
 * - Time series data
 * - Metrics and logs
 * - Annotations
 *
 * @version 1.0.0
 * @author AutomataNexus Development Team
 */

import {
  DataSourceApi,
  DataSourceInstanceSettings,
  DataQueryRequest,
  DataQueryResponse,
  DataSourceJsonData,
  MetricFindValue,
  ScopedVars,
  TimeRange,
  AnnotationEvent,
  AnnotationQueryRequest,
} from '@grafana/data';

import { getBackendSrv, getTemplateSrv } from '@grafana/runtime';

// Query interface
export interface AegisQuery {
  refId: string;
  queryText: string;
  queryType: 'sql' | 'timeseries' | 'metrics' | 'logs';
  table?: string;
  metric?: string;
  aggregation?: 'sum' | 'avg' | 'min' | 'max' | 'count';
  groupBy?: string[];
  filters?: AegisFilter[];
  maxDataPoints?: number;
}

export interface AegisFilter {
  field: string;
  operator: '=' | '!=' | '>' | '<' | '>=' | '<=' | 'LIKE' | 'IN';
  value: string | number | string[];
}

// Data source configuration
export interface AegisDataSourceOptions extends DataSourceJsonData {
  url: string;
  database?: string;
  username?: string;
  tlsEnabled?: boolean;
}

// Secure configuration (stored encrypted)
export interface AegisSecureJsonData {
  password?: string;
  apiKey?: string;
}

/**
 * Aegis Database Data Source for Grafana
 */
export class AegisDataSource extends DataSourceApi<AegisQuery, AegisDataSourceOptions> {
  url: string;
  database: string;
  username: string;

  constructor(instanceSettings: DataSourceInstanceSettings<AegisDataSourceOptions>) {
    super(instanceSettings);
    this.url = instanceSettings.jsonData.url || 'http://localhost:8080';
    this.database = instanceSettings.jsonData.database || 'default';
    this.username = instanceSettings.jsonData.username || '';
  }

  /**
   * Execute queries against Aegis DB
   */
  async query(options: DataQueryRequest<AegisQuery>): Promise<DataQueryResponse> {
    const { range, targets, maxDataPoints } = options;

    const promises = targets
      .filter(target => !target.hide && target.queryText)
      .map(target => this.executeQuery(target, range, maxDataPoints));

    const results = await Promise.all(promises);

    return { data: results.flat() };
  }

  /**
   * Execute a single query
   */
  private async executeQuery(
    query: AegisQuery,
    range: TimeRange,
    maxDataPoints?: number
  ): Promise<any[]> {
    const templateSrv = getTemplateSrv();
    const interpolatedQuery = templateSrv.replace(query.queryText, {});

    const payload = {
      query: interpolatedQuery,
      queryType: query.queryType || 'sql',
      database: this.database,
      timeRange: {
        from: range.from.valueOf(),
        to: range.to.valueOf(),
      },
      maxDataPoints: maxDataPoints || 1000,
      aggregation: query.aggregation,
      groupBy: query.groupBy,
      filters: query.filters,
    };

    try {
      const response = await getBackendSrv().post(
        `${this.url}/api/v1/query`,
        payload
      );

      return this.transformResponse(response, query);
    } catch (error) {
      console.error('Aegis query failed:', error);
      throw error;
    }
  }

  /**
   * Transform Aegis response to Grafana format
   */
  private transformResponse(response: any, query: AegisQuery): any[] {
    if (!response || !response.rows) {
      return [];
    }

    const { columns, rows } = response;

    // Time series data
    if (query.queryType === 'timeseries' || this.isTimeSeriesData(columns, rows)) {
      return this.transformToTimeSeries(columns, rows, query);
    }

    // Table data
    return this.transformToTable(columns, rows, query);
  }

  /**
   * Check if data appears to be time series
   */
  private isTimeSeriesData(columns: string[], rows: any[][]): boolean {
    if (!columns || columns.length < 2) return false;
    const timeCol = columns[0].toLowerCase();
    return timeCol === 'time' || timeCol === 'timestamp' || timeCol === 'ts';
  }

  /**
   * Transform to Grafana time series format
   */
  private transformToTimeSeries(columns: string[], rows: any[][], query: AegisQuery): any[] {
    const timeIndex = 0;
    const valueColumns = columns.slice(1);

    return valueColumns.map((colName, idx) => ({
      target: colName,
      refId: query.refId,
      datapoints: rows.map(row => [
        row[idx + 1], // value
        row[timeIndex], // timestamp
      ]),
    }));
  }

  /**
   * Transform to Grafana table format
   */
  private transformToTable(columns: string[], rows: any[][], query: AegisQuery): any[] {
    return [{
      refId: query.refId,
      columns: columns.map(name => ({ text: name, type: 'string' })),
      rows: rows,
      type: 'table',
    }];
  }

  /**
   * Test data source connection
   */
  async testDatasource(): Promise<{ status: string; message: string }> {
    try {
      const response = await getBackendSrv().get(`${this.url}/health`);

      if (response.status === 'healthy') {
        return {
          status: 'success',
          message: 'Aegis DB connection successful',
        };
      }

      return {
        status: 'error',
        message: `Aegis DB unhealthy: ${response.status}`,
      };
    } catch (error: any) {
      return {
        status: 'error',
        message: `Connection failed: ${error.message || 'Unknown error'}`,
      };
    }
  }

  /**
   * Get metric suggestions for query editor
   */
  async metricFindQuery(query: string, options?: any): Promise<MetricFindValue[]> {
    const interpolatedQuery = getTemplateSrv().replace(query, options?.scopedVars);

    try {
      // Get tables
      if (interpolatedQuery === '*' || interpolatedQuery === 'tables') {
        const response = await getBackendSrv().get(`${this.url}/api/v1/tables`);
        return response.tables.map((t: any) => ({ text: t.name, value: t.name }));
      }

      // Get columns for a table
      if (interpolatedQuery.startsWith('columns:')) {
        const table = interpolatedQuery.replace('columns:', '');
        const response = await getBackendSrv().get(`${this.url}/api/v1/tables/${table}`);
        return response.columns.map((c: any) => ({ text: c.name, value: c.name }));
      }

      // Get metrics
      if (interpolatedQuery === 'metrics') {
        const response = await getBackendSrv().get(`${this.url}/api/v1/metrics`);
        return Object.keys(response).map(m => ({ text: m, value: m }));
      }

      // Execute custom query for variable values
      const response = await getBackendSrv().post(`${this.url}/api/v1/query`, {
        query: interpolatedQuery,
        queryType: 'sql',
        database: this.database,
      });

      if (response.rows && response.rows.length > 0) {
        return response.rows.map((row: any[]) => ({
          text: String(row[0]),
          value: String(row[0]),
        }));
      }

      return [];
    } catch (error) {
      console.error('metricFindQuery failed:', error);
      return [];
    }
  }

  /**
   * Get annotations from Aegis DB
   */
  async annotationQuery(request: AnnotationQueryRequest<AegisQuery>): Promise<AnnotationEvent[]> {
    const { annotation, range } = request;
    const query = annotation.query || '';

    if (!query) {
      return [];
    }

    try {
      const response = await getBackendSrv().post(`${this.url}/api/v1/query`, {
        query: getTemplateSrv().replace(query, {}),
        queryType: 'sql',
        database: this.database,
        timeRange: {
          from: range.from.valueOf(),
          to: range.to.valueOf(),
        },
      });

      if (!response.rows) {
        return [];
      }

      // Expected columns: time, title, text, tags (optional)
      const columns = response.columns || [];
      const timeIdx = columns.findIndex((c: string) =>
        c.toLowerCase() === 'time' || c.toLowerCase() === 'timestamp'
      );
      const titleIdx = columns.findIndex((c: string) => c.toLowerCase() === 'title');
      const textIdx = columns.findIndex((c: string) => c.toLowerCase() === 'text');
      const tagsIdx = columns.findIndex((c: string) => c.toLowerCase() === 'tags');

      return response.rows.map((row: any[]) => ({
        time: row[timeIdx >= 0 ? timeIdx : 0],
        title: row[titleIdx >= 0 ? titleIdx : 1] || 'Event',
        text: row[textIdx >= 0 ? textIdx : 2] || '',
        tags: tagsIdx >= 0 && row[tagsIdx] ? String(row[tagsIdx]).split(',') : [],
      }));
    } catch (error) {
      console.error('annotationQuery failed:', error);
      return [];
    }
  }

  /**
   * Get tag keys for ad-hoc filters
   */
  async getTagKeys(): Promise<MetricFindValue[]> {
    try {
      const response = await getBackendSrv().get(`${this.url}/api/v1/tables`);
      const tagKeys: MetricFindValue[] = [];

      for (const table of response.tables || []) {
        const tableInfo = await getBackendSrv().get(`${this.url}/api/v1/tables/${table.name}`);
        for (const col of tableInfo.columns || []) {
          tagKeys.push({ text: `${table.name}.${col.name}`, value: `${table.name}.${col.name}` });
        }
      }

      return tagKeys;
    } catch (error) {
      console.error('getTagKeys failed:', error);
      return [];
    }
  }

  /**
   * Get tag values for ad-hoc filters
   */
  async getTagValues(options: { key: string }): Promise<MetricFindValue[]> {
    const [table, column] = options.key.split('.');

    if (!table || !column) {
      return [];
    }

    try {
      const response = await getBackendSrv().post(`${this.url}/api/v1/query`, {
        query: `SELECT DISTINCT ${column} FROM ${table} LIMIT 100`,
        queryType: 'sql',
        database: this.database,
      });

      if (!response.rows) {
        return [];
      }

      return response.rows.map((row: any[]) => ({
        text: String(row[0]),
        value: String(row[0]),
      }));
    } catch (error) {
      console.error('getTagValues failed:', error);
      return [];
    }
  }
}
