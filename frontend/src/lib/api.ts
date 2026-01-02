/**
 * Frostbit API client
 */

export interface Service {
  name: string;
  has_logs: boolean;
  has_traces: boolean;
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
 * Accepts both boolean and numeric (0/1) values for has_logs/has_traces
 * since the API returns integers.
 */
function isServiceLike(obj: unknown): boolean {
  if (typeof obj !== 'object' || obj === null) return false;
  const s = obj as Record<string, unknown>;
  return (
    typeof s.name === 'string' &&
    (typeof s.has_logs === 'boolean' || typeof s.has_logs === 'number') &&
    (typeof s.has_traces === 'boolean' || typeof s.has_traces === 'number')
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

export async function fetchServices(workerUrl: string): Promise<Service[]> {
  const url = `${workerUrl}/v1/services`;

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch services: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of services, got:', typeof data);
    throw new Error('Invalid API response: expected array of services');
  }

  const services: Service[] = [];
  for (let i = 0; i < data.length; i++) {
    if (isServiceLike(data[i])) {
      services.push(toService(data[i] as Record<string, unknown>));
    } else {
      console.warn('Invalid service at index', i, ':', data[i]);
    }
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

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch log stats: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of LogStats, got:', typeof data);
    throw new Error('Invalid API response: expected array of LogStats');
  }

  const stats: LogStats[] = [];
  for (let i = 0; i < data.length; i++) {
    if (isLogStatsLike(data[i])) {
      stats.push(toLogStats(data[i] as Record<string, unknown>));
    } else {
      console.warn('Invalid LogStats at index', i, ':', data[i]);
    }
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

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch trace stats: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();

  if (!Array.isArray(data)) {
    console.error('Expected array of TraceStats, got:', typeof data);
    throw new Error('Invalid API response: expected array of TraceStats');
  }

  const stats: TraceStats[] = [];
  for (let i = 0; i < data.length; i++) {
    if (isTraceStatsLike(data[i])) {
      stats.push(toTraceStats(data[i] as Record<string, unknown>));
    } else {
      console.warn('Invalid TraceStats at index', i, ':', data[i]);
    }
  }

  return stats;
}
