import { useCredentials } from '../hooks/useCredentials';
import { useServices } from '../hooks/useServices';
import { ServiceCard } from '../components/ServiceCard';
import { LoadingSpinner, ErrorMessage } from '../components/LoadingState';

function EmptyState() {
  return (
    <div className="rounded-lg border border-slate-700 bg-slate-800 p-6 text-center">
      <p className="text-slate-400">No services found.</p>
      <p className="mt-1 text-sm text-slate-500">
        Services will appear here once they start sending telemetry data.
      </p>
    </div>
  );
}

export function ServiceList() {
  const { credentials } = useCredentials();
  const workerUrl = credentials?.workerUrl ?? null;
  const { services, loading, error, refetch } = useServices(workerUrl);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-slate-100">Services</h1>
        <span className="text-sm text-slate-500">Last 1 hour</span>
      </div>

      {/* Content */}
      {loading && <LoadingSpinner />}

      {error && <ErrorMessage message={error} onRetry={refetch} />}

      {!loading && !error && services.length === 0 && <EmptyState />}

      {!loading && !error && services.length > 0 && (
        <div className="space-y-3">
          {services.map((service) => (
            <ServiceCard key={service.name} service={service} />
          ))}
        </div>
      )}
    </div>
  );
}
