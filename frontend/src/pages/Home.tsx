import { useState, useMemo, useCallback } from 'react';
import { useCredentials } from '../hooks/useCredentials';
import { useServices } from '../hooks/useServices';
import { useStats, TIME_RANGES } from '../hooks/useStats';
import { useServiceStats } from '../hooks/useServiceStats';
import { ServiceHealthCards, type ServiceWithStats } from '../components/ServiceHealthCards';
import { LoadingSpinner, ErrorMessage } from '../components/LoadingState';

export function Home() {
  const { credentials } = useCredentials();
  const workerUrl = credentials?.workerUrl ?? null;

  // State for selected service (toggle selection on click)
  const [selectedService, setSelectedService] = useState<string | null>(null);

  // Search filter for services
  const [searchQuery, setSearchQuery] = useState('');

  // Fixed time range for detail stats (1 hour)
  const timeRange = TIME_RANGES[1];

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
  } = useStats(workerUrl, selectedService ?? '', timeRange);

  // Combined loading state (services or stats loading)
  const loading = servicesLoading || statsLoading;

  // Primary error to display (services error takes precedence)
  const error = servicesError ?? statsError;

  // Combine services with their error rates for the cards
  const servicesWithStats = useMemo<ServiceWithStats[]>(() => {
    const allServices = services.map((service) => {
      const stats = serviceStats.get(service.name);
      return {
        service,
        errorRate: stats?.errorRate ?? 0,
        totalCount: stats?.totalCount ?? 0,
        errorCount: stats?.errorCount ?? 0,
      };
    });

    // Filter by search query
    if (!searchQuery.trim()) return allServices;
    const query = searchQuery.toLowerCase();
    return allServices.filter((s) => s.service.name.toLowerCase().includes(query));
  }, [services, serviceStats, searchQuery]);

  // Handle service selection (toggle on click)
  const handleSelectService = useCallback((name: string | null) => {
    setSelectedService(name);
  }, []);

  // Detail stats for the selected service
  const detailStats = useMemo(() => {
    if (!selectedService) return undefined;
    return { logStats, traceStats };
  }, [selectedService, logStats, traceStats]);

  return (
    <div className="space-y-6">
      {/* Search filter */}
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
          {servicesWithStats.length} of {services.length} services
        </p>
        <div className="relative">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Filter services..."
            className="w-64 px-4 py-2 pl-10 text-sm rounded-lg transition-colors"
            style={{
              backgroundColor: 'white',
              border: '1px solid var(--color-border)',
              color: 'var(--color-text-primary)',
            }}
          />
          <svg
            className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
            style={{ color: 'var(--color-text-muted)' }}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
        </div>
      </div>

      {/* Loading state */}
      {loading && <LoadingSpinner />}

      {/* Error state */}
      {error && !loading && <ErrorMessage message={error} onRetry={refetchServices} />}

      {/* Service health cards */}
      {!loading && !error && (
        <ServiceHealthCards
          services={servicesWithStats}
          selectedService={selectedService}
          onSelectService={handleSelectService}
          detailStats={detailStats}
          detailLoading={detailLoading}
        />
      )}
    </div>
  );
}
