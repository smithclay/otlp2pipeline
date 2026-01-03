import { useState, useEffect, useCallback } from 'react';
import type { Service } from '../lib/api';
import { fetchAllServicesStats } from '../lib/api';

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
 * Hook to fetch error stats for all services.
 * Uses the combined /v1/services/stats endpoint for efficiency (2 requests instead of 2*N).
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

    try {
      // Fetch logs and traces stats for ALL services in just 2 requests
      const [logResults, traceResults] = await Promise.all([
        fetchAllServicesStats(workerUrl, 'logs', from, to),
        fetchAllServicesStats(workerUrl, 'traces', from, to),
      ]);

      // Build a map of service name to accumulated counts
      const newStats = new Map<string, ServiceErrorStats>();

      // Initialize all services with zero counts
      for (const service of services) {
        newStats.set(service.name, {
          name: service.name,
          errorRate: 0,
          totalCount: 0,
          errorCount: 0,
        });
      }

      // Accumulate log stats
      for (const result of logResults) {
        const existing = newStats.get(result.service);
        if (existing) {
          for (const stat of result.stats) {
            existing.totalCount += stat.count;
            existing.errorCount += stat.error_count;
          }
        }
      }

      // Accumulate trace stats
      for (const result of traceResults) {
        const existing = newStats.get(result.service);
        if (existing) {
          for (const stat of result.stats) {
            existing.totalCount += stat.count;
            existing.errorCount += stat.error_count;
          }
        }
      }

      // Calculate error rates
      for (const stat of newStats.values()) {
        stat.errorRate = stat.totalCount > 0 ? (stat.errorCount / stat.totalCount) * 100 : 0;
      }

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
