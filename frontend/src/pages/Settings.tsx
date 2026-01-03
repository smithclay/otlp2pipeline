import { useState } from 'react';
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
  const { credentials, isConfigured, setCredentials, clearCredentials } = useCredentials();
  const { stats, isLoading, error, refresh } = useCatalogStats(
    credentials?.workerUrl ?? null,
    credentials?.r2Token ?? null
  );
  const [showClearConfirm, setShowClearConfirm] = useState(false);

  // Handle save - update credentials (stats will auto-refresh via useEffect when credentials change)
  const handleSave = (newCreds: Credentials) => {
    setCredentials(newCreds);
  };

  // Handle clear with confirmation
  const handleClear = () => {
    clearCredentials();
    setShowClearConfirm(false);
  };

  return (
    <div className="space-y-10">
      {/* Connection Settings Section */}
      <section>
        <div className="flex items-center justify-between mb-6">
          <h2
            className="headline text-xl"
            style={{ color: 'var(--color-text-primary)' }}
          >
            Connection Settings
          </h2>
          {isConfigured && !showClearConfirm && (
            <button
              onClick={() => setShowClearConfirm(true)}
              className="text-sm px-3 py-1.5 rounded-md transition-colors hover:bg-red-50"
              style={{ color: 'var(--color-error)' }}
            >
              Clear Settings
            </button>
          )}
        </div>

        {/* Clear confirmation */}
        {showClearConfirm && (
          <div
            className="mb-4 p-4 rounded-lg flex items-center justify-between"
            style={{
              backgroundColor: 'var(--color-error-bg)',
              border: '1px solid var(--color-error)',
            }}
          >
            <p className="text-sm" style={{ color: 'var(--color-text-primary)' }}>
              Clear all connection settings? You'll need to re-enter them to use the app.
            </p>
            <div className="flex gap-2 ml-4">
              <button
                onClick={() => setShowClearConfirm(false)}
                className="text-sm px-3 py-1.5 rounded-md transition-colors"
                style={{
                  backgroundColor: 'white',
                  border: '1px solid var(--color-border)',
                  color: 'var(--color-text-secondary)',
                }}
              >
                Cancel
              </button>
              <button
                onClick={handleClear}
                className="text-sm px-3 py-1.5 rounded-md text-white transition-colors hover:opacity-90"
                style={{ backgroundColor: 'var(--color-error)' }}
              >
                Clear
              </button>
            </div>
          </div>
        )}

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
