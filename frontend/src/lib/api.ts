/**
 * Frostbit API client
 */

/** Default timeout for API requests (5 minutes) */
const DEFAULT_TIMEOUT_MS = 300000;

/**
 * Fetch with timeout and abort support.
 * @param url - URL to fetch
 * @param options - Fetch options
 * @param timeoutMs - Timeout in milliseconds (default: 30000)
 * @returns Response from fetch
 * @throws Error if request times out or fails
 */
async function fetchWithTimeout(
  url: string,
  options: RequestInit = {},
  timeoutMs: number = DEFAULT_TIMEOUT_MS
): Promise<Response> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    return response;
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error(`Request timed out after ${timeoutMs}ms`);
    }
    throw error;
  } finally {
    clearTimeout(timeoutId);
  }
}

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
 */
function toLogStats(obj: Record<string, unknown>): LogStats {
  return {
    minute: String(obj.minute),
    count: obj.count as number,
    error_count: obj.error_count as number,
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
 */
function toTraceStats(obj: Record<string, unknown>): TraceStats {
  const result: TraceStats = {
    minute: String(obj.minute),
    count: obj.count as number,
    error_count: obj.error_count as number,
  };
  // Include optional latency fields if present
  if (obj.latency_sum_us !== undefined) result.latency_sum_us = obj.latency_sum_us as number;
  if (obj.latency_min_us !== undefined) result.latency_min_us = obj.latency_min_us as number;
  if (obj.latency_max_us !== undefined) result.latency_max_us = obj.latency_max_us as number;
  return result;
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
  for (const entry of data) {
    if (typeof entry !== 'object' || entry === null) continue;
    const e = entry as Record<string, unknown>;
    if (typeof e.service !== 'string' || !Array.isArray(e.stats)) continue;

    // Validate stats array based on signal type
    const validator = signal === 'logs' ? isLogStatsLike : isTraceStatsLike;
    const converter = signal === 'logs' ? toLogStats : toTraceStats;
    const validStats = e.stats.filter(validator).map((s) => converter(s as Record<string, unknown>));

    results.push({
      service: e.service,
      stats: validStats,
    });
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

  const services: Service[] = [];
  const invalidIndices: number[] = [];

  for (let i = 0; i < data.length; i++) {
    if (isServiceLike(data[i])) {
      services.push(toService(data[i] as Record<string, unknown>));
    } else {
      console.warn('Invalid service at index', i, ':', data[i]);
      invalidIndices.push(i);
    }
  }

  // If ALL items were invalid, this indicates an API compatibility issue
  if (data.length > 0 && services.length === 0) {
    console.error('All services failed validation:', data);
    throw new Error('API returned data in unexpected format. Check API version compatibility.');
  }

  // Log prominently if significant portion dropped
  if (invalidIndices.length > 0) {
    console.error(`Dropped ${invalidIndices.length} of ${data.length} services due to validation failure`);
  }

  return services;
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

  const stats: LogStats[] = [];
  const invalidIndices: number[] = [];

  for (let i = 0; i < data.length; i++) {
    if (isLogStatsLike(data[i])) {
      stats.push(toLogStats(data[i] as Record<string, unknown>));
    } else {
      console.warn('Invalid LogStats at index', i, ':', data[i]);
      invalidIndices.push(i);
    }
  }

  // If ALL items were invalid, this indicates an API compatibility issue
  if (data.length > 0 && stats.length === 0) {
    console.error('All LogStats failed validation:', data);
    throw new Error('API returned log stats in unexpected format. Check API version compatibility.');
  }

  // Log prominently if significant portion dropped
  if (invalidIndices.length > 0) {
    console.error(`Dropped ${invalidIndices.length} of ${data.length} LogStats due to validation failure`);
  }

  return stats;
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

  const stats: TraceStats[] = [];
  const invalidIndices: number[] = [];

  for (let i = 0; i < data.length; i++) {
    if (isTraceStatsLike(data[i])) {
      stats.push(toTraceStats(data[i] as Record<string, unknown>));
    } else {
      console.warn('Invalid TraceStats at index', i, ':', data[i]);
      invalidIndices.push(i);
    }
  }

  // If ALL items were invalid, this indicates an API compatibility issue
  if (data.length > 0 && stats.length === 0) {
    console.error('All TraceStats failed validation:', data);
    throw new Error('API returned trace stats in unexpected format. Check API version compatibility.');
  }

  // Log prominently if significant portion dropped
  if (invalidIndices.length > 0) {
    console.error(`Dropped ${invalidIndices.length} of ${data.length} TraceStats due to validation failure`);
  }

  return stats;
}
