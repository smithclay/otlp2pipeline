import { useCredentials, type Credentials } from '../hooks/useCredentials';
import { useCatalogStats } from '../hooks/useCatalogStats';
import { ConnectionForm } from '../components/ConnectionForm';
import { CatalogOverview } from '../components/CatalogOverview';

/**
 * Settings page that combines connection configuration with catalog overview.
 *
 * Layout:
 * - Connection Settings section (always visible)
 * - Catalog Overview section (only visible when configured)
 */
export function Settings() {
  const { credentials, isConfigured, setCredentials } = useCredentials();
  const { stats, isLoading, error, refresh } = useCatalogStats(
    credentials?.workerUrl ?? null,
    credentials?.r2Token ?? null
  );

  // Handle save - update credentials (stats will auto-refresh via useEffect when credentials change)
  const handleSave = (newCreds: Credentials) => {
    setCredentials(newCreds);
  };

  return (
    <div className="space-y-10">
      {/* Connection Settings Section */}
      <section>
        <h2
          className="headline text-xl mb-6"
          style={{ color: 'var(--color-text-primary)' }}
        >
          Connection Settings
        </h2>
        <div
          className="rounded-lg p-6"
          style={{
            backgroundColor: 'white',
            border: '1px solid var(--color-border)',
            boxShadow: 'var(--shadow-sm)',
          }}
        >
          <ConnectionForm
            onSave={handleSave}
            initialValues={credentials ?? undefined}
            submitLabel={isConfigured ? 'Save' : 'Connect'}
          />
        </div>
      </section>

      {/* Catalog Overview Section - only show if configured */}
      {isConfigured && (
        <section>
          <CatalogOverview
            stats={stats}
            isLoading={isLoading}
            error={error}
            onRefresh={refresh}
          />
        </section>
      )}
    </div>
  );
}
