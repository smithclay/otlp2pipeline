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
  latency_sum_us: number;
  latency_min_us: number;
  latency_max_us: number;
}

/**
 * Type guard for Service objects.
 */
function isService(obj: unknown): obj is Service {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    typeof (obj as Service).name === 'string' &&
    typeof (obj as Service).has_logs === 'boolean' &&
    typeof (obj as Service).has_traces === 'boolean'
  );
}

/**
 * Type guard for LogStats objects.
 */
function isLogStats(obj: unknown): obj is LogStats {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    typeof (obj as LogStats).minute === 'string' &&
    typeof (obj as LogStats).count === 'number' &&
    typeof (obj as LogStats).error_count === 'number'
  );
}

/**
 * Type guard for TraceStats objects.
 */
function isTraceStats(obj: unknown): obj is TraceStats {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    typeof (obj as TraceStats).minute === 'string' &&
    typeof (obj as TraceStats).count === 'number' &&
    typeof (obj as TraceStats).error_count === 'number' &&
    typeof (obj as TraceStats).latency_sum_us === 'number' &&
    typeof (obj as TraceStats).latency_min_us === 'number' &&
    typeof (obj as TraceStats).latency_max_us === 'number'
  );
}

/**
 * Validate an array of items using a type guard.
 * Logs warnings for invalid items and returns only valid ones.
 */
function validateArray<T>(
  data: unknown,
  guard: (item: unknown) => item is T,
  typeName: string
): T[] {
  if (!Array.isArray(data)) {
    console.error(`Expected array of ${typeName}, got:`, typeof data);
    throw new Error(`Invalid API response: expected array of ${typeName}`);
  }

  const valid: T[] = [];
  for (let i = 0; i < data.length; i++) {
    if (guard(data[i])) {
      valid.push(data[i]);
    } else {
      console.warn(`Invalid ${typeName} at index ${i}:`, data[i]);
    }
  }

  return valid;
}

export async function fetchServices(workerUrl: string): Promise<Service[]> {
  const url = `${workerUrl}/v1/services`;

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch services: ${response.status} ${response.statusText}`);
  }

  const data: unknown = await response.json();
  return validateArray(data, isService, 'Service');
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
  return validateArray(data, isLogStats, 'LogStats');
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
  return validateArray(data, isTraceStats, 'TraceStats');
}
