import { useCredentials } from '../hooks/useCredentials';
import { useServices } from '../hooks/useServices';
import { ServiceCard } from '../components/ServiceCard';

function LoadingSpinner() {
  return (
    <div className="flex items-center justify-center py-12">
      <div className="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-cyan-500" />
    </div>
  );
}

interface ErrorMessageProps {
  message: string;
  onRetry: () => void;
}

function ErrorMessage({ message, onRetry }: ErrorMessageProps) {
  return (
    <div className="rounded-lg border border-red-900 bg-red-950 p-4">
      <p className="text-red-400">{message}</p>
      <button
        type="button"
        onClick={onRetry}
        className="mt-3 rounded-md bg-red-900 px-3 py-1.5 text-sm text-red-200 hover:bg-red-800 transition-colors"
      >
        Retry
      </button>
    </div>
  );
}

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
