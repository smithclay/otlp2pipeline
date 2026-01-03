import { useState, useCallback, useRef, useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { useCredentials } from '../hooks/useCredentials';
import { useDuckDB, type QueryResult } from '../hooks/useDuckDB';
import { getPerspectiveWorker } from '../lib/perspective';
import { usePerspectiveConfig, type ViewConfig } from '../hooks/usePerspectiveConfig';
import { parseCommand, isTailCommand, type Signal } from '../lib/parseCommand';
import { useLiveTail } from '../hooks/useLiveTail';
import type { TailStatus, TailRecord } from '../hooks/useLiveTail';
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

interface LocationState {
  initialQuery?: string;
}

export function RecordsExplorer() {
  const location = useLocation();
  const { credentials, isConfigured } = useCredentials();
  const { executeQuery, loading: duckdbLoading, error: duckdbError, isConnected } = useDuckDB(
    credentials?.workerUrl ?? null,
    credentials?.r2Token ?? null
  );

  // Get initial query from navigation state
  const locationState = location.state as LocationState | null;
  const initialQuery = locationState?.initialQuery ?? DEFAULT_QUERY;

  // Query state
  const [sql, setSql] = useState(initialQuery);
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [queryTimeMs, setQueryTimeMs] = useState<number | null>(null);

  // Live tail state (will be wired up in subsequent tasks)
  const [tailConfig, setTailConfig] = useState<{ service: string; signal: Signal; limit: number } | null>(null);
  const [mode, setMode] = useState<'query' | 'tail'>('query');
  const [parseError, setParseError] = useState<string | null>(null);

  // Detect if current input looks like a TAIL command (for UI hints)
  const inputLooksTail = isTailCommand(sql);

  // TODO: These will be used in subsequent tasks - suppress unused warnings for now
  void parseCommand; void useLiveTail;
  void tailConfig; void setTailConfig;
  void mode; void setMode;
  void parseError; void setParseError;
  void inputLooksTail;
  // Type placeholders - will be used in subsequent tasks
  const _tailStatusType: TailStatus | null = null; void _tailStatusType;
  const _tailRecordType: TailRecord | null = null; void _tailRecordType;

  // Update SQL when navigating with a new query
  useEffect(() => {
    if (locationState?.initialQuery) {
      setSql(locationState.initialQuery);
    }
  }, [locationState?.initialQuery]);

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

        // Apply saved config or default (no theme = uses built-in default)
        const savedConfig = load();
        const defaultConfig: ViewConfig = {
          plugin: 'Datagrid',
          settings: true,
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
    <div className="flex flex-col h-full space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <p className="mt-1 text-sm" style={{ color: 'var(--color-text-muted)' }}>
            Explore your telemetry data with SQL
          </p>
        </div>
        {queryTimeMs !== null && (
          <span
            className="text-sm font-medium mono"
            style={{ color: 'var(--color-text-tertiary)' }}
          >
            {queryTimeMs}ms
            {queryResult && ` · ${queryResult.rows.length} rows`}
          </span>
        )}
      </div>

      {/* SQL Input */}
      <div
        className="rounded-lg p-5"
        style={{
          backgroundColor: 'white',
          border: '1px solid var(--color-border)',
          boxShadow: 'var(--shadow-sm)',
        }}
      >
        <textarea
          value={sql}
          onChange={(e) => setSql(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Enter SQL query..."
          className="w-full h-32 px-3 py-2 font-mono text-sm rounded-md resize-y"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-primary)',
          }}
          spellCheck={false}
        />
        <div className="flex items-center justify-between mt-3">
          <span className="text-xs" style={{ color: 'var(--color-text-muted)' }}>
            Press Cmd+Enter to run query
          </span>
          <button
            type="button"
            onClick={runQuery}
            disabled={!canRun}
            className="px-4 py-2 text-sm font-medium rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            style={{
              backgroundColor: 'var(--color-accent)',
              color: 'white',
            }}
          >
            {queryLoading ? 'Running...' : 'Run Query'}
          </button>
        </div>
      </div>

      {/* Error Display */}
      {queryError && (
        <div
          className="rounded-lg p-4"
          style={{
            backgroundColor: 'var(--color-error-bg)',
            border: '1px solid var(--color-error)',
          }}
        >
          <p className="font-mono text-sm" style={{ color: 'var(--color-error)' }}>
            {queryError}
          </p>
        </div>
      )}

      {/* DuckDB Error */}
      {duckdbError && (
        <div
          className="rounded-lg p-4"
          style={{
            backgroundColor: 'var(--color-error-bg)',
            border: '1px solid var(--color-error)',
          }}
        >
          <p style={{ color: 'var(--color-error)' }}>{duckdbError}</p>
        </div>
      )}

      {/* Connection Status */}
      {!isConfigured && (
        <div
          className="rounded-lg p-6 text-center"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
          }}
        >
          <p style={{ color: 'var(--color-text-secondary)' }}>
            Configure credentials in Settings to connect to DuckDB.
          </p>
        </div>
      )}

      {isConfigured && duckdbLoading && (
        <div className="flex items-center justify-center py-8">
          <div
            className="h-8 w-8 animate-spin rounded-full border-2"
            style={{
              borderColor: 'var(--color-border)',
              borderTopColor: 'var(--color-accent)',
            }}
          />
          <span className="ml-3" style={{ color: 'var(--color-text-muted)' }}>
            Connecting to DuckDB...
          </span>
        </div>
      )}

      {isConfigured && !duckdbLoading && !isConnected && !duckdbError && (
        <div
          className="rounded-lg p-4 text-center"
          style={{
            backgroundColor: 'var(--color-warning-bg)',
            border: '1px solid var(--color-warning)',
          }}
        >
          <p style={{ color: 'var(--color-warning)' }}>
            DuckDB is not connected. Check your configuration.
          </p>
        </div>
      )}

      {/* No Results Message */}
      {isConnected && queryResult && queryResult.rows.length === 0 && (
        <div
          className="rounded-lg p-8 text-center"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
          }}
        >
          <p style={{ color: 'var(--color-text-secondary)' }}>
            No results found. Try adjusting your query.
          </p>
        </div>
      )}

      {/* Perspective Viewer - always render when connected so ref is stable */}
      {isConnected && (
        <div
          className="flex-1 min-h-[400px] rounded-lg overflow-hidden"
          style={{
            display: queryResult && queryResult.rows.length > 0 ? 'flex' : 'none',
            border: '1px solid var(--color-border)',
            boxShadow: 'var(--shadow-sm)',
          }}
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
