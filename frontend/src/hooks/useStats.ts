import { useState, useEffect, useCallback } from 'react';
import { LogStats, TraceStats, fetchLogStats, fetchTraceStats } from '../lib/api';

/**
 * Time range configuration for stats queries.
 */
export interface TimeRange {
  label: string;
  value: string;
  from: () => Date;
}

/**
 * Available time ranges for the picker.
 */
export const TIME_RANGES: TimeRange[] = [
  {
    label: 'Last 15 minutes',
    value: '15m',
    from: () => new Date(Date.now() - 15 * 60 * 1000),
  },
  {
    label: 'Last 1 hour',
    value: '1h',
    from: () => new Date(Date.now() - 60 * 60 * 1000),
  },
  {
    label: 'Last 6 hours',
    value: '6h',
    from: () => new Date(Date.now() - 6 * 60 * 60 * 1000),
  },
  {
    label: 'Last 24 hours',
    value: '24h',
    from: () => new Date(Date.now() - 24 * 60 * 60 * 1000),
  },
  {
    label: 'Last 7 days',
    value: '7d',
    from: () => new Date(Date.now() - 7 * 24 * 60 * 60 * 1000),
  },
];

/**
 * Result returned by the useStats hook.
 */
export interface UseStatsResult {
  logStats: LogStats[];
  traceStats: TraceStats[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * Hook to fetch log and trace stats for a service within a time range.
 */
export function useStats(
  workerUrl: string | null,
  service: string,
  timeRange: TimeRange
): UseStatsResult {
  const [logStats, setLogStats] = useState<LogStats[]>([]);
  const [traceStats, setTraceStats] = useState<TraceStats[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!workerUrl || !service) {
      setLogStats([]);
      setTraceStats([]);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);

    const from = timeRange.from();
    const to = new Date();

    try {
      // Fetch both log and trace stats in parallel
      const [logs, traces] = await Promise.all([
        fetchLogStats(workerUrl, service, from, to).catch(() => [] as LogStats[]),
        fetchTraceStats(workerUrl, service, from, to).catch(() => [] as TraceStats[]),
      ]);

      setLogStats(logs);
      setTraceStats(traces);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch stats';
      setError(message);
      setLogStats([]);
      setTraceStats([]);
    } finally {
      setLoading(false);
    }
  }, [workerUrl, service, timeRange]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return {
    logStats,
    traceStats,
    loading,
    error,
    refetch: fetchData,
  };
}
