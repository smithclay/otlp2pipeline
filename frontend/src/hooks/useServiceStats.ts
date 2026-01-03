import { useState, useEffect, useCallback } from 'react';
import type { Service } from '../lib/api';
import { fetchLogStats, fetchTraceStats } from '../lib/api';

export interface ServiceErrorStats {
  name: string;
  errorRate: number;
  totalCount: number;
  errorCount: number;
}

export interface UseServiceStatsResult {
  stats: Map<string, ServiceErrorStats>;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * Calculate error rate from stats arrays.
 */
function calculateErrorRate(
  logStats: { count: number; error_count: number }[],
  traceStats: { count: number; error_count: number }[]
): { totalCount: number; errorCount: number; errorRate: number } {
  let totalCount = 0;
  let errorCount = 0;

  for (const stat of logStats) {
    totalCount += stat.count;
    errorCount += stat.error_count;
  }

  for (const stat of traceStats) {
    totalCount += stat.count;
    errorCount += stat.error_count;
  }

  const errorRate = totalCount > 0 ? (errorCount / totalCount) * 100 : 0;

  return { totalCount, errorCount, errorRate };
}

/**
 * Hook to fetch error stats for all services.
 * Returns a map of service name to error rate for traffic light display.
 */
export function useServiceStats(
  workerUrl: string | null,
  services: Service[]
): UseServiceStatsResult {
  const [stats, setStats] = useState<Map<string, ServiceErrorStats>>(new Map());
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!workerUrl || services.length === 0) {
      setStats(new Map());
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);

    // Query last 15 minutes for traffic light calculation
    const from = new Date(Date.now() - 15 * 60 * 1000);
    const to = new Date();

    const newStats = new Map<string, ServiceErrorStats>();

    try {
      // Fetch stats for all services in parallel
      await Promise.all(
        services.map(async (service) => {
          try {
            const [logStats, traceStats] = await Promise.all([
              service.has_logs
                ? fetchLogStats(workerUrl, service.name, from, to)
                : Promise.resolve([]),
              service.has_traces
                ? fetchTraceStats(workerUrl, service.name, from, to)
                : Promise.resolve([]),
            ]);

            const { totalCount, errorCount, errorRate } = calculateErrorRate(
              logStats,
              traceStats
            );

            newStats.set(service.name, {
              name: service.name,
              errorRate,
              totalCount,
              errorCount,
            });
          } catch (err) {
            console.warn(`Failed to fetch stats for ${service.name}:`, err);
            // Default to 0% error rate on failure
            newStats.set(service.name, {
              name: service.name,
              errorRate: 0,
              totalCount: 0,
              errorCount: 0,
            });
          }
        })
      );

      setStats(newStats);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch stats';
      console.error('Stats fetch error:', err);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [workerUrl, services]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  // Poll every 30 seconds
  useEffect(() => {
    if (!workerUrl || services.length === 0) return;

    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, [workerUrl, services, fetchData]);

  return {
    stats,
    loading,
    error,
    refetch: fetchData,
  };
}
