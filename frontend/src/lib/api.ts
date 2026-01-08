/**
 * Frostbit API client
 */

import { fetchWithTimeout } from './fetchWithTimeout.js';

export interface Service {
  name: string;
  has_logs: boolean;
  has_traces: boolean;
  has_metrics: boolean;
}

export interface LogStats {
  minute: string;
  count: number;
  error_count: number;
}

export interface TraceStats {
  minute: string;
  count: number;
  error_count: number;
  latency_sum_us?: number;
  latency_min_us?: number;
  latency_max_us?: number;
}

/**
 * Type guard for Service objects.
 * Accepts both boolean and numeric (0/1) values for has_logs/has_traces/has_metrics
 * since the API returns integers.
 */
function isServiceLike(obj: unknown): boolean {
  if (typeof obj !== 'object' || obj === null) return false;
  const s = obj as Record<string, unknown>;
  return (
    typeof s.name === 'string' &&
    (typeof s.has_logs === 'boolean' || typeof s.has_logs === 'number') &&
    (typeof s.has_traces === 'boolean' || typeof s.has_traces === 'number') &&
    (typeof s.has_metrics === 'boolean' || typeof s.has_metrics === 'number')
  );
}

/**
 * Convert API response to Service with boolean fields.
 */
function toService(obj: Record<string, unknown>): Service {
  return {
    name: obj.name as string,
    has_logs: Boolean(obj.has_logs),
    has_traces: Boolean(obj.has_traces),
    has_metrics: Boolean(obj.has_metrics),
  };
}

/**
 * Type guard for LogStats-like objects.
 * Accepts both string and number for minute since the API returns integers.
 */
function isLogStatsLike(obj: unknown): boolean {
  if (typeof obj !== 'object' || obj === null) return false;
  const s = obj as Record<string, unknown>;
  return (
    (typeof s.minute === 'string' || typeof s.minute === 'number') &&
    typeof s.count === 'number' &&
    typeof s.error_count === 'number'
  );
}

/**
 * Convert API response to LogStats with string minute.
 * Warns on semantic violations (negative counts, error_count > count).
 */
function toLogStats(obj: Record<string, unknown>): LogStats {
  const count = obj.count as number;
  const error_count = obj.error_count as number;

  // Warn on semantic violations (don't throw - data might still be useful)
  if (count < 0 || error_count < 0) {
    console.warn('Negative count in LogStats:', obj);
  }
  if (error_count > count) {
    console.warn('error_count exceeds count in LogStats:', obj);
  }

  return {
    minute: String(obj.minute),
    count,
    error_count,
  };
}

/**
 * Type guard for TraceStats-like objects.
 * Accepts both string and number for minute since the API returns integers.
 * Latency fields are optional (may be missing if no latency data recorded).
 */
function isTraceStatsLike(obj: unknown): boolean {
  if (typeof obj !== 'object' || obj === null) return false;
  const s = obj as Record<string, unknown>;
  // Required fields
  if (
    !(typeof s.minute === 'string' || typeof s.minute === 'number') ||
    typeof s.count !== 'number' ||
    typeof s.error_count !== 'number'
  ) {
    return false;
  }
  // Optional latency fields - must be number if present
  if (s.latency_sum_us !== undefined && typeof s.latency_sum_us !== 'number') return false;
  if (s.latency_min_us !== undefined && typeof s.latency_min_us !== 'number') return false;
  if (s.latency_max_us !== undefined && typeof s.latency_max_us !== 'number') return false;
  return true;
}

/**
 * Convert API response to TraceStats with string minute.
 * Warns on semantic violations (negative counts, error_count > count, invalid latency).
 */
function toTraceStats(obj: Record<string, unknown>): TraceStats {
  const count = obj.count as number;
  const error_count = obj.error_count as number;

  // Warn on semantic violations (don't throw - data might still be useful)
  if (count < 0 || error_count < 0) {
    console.warn('Negative count in TraceStats:', obj);
  }
  if (error_count > count) {
    console.warn('error_count exceeds count in TraceStats:', obj);
  }

  const result: TraceStats = {
    minute: String(obj.minute),
    count,
    error_count,
  };

  // Include optional latency fields if present
  if (obj.latency_sum_us !== undefined) {
    result.latency_sum_us = obj.latency_sum_us as number;
    if (result.latency_sum_us < 0) {
      console.warn('Negative latency_sum_us in TraceStats:', obj);
    }
  }
  if (obj.latency_min_us !== undefined) {
    result.latency_min_us = obj.latency_min_us as number;
  }
  if (obj.latency_max_us !== undefined) {
    result.latency_max_us = obj.latency_max_us as number;
  }

  // Validate min <= max when both present
  if (result.latency_min_us !== undefined && result.latency_max_us !== undefined) {
    if (result.latency_min_us > result.latency_max_us) {
      console.warn('latency_min_us exceeds latency_max_us in TraceStats:', obj);
    }
  }

  return result;
}

/**
 * Generic array validation and conversion helper.
 * Validates each item, converts valid ones, and logs/throws on failures.
 */
function validateAndConvert<T>(
  data: unknown[],
  validator: (item: unknown) => boolean,
  converter: (item: Record<string, unknown>) => T,
  typeName: string
): T[] {
  const results: T[] = [];
  const invalidIndices: number[] = [];

  for (let i = 0; i < data.length; i++) {
    if (validator(data[i])) {
      results.push(converter(data[i] as Record<string, unknown>));
    } else {
      console.warn(`Invalid ${typeName} at index`, i, ':', data[i]);
      invalidIndices.push(i);
    }
  }

  // If ALL items were invalid, this indicates an API compatibility issue
  if (data.length > 0 && results.length === 0) {
    console.error(`All ${typeName} failed validation:`, data);
    throw new Error(`API returned ${typeName.toLowerCase()} in unexpected format. Check API version compatibility.`);
  }

  // Log prominently if significant portion dropped
  if (invalidIndices.length > 0) {
    console.error(`Dropped ${invalidIndices.length} of ${data.length} ${typeName} due to validation failure`);
  }

  return results;
}

/**
 * Response from the all-services stats endpoint.
 */
export interface AllServicesStatsResponse {
  service: string;
  stats: LogStats[] | TraceStats[];
}

/**
 * Fetch stats for all services at once.
 * Uses the combined endpoint to reduce N+1 API calls.
 */
export async function fetchAllServicesStats(
  workerUrl: string,
  signal: 'logs' | 'traces',
  from: Date,
  to: Date
): Promise<AllServicesStatsResponse[]> {
  const params = new URLSearchParams({
    signal,
    from: from.toISOString(),
    to: to.toISOString(),
  });

  const url = `${workerUrl}/v1/services/stats?${params}`;

  const response = await fetchWithTimeout(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch all services stats: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of service stats, got:', typeof data);
    throw new Error('Invalid API response: expected array of service stats');
  }

  // Validate each service stats entry
  const results: AllServicesStatsResponse[] = [];
  let skippedEntries = 0;

  for (const entry of data) {
    if (typeof entry !== 'object' || entry === null) {
      skippedEntries++;
      continue;
    }
    const e = entry as Record<string, unknown>;
    if (typeof e.service !== 'string' || !Array.isArray(e.stats)) {
      skippedEntries++;
      continue;
    }

    // Validate stats array based on signal type
    const validator = signal === 'logs' ? isLogStatsLike : isTraceStatsLike;
    const converter = signal === 'logs' ? toLogStats : toTraceStats;
    const validStats = e.stats.filter(validator).map((s) => converter(s as Record<string, unknown>));

    results.push({
      service: e.service,
      stats: validStats,
    });
  }

  if (skippedEntries > 0) {
    console.error(
      `Dropped ${skippedEntries} of ${data.length} service stats entries due to invalid format`
    );
  }

  return results;
}

export async function fetchServices(workerUrl: string): Promise<Service[]> {
  const url = `${workerUrl}/v1/services`;

  const response = await fetchWithTimeout(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch services: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of services, got:', typeof data);
    throw new Error('Invalid API response: expected array of services');
  }

  return validateAndConvert(data, isServiceLike, toService, 'Service');
}

/**
 * Fetch log stats for a service within a time range.
 */
export async function fetchLogStats(
  workerUrl: string,
  service: string,
  from: Date,
  to: Date
): Promise<LogStats[]> {
  const params = new URLSearchParams({
    from: from.toISOString(),
    to: to.toISOString(),
  });

  const url = `${workerUrl}/v1/services/${encodeURIComponent(service)}/logs/stats?${params}`;

  const response = await fetchWithTimeout(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch log stats: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of LogStats, got:', typeof data);
    throw new Error('Invalid API response: expected array of LogStats');
  }

  return validateAndConvert(data, isLogStatsLike, toLogStats, 'LogStats');
}

/**
 * Fetch trace stats for a service within a time range.
 */
export async function fetchTraceStats(
  workerUrl: string,
  service: string,
  from: Date,
  to: Date
): Promise<TraceStats[]> {
  const params = new URLSearchParams({
    from: from.toISOString(),
    to: to.toISOString(),
  });

  const url = `${workerUrl}/v1/services/${encodeURIComponent(service)}/traces/stats?${params}`;

  const response = await fetchWithTimeout(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch trace stats: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of TraceStats, got:', typeof data);
    throw new Error('Invalid API response: expected array of TraceStats');
  }

  return validateAndConvert(data, isTraceStatsLike, toTraceStats, 'TraceStats');
}
