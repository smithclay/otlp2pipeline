import { useState, useCallback, useRef, useEffect } from 'react';
import { useCredentials } from '../hooks/useCredentials';
import { useDuckDB, type QueryResult } from '../hooks/useDuckDB';
import { getPerspectiveWorker } from '../lib/perspective';
import { usePerspectiveConfig, type ViewConfig } from '../hooks/usePerspectiveConfig';
import type { Table } from '@finos/perspective';
import type { HTMLPerspectiveViewerElement } from '@finos/perspective-viewer';

import '@finos/perspective-viewer';
import '@finos/perspective-viewer-datagrid';
import '@finos/perspective-viewer/dist/css/themes.css';

const DEFAULT_QUERY = `SELECT *
FROM r2_catalog.default.logs
LIMIT 100`;

/**
 * Perspective-compatible value types.
 */
type PerspectiveValue = string | number | boolean | Date;

/**
 * Convert QueryResult to column-oriented format for Perspective.
 * Handles BigInt conversion to Number for Perspective compatibility.
 */
function toColumnarData(result: QueryResult): Record<string, PerspectiveValue[]> {
  const columnar: Record<string, PerspectiveValue[]> = {};

  for (const col of result.columns) {
    columnar[col] = [];
  }

  for (const row of result.rows) {
    for (const col of result.columns) {
      let value = row[col];
      // Convert BigInt to Number for Perspective compatibility
      if (typeof value === 'bigint') {
        value = Number(value);
      }
      // Convert null/undefined to empty string for Perspective compatibility
      if (value === undefined || value === null) {
        value = '';
      }
      columnar[col].push(value as PerspectiveValue);
    }
  }

  return columnar;
}

export function RecordsExplorer() {
  const { credentials, isConfigured } = useCredentials();
  const { executeQuery, loading: duckdbLoading, error: duckdbError, isConnected } = useDuckDB(
    credentials?.bucketName ?? null,
    credentials?.r2Token ?? null,
    credentials?.accountId ?? null,
    credentials?.workerUrl ?? null
  );

  // Query state
  const [sql, setSql] = useState(DEFAULT_QUERY);
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [queryTimeMs, setQueryTimeMs] = useState<number | null>(null);

  // Perspective refs
  const viewerRef = useRef<HTMLPerspectiveViewerElement | null>(null);
  const tableRef = useRef<Table | null>(null);

  // Run the query
  const runQuery = useCallback(async () => {
    if (!isConnected || queryLoading) return;

    setQueryLoading(true);
    setQueryError(null);
    setQueryTimeMs(null);

    const startTime = performance.now();

    try {
      const result = await executeQuery(sql);
      const endTime = performance.now();
      setQueryTimeMs(Math.round(endTime - startTime));
      setQueryResult(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Query execution failed';
      setQueryError(message);
      setQueryResult(null);
    } finally {
      setQueryLoading(false);
    }
  }, [isConnected, queryLoading, executeQuery, sql]);

  // Handle keyboard shortcut (Cmd/Ctrl+Enter)
  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
        event.preventDefault();
        runQuery();
      }
    },
    [runQuery]
  );

  // Config persistence for the explorer
  const { save, load } = usePerspectiveConfig('records-explorer');

  // Update Perspective when query result changes
  useEffect(() => {
    if (!queryResult || queryResult.rows.length === 0) {
      return;
    }

    let mounted = true;
    const viewer = viewerRef.current;

    async function updatePerspective() {
      if (!queryResult || !viewer) return;

      try {
        // Wait for the custom element to be defined
        await customElements.whenDefined('perspective-viewer');

        const worker = await getPerspectiveWorker();
        const columnarData = toColumnarData(queryResult);

        // Create new table with the data
        const newTable = await worker.table(columnarData);

        if (!mounted) {
          await newTable.delete();
          return;
        }

        // Store reference
        tableRef.current = newTable;

        // Load into viewer
        await viewer.load(newTable);

        // Apply saved config or default
        const savedConfig = load();
        const defaultConfig: ViewConfig = {
          plugin: 'Datagrid',
          settings: true,
          theme: 'Pro Dark',
        };
        await viewer.restore((savedConfig ?? defaultConfig) as unknown as Parameters<typeof viewer.restore>[0]);
      } catch (err) {
        console.error('Failed to update Perspective:', err);
      }
    }

    // Save config when it changes
    const handleConfigChange = async () => {
      if (viewer) {
        try {
          const config = await viewer.save();
          save(config as ViewConfig);
        } catch (err) {
          console.warn('Failed to save Perspective config:', err);
        }
      }
    };

    updatePerspective().then(() => {
      if (viewer) {
        viewer.addEventListener('perspective-config-update', handleConfigChange);
      }
    });

    return () => {
      mounted = false;
      if (viewer) {
        viewer.removeEventListener('perspective-config-update', handleConfigChange);
      }
      // Note: We don't delete tables here - viewer manages the lifecycle
      tableRef.current = null;
    };
  }, [queryResult, save, load]);

  // Cleanup table on unmount
  useEffect(() => {
    return () => {
      if (tableRef.current) {
        tableRef.current.delete().catch(console.error);
        tableRef.current = null;
      }
    };
  }, []);

  const canRun = isConnected && !queryLoading && !duckdbLoading;

  return (
    <div className="flex flex-col h-full space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-slate-100">Records Explorer</h1>
        {queryTimeMs !== null && (
          <span className="text-sm text-slate-400">
            Query executed in {queryTimeMs}ms
            {queryResult && ` - ${queryResult.rows.length} rows`}
          </span>
        )}
      </div>

      {/* SQL Input */}
      <div className="space-y-2">
        <textarea
          value={sql}
          onChange={(e) => setSql(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Enter SQL query..."
          className="w-full h-32 px-3 py-2 font-mono text-sm bg-slate-800 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500 resize-y"
          spellCheck={false}
        />
        <div className="flex items-center justify-between">
          <span className="text-xs text-slate-500">
            Press Cmd+Enter to run query
          </span>
          <button
            type="button"
            onClick={runQuery}
            disabled={!canRun}
            className="px-4 py-2 text-sm font-medium bg-cyan-600 text-white rounded-lg hover:bg-cyan-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {queryLoading ? 'Running...' : 'Run Query'}
          </button>
        </div>
      </div>

      {/* Error Display */}
      {queryError && (
        <div className="rounded-lg border border-red-900 bg-red-950 p-4">
          <p className="text-red-400 font-mono text-sm">{queryError}</p>
        </div>
      )}

      {/* DuckDB Error */}
      {duckdbError && (
        <div className="rounded-lg border border-red-900 bg-red-950 p-4">
          <p className="text-red-400">{duckdbError}</p>
        </div>
      )}

      {/* Connection Status */}
      {!isConfigured && (
        <div className="rounded-lg border border-slate-700 bg-slate-800 p-4 text-center">
          <p className="text-slate-400">Configure credentials in Settings to connect to DuckDB.</p>
        </div>
      )}

      {isConfigured && duckdbLoading && (
        <div className="flex items-center justify-center py-8">
          <div className="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-cyan-500" />
          <span className="ml-3 text-slate-400">Connecting to DuckDB...</span>
        </div>
      )}

      {isConfigured && !duckdbLoading && !isConnected && !duckdbError && (
        <div className="rounded-lg border border-yellow-900 bg-yellow-950 p-4 text-center">
          <p className="text-yellow-400">DuckDB is not connected. Check your configuration.</p>
        </div>
      )}

      {/* No Results Message */}
      {isConnected && queryResult && queryResult.rows.length === 0 && (
        <div className="rounded-lg border border-slate-700 bg-slate-800 p-8 text-center">
          <p className="text-slate-400">No results found. Try adjusting your query.</p>
        </div>
      )}

      {/* Perspective Viewer - always render when connected so ref is stable */}
      {isConnected && (
        <div
          className="flex-1 min-h-[400px] rounded-lg overflow-hidden border border-slate-700"
          style={{ display: queryResult && queryResult.rows.length > 0 ? 'flex' : 'none' }}
        >
          <perspective-viewer
            ref={viewerRef}
            style={{ flex: 1, minHeight: '400px' }}
          />
        </div>
      )}
    </div>
  );
}
