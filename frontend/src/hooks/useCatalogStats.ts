import { useState, useEffect, useCallback } from 'react';
import {
  getCatalogConfig,
  listNamespaces,
  listTables,
  loadTable,
  TableIdentifier,
  LoadTableResponse,
} from '../lib/iceberg';

/**
 * Config response from worker's /v1/config endpoint.
 */
interface WorkerConfig {
  accountId: string | null;
  bucketName: string | null;
  icebergProxyEnabled: boolean;
}

/**
 * Fetch worker config from /v1/config endpoint.
 */
async function fetchWorkerConfig(workerUrl: string): Promise<WorkerConfig> {
  const response = await fetch(`${workerUrl}/v1/config`);
  if (!response.ok) {
    throw new Error(`Failed to fetch worker config: ${response.status}`);
  }
  return response.json();
}

/**
 * Stats for a single Iceberg table.
 */
export interface TableStats {
  namespace: string;
  name: string;
  fileCount: number;
  recordCount: number;
  snapshotCount: number;
  lastUpdatedMs: number | null;
}

/**
 * Aggregated stats for the entire catalog.
 */
export interface CatalogStats {
  tables: TableStats[];
  totals: {
    tableCount: number;
    fileCount: number;
    recordCount: number;
    snapshotCount: number;
  };
}

/**
 * Result returned by the useCatalogStats hook.
 */
export interface UseCatalogStatsResult {
  stats: CatalogStats | null;
  isLoading: boolean;
  error: string | null;
  refresh: () => void;
}


/**
 * Extract stats from a loaded table response.
 */
function extractTableStats(
  namespace: string,
  tableName: string,
  tableResponse: LoadTableResponse
): TableStats {
  const metadata = tableResponse.metadata;
  const snapshots = metadata.snapshots || [];
  const snapshotCount = snapshots.length;

  // Find the current snapshot
  const currentSnapshotId = metadata['current-snapshot-id'];
  const currentSnapshot = currentSnapshotId != null
    ? snapshots.find((s) => s['snapshot-id'] === currentSnapshotId)
    : undefined;

  // Extract file and record counts from current snapshot summary
  let fileCount = 0;
  let recordCount = 0;
  if (currentSnapshot?.summary) {
    const summary = currentSnapshot.summary;
    // Use total-data-files and total-records from snapshot summary
    fileCount = parseInt(summary['total-data-files'] || '0', 10) || 0;
    recordCount = parseInt(summary['total-records'] || '0', 10) || 0;
  }

  // Last updated from current snapshot timestamp or metadata
  const lastUpdatedMs = currentSnapshot?.['timestamp-ms'] ?? metadata['last-updated-ms'] ?? null;

  return {
    namespace,
    name: tableName,
    fileCount,
    recordCount,
    snapshotCount,
    lastUpdatedMs,
  };
}

/**
 * Hook to fetch and aggregate Iceberg catalog metadata.
 *
 * Fetches catalog stats including:
 * - All tables across all namespaces
 * - Snapshot counts, file counts, and record counts per table
 * - Aggregated totals
 *
 * @param workerUrl - Base URL of the worker (e.g., "https://my-worker.workers.dev")
 * @param r2Token - R2 API token for authentication with the Iceberg catalog
 * @returns Catalog stats, loading state, error message, and refresh function
 */
export function useCatalogStats(
  workerUrl: string | null,
  r2Token: string | null
): UseCatalogStatsResult {
  const [stats, setStats] = useState<CatalogStats | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    // Only fetch if both credentials are provided
    if (!workerUrl || !r2Token) {
      setStats(null);
      setError(null);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      // Step 1: Fetch worker config to get accountId and bucketName
      const workerConfig = await fetchWorkerConfig(workerUrl);

      if (!workerConfig.accountId || !workerConfig.bucketName) {
        throw new Error('Worker is not configured with R2 catalog settings');
      }

      if (!workerConfig.icebergProxyEnabled) {
        throw new Error('Iceberg proxy is not enabled on this worker');
      }

      // Step 2: Fetch Iceberg catalog config to get the warehouse prefix (UUID)
      const warehouseParam = `${workerConfig.accountId}_${workerConfig.bucketName}`;
      const catalogConfig = await getCatalogConfig(workerUrl, warehouseParam, r2Token);

      const warehouse = catalogConfig.overrides?.prefix;
      if (!warehouse) {
        throw new Error('Catalog config does not contain a warehouse prefix');
      }

      // Step 3: List all namespaces
      const namespaces = await listNamespaces(workerUrl, warehouse, r2Token);

      // Step 4: List all tables in each namespace (in parallel)
      const tablesByNamespace = await Promise.all(
        namespaces.map(async (ns) => {
          const nsString = ns.join('.');
          try {
            const tables = await listTables(workerUrl, warehouse, r2Token, nsString);
            return { namespace: nsString, tables };
          } catch (err) {
            console.warn(`Failed to list tables in namespace ${nsString}:`, err);
            return { namespace: nsString, tables: [] as TableIdentifier[] };
          }
        })
      );

      // Flatten table identifiers with their namespace strings
      const allTables: Array<{ namespace: string; identifier: TableIdentifier }> = [];
      for (const { namespace, tables } of tablesByNamespace) {
        for (const identifier of tables) {
          allTables.push({ namespace, identifier });
        }
      }

      // Step 5: Load metadata for each table (in parallel)
      const tableStatsResults = await Promise.allSettled(
        allTables.map(async ({ namespace, identifier }) => {
          const tableResponse = await loadTable(
            workerUrl,
            warehouse,
            r2Token,
            namespace,
            identifier.name
          );
          return extractTableStats(namespace, identifier.name, tableResponse);
        })
      );

      // Collect successful results and log failures
      const tableStats: TableStats[] = [];
      let failedCount = 0;

      for (let i = 0; i < tableStatsResults.length; i++) {
        const result = tableStatsResults[i];
        if (result.status === 'fulfilled') {
          tableStats.push(result.value);
        } else {
          failedCount++;
          const table = allTables[i];
          console.warn(
            `Failed to load metadata for ${table.namespace}.${table.identifier.name}:`,
            result.reason
          );
        }
      }

      // Step 6: Calculate totals
      const totals = {
        tableCount: tableStats.length,
        fileCount: tableStats.reduce((sum, t) => sum + t.fileCount, 0),
        recordCount: tableStats.reduce((sum, t) => sum + t.recordCount, 0),
        snapshotCount: tableStats.reduce((sum, t) => sum + t.snapshotCount, 0),
      };

      setStats({ tables: tableStats, totals });

      // Report partial failures
      if (failedCount > 0) {
        setError(`Loaded ${tableStats.length} tables. Failed to load metadata for ${failedCount} table(s).`);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch catalog stats';
      console.error('Catalog stats fetch error:', err);
      setError(message);
      setStats(null);
    } finally {
      setIsLoading(false);
    }
  }, [workerUrl, r2Token]);

  // Auto-fetch on mount and when credentials change
  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return {
    stats,
    isLoading,
    error,
    refresh: fetchData,
  };
}
