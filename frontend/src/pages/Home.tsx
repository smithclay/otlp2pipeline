import { useState, useMemo, useCallback, useEffect } from 'react';
import { useCredentials } from '../hooks/useCredentials';
import { useServices } from '../hooks/useServices';
import { useStats, TIME_RANGES } from '../hooks/useStats';
import { useServiceStats } from '../hooks/useServiceStats';
import { ServiceHealthCards, type ServiceWithStats } from '../components/ServiceHealthCards';
import { LoadingSpinner, ErrorMessage } from '../components/LoadingState';

function formatTimeAgo(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

export function Home() {
  const { credentials } = useCredentials();
  const workerUrl = credentials?.workerUrl ?? null;

  // State for selected service (toggle selection on click)
  const [selectedService, setSelectedService] = useState<string | null>(null);

  // Search filter for services
  const [searchQuery, setSearchQuery] = useState('');

  // Tick to force timestamp re-render every 10s
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 10000);
    return () => clearInterval(id);
  }, []);

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
    refetch: refetchStats,
    lastUpdated: statsLastUpdated,
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

  // Combined refetch that handles both error sources
  const handleRetry = useCallback(() => {
    if (servicesError) refetchServices();
    if (statsError) refetchStats();
    // If neither has an error but we're stale, refetch both
    if (!servicesError && !statsError) {
      refetchServices();
      refetchStats();
    }
  }, [servicesError, statsError, refetchServices, refetchStats]);

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
          {statsLastUpdated && (
            <span> · updated {formatTimeAgo(statsLastUpdated)}</span>
          )}
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

      {/* Error state - keep visible during loading so users know there's a problem */}
      {error && <ErrorMessage message={error} onRetry={handleRetry} />}

      {/* Service health cards - always render if we have data, even while refreshing */}
      {servicesWithStats.length > 0 || !loading ? (
        <div className="relative">
          <ServiceHealthCards
            services={servicesWithStats}
            selectedService={selectedService}
            onSelectService={handleSelectService}
            detailStats={detailStats}
            detailLoading={detailLoading}
          />
          {/* Overlay spinner for refreshes (not initial load) */}
          {loading && servicesWithStats.length > 0 && (
            <div className="absolute inset-0 bg-white/50 flex items-center justify-center rounded-xl pointer-events-none">
              <LoadingSpinner />
            </div>
          )}
        </div>
      ) : (
        /* Initial loading state (no data yet) */
        loading && <LoadingSpinner />
      )}
    </div>
  );
}
