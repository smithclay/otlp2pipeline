import { useState, useEffect, useCallback } from 'react';
import { Service, fetchServices } from '../lib/api';

export interface UseServicesResult {
  services: Service[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
  lastUpdated: Date | null;
  isStale: boolean;
}

export function useServices(workerUrl: string | null): UseServicesResult {
  const [services, setServices] = useState<Service[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  // Data is stale if last successful fetch was more than 1 minute ago
  const isStale = lastUpdated !== null && Date.now() - lastUpdated.getTime() > 60000;

  const fetchData = useCallback(async () => {
    if (!workerUrl) {
      setServices([]);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const data = await fetchServices(workerUrl);
      setServices(data);
      setLastUpdated(new Date());
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch services';
      setError(message);
      // Keep previous services on error - don't clear
    } finally {
      setLoading(false);
    }
  }, [workerUrl]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return {
    services,
    loading,
    error,
    refetch: fetchData,
    lastUpdated,
    isStale,
  };
}
