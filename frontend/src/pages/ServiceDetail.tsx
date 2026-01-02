import { useState, useMemo, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useCredentials } from '../hooks/useCredentials';
import { useStats, TIME_RANGES, TimeRange, LogStats, TraceStats } from '../hooks/useStats';
import { TimeRangePicker } from '../components/TimeRangePicker';
import { RedChart, ChartDataPoint } from '../components/RedChart';
import { RecordsPanel } from '../components/RecordsPanel';
import { LoadingSpinner, ErrorMessage } from '../components/LoadingState';

/**
 * Time range for records drilldown.
 */
interface DrilldownTimeRange {
  from: Date;
  to: Date;
}

/**
 * Merge log and trace stats into chart data points.
 * Extracts the specified field from each stat type and groups by minute.
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
    const existing = minuteMap.get(stat.minute);
    if (existing) {
      existing.logs = value;
    } else {
      minuteMap.set(stat.minute, { minute: stat.minute, logs: value });
    }
  }

  for (const stat of traceStats) {
    const value = stat[traceField];
    if (typeof value !== 'number') continue;
    const existing = minuteMap.get(stat.minute);
    if (existing) {
      existing.traces = value;
    } else {
      minuteMap.set(stat.minute, { minute: stat.minute, traces: value });
    }
  }

  return Array.from(minuteMap.values()).sort(
    (a, b) => new Date(a.minute).getTime() - new Date(b.minute).getTime()
  );
}

export function ServiceDetail() {
  const { name } = useParams<{ name: string }>();
  const { credentials } = useCredentials();
  const workerUrl = credentials?.workerUrl ?? null;

  // Default to last 1 hour
  const [timeRange, setTimeRange] = useState<TimeRange>(TIME_RANGES[1]);

  // Drilldown state: selected time point for records panel
  const [drilldownRange, setDrilldownRange] = useState<DrilldownTimeRange | null>(null);

  const { logStats, traceStats, loading, error, refetch } = useStats(
    workerUrl,
    name ?? '',
    timeRange
  );

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
      .map((stat) => {
        // Calculate average latency in milliseconds
        const avgLatencyMs = stat.count > 0 ? stat.latency_sum_us / stat.count / 1000 : 0;
        return {
          minute: stat.minute,
          traces: Math.round(avgLatencyMs * 100) / 100, // Round to 2 decimal places
        };
      })
      .sort((a, b) => new Date(a.minute).getTime() - new Date(b.minute).getTime());
  }, [traceStats]);

  // Handle chart point click for drilldown
  const handleChartClick = useCallback((minute: string) => {
    const clickedTime = new Date(minute);
    // Create a time range of +/- 30 seconds around the clicked point
    const from = new Date(clickedTime.getTime() - 30 * 1000);
    const to = new Date(clickedTime.getTime() + 30 * 1000);
    setDrilldownRange({ from, to });
  }, []);

  // Close the records drilldown panel
  const handleCloseDrilldown = useCallback(() => {
    setDrilldownRange(null);
  }, []);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Link
            to="/"
            className="text-slate-400 hover:text-slate-100 transition-colors"
          >
            Services
          </Link>
          <span className="text-slate-600">/</span>
          <h1 className="text-xl font-semibold text-cyan-500">{name}</h1>
        </div>
        <TimeRangePicker value={timeRange} onChange={setTimeRange} />
      </div>

      {/* Content */}
      {loading && <LoadingSpinner />}

      {error && <ErrorMessage message={error} onRetry={refetch} />}

      {!loading && !error && (
        <div className="space-y-4">
          {/* Request Rate Chart */}
          <RedChart
            title="Request Rate"
            data={rateData}
            yLabel="Requests per minute"
            onPointClick={handleChartClick}
          />

          {/* Error Rate Chart */}
          <RedChart
            title="Error Rate"
            data={errorData}
            yLabel="Errors per minute"
            onPointClick={handleChartClick}
          />

          {/* Latency Chart (traces only) */}
          <RedChart
            title="Latency (traces only)"
            data={latencyData}
            yLabel="Average latency (ms)"
            onPointClick={handleChartClick}
          />

          {/* Records Drilldown Panel */}
          {drilldownRange && name && (
            <RecordsPanel
              service={name}
              timeRange={drilldownRange}
              onClose={handleCloseDrilldown}
            />
          )}
        </div>
      )}
    </div>
  );
}
