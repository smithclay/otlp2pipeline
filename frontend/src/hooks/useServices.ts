import { useState, useEffect, useCallback } from 'react';
import { Service, fetchServices } from '../lib/api';

export interface UseServicesResult {
  services: Service[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useServices(workerUrl: string | null): UseServicesResult {
  const [services, setServices] = useState<Service[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

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
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch services';
      setError(message);
      setServices([]);
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
  };
}
