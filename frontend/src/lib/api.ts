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

export async function fetchServices(workerUrl: string): Promise<Service[]> {
  const url = `${workerUrl}/v1/services`;

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`Failed to fetch services: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();
  return data as Service[];
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

  const data = await response.json();
  return data as LogStats[];
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

  const data = await response.json();
  return data as TraceStats[];
}
