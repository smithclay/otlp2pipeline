import { useState, useMemo, useCallback } from 'react';
import { useCredentials } from '../hooks/useCredentials';
import { useServices } from '../hooks/useServices';
import { useStats, TIME_RANGES, TimeRange, LogStats, TraceStats } from '../hooks/useStats';
import { useServiceStats } from '../hooks/useServiceStats';
import { TimeRangePicker } from '../components/TimeRangePicker';
import { HoneycombGrid, type ServiceWithStats } from '../components/HoneycombGrid';
import { RedChart, type ChartDataPoint } from '../components/RedChart';
import { LoadingSpinner, ErrorMessage } from '../components/LoadingState';

/**
 * Convert a minute bucket (Unix timestamp / 60) to an ISO timestamp string.
 */
function minuteBucketToTimestamp(minuteBucket: string): string {
  const bucket = parseInt(minuteBucket, 10);
  return new Date(bucket * 60 * 1000).toISOString();
}

/**
 * Merge log and trace stats into chart data points.
 * Extracts the specified field from each stat type and groups by minute.
 * Converts minute buckets to ISO timestamps for display.
 */
function mergeStatsToChartData(
  logStats: LogStats[],
  traceStats: TraceStats[],
  logField: keyof LogStats,
  traceField: keyof TraceStats
): ChartDataPoint[] {
  const minuteMap = new Map<string, ChartDataPoint>();

  for (const stat of logStats) {
    const value = stat[logField];
    if (typeof value !== 'number') continue;
    const timestamp = minuteBucketToTimestamp(stat.minute);
    const existing = minuteMap.get(stat.minute);
    if (existing) {
      existing.logs = value;
    } else {
      minuteMap.set(stat.minute, { minute: timestamp, logs: value });
    }
  }

  for (const stat of traceStats) {
    const value = stat[traceField];
    if (typeof value !== 'number') continue;
    const timestamp = minuteBucketToTimestamp(stat.minute);
    const existing = minuteMap.get(stat.minute);
    if (existing) {
      existing.traces = value;
    } else {
      minuteMap.set(stat.minute, { minute: timestamp, traces: value });
    }
  }

  return Array.from(minuteMap.values()).sort(
    (a, b) => new Date(a.minute).getTime() - new Date(b.minute).getTime()
  );
}

export function Home() {
  const { credentials } = useCredentials();
  const workerUrl = credentials?.workerUrl ?? null;

  // State for selected service (toggle selection on click)
  const [selectedService, setSelectedService] = useState<string | null>(null);

  // Default to last 1 hour for charts
  const [timeRange, setTimeRange] = useState<TimeRange>(TIME_RANGES[1]);

  // Fetch list of services
  const {
    services,
    loading: servicesLoading,
    error: servicesError,
    refetch: refetchServices,
  } = useServices(workerUrl);

  // Fetch error stats for all services (for traffic light display)
  const {
    stats: serviceStats,
    loading: statsLoading,
    error: statsError,
  } = useServiceStats(workerUrl, services);

  // Fetch detailed stats for selected service
  const {
    logStats,
    traceStats,
    loading: detailLoading,
    error: detailError,
    refetch: refetchDetail,
  } = useStats(workerUrl, selectedService ?? '', timeRange);

  // Combined loading state (services or stats loading)
  const loading = servicesLoading || statsLoading;

  // Primary error to display (services error takes precedence)
  const error = servicesError ?? statsError;

  // Combine services with their error rates for the HoneycombGrid
  const servicesWithStats = useMemo<ServiceWithStats[]>(() => {
    return services.map((service) => {
      const stats = serviceStats.get(service.name);
      return {
        service,
        errorRate: stats?.errorRate ?? 0,
      };
    });
  }, [services, serviceStats]);

  // Handle service selection (toggle on click)
  const handleSelectService = useCallback((name: string) => {
    setSelectedService((prev) => (prev === name ? null : name));
  }, []);

  // Transform stats data for Rate chart (logs and traces count over time)
  const rateData = useMemo<ChartDataPoint[]>(
    () => mergeStatsToChartData(logStats, traceStats, 'count', 'count'),
    [logStats, traceStats]
  );

  // Transform stats data for Error Rate chart
  const errorData = useMemo<ChartDataPoint[]>(
    () => mergeStatsToChartData(logStats, traceStats, 'error_count', 'error_count'),
    [logStats, traceStats]
  );

  // Transform stats data for Latency chart (traces only, average latency)
  const latencyData = useMemo<ChartDataPoint[]>(() => {
    return traceStats
      .filter((stat) => stat.latency_sum_us !== undefined)
      .map((stat) => {
        // Calculate average latency in milliseconds
        const avgLatencyMs = stat.count > 0 ? (stat.latency_sum_us ?? 0) / stat.count / 1000 : 0;
        return {
          minute: minuteBucketToTimestamp(stat.minute),
          traces: Math.round(avgLatencyMs * 100) / 100, // Round to 2 decimal places
        };
      })
      .sort((a, b) => new Date(a.minute).getTime() - new Date(b.minute).getTime());
  }, [traceStats]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold text-slate-100">Services</h1>
        <TimeRangePicker value={timeRange} onChange={setTimeRange} />
      </div>

      {/* Loading state */}
      {loading && <LoadingSpinner />}

      {/* Error state */}
      {error && !loading && <ErrorMessage message={error} onRetry={refetchServices} />}

      {/* Main content */}
      {!loading && !error && (
        <>
          {/* Honeycomb grid of services */}
          <HoneycombGrid
            services={servicesWithStats}
            selectedService={selectedService}
            onSelectService={handleSelectService}
          />

          {/* Selected service detail section */}
          {selectedService && (
            <>
              {/* Divider */}
              <div className="border-t border-slate-700" />

              {/* Service detail header */}
              <div className="flex items-center justify-between">
                <h2 className="text-lg font-medium text-cyan-500">{selectedService}</h2>
                <button
                  onClick={() => setSelectedService(null)}
                  className="text-sm text-slate-400 hover:text-slate-200 transition-colors"
                >
                  Close
                </button>
              </div>

              {/* Detail loading state */}
              {detailLoading && <LoadingSpinner />}

              {/* Detail error state */}
              {detailError && !detailLoading && (
                <ErrorMessage message={detailError} onRetry={refetchDetail} />
              )}

              {/* RED Charts */}
              {!detailLoading && !detailError && (
                <div className="space-y-4">
                  {/* Request Rate Chart */}
                  <RedChart
                    title="Request Rate"
                    data={rateData}
                    yLabel="Requests per minute"
                  />

                  {/* Error Rate Chart */}
                  <RedChart
                    title="Error Rate"
                    data={errorData}
                    yLabel="Errors per minute"
                  />

                  {/* Latency Chart (traces only) */}
                  <RedChart
                    title="Latency (traces only)"
                    data={latencyData}
                    yLabel="Average latency (ms)"
                  />
                </div>
              )}
            </>
          )}
        </>
      )}
    </div>
  );
}
