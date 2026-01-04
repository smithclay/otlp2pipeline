import { useState, useCallback, useRef, useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { useCredentials } from '../hooks/useCredentials';
import { useDuckDB, type QueryResult } from '../hooks/useDuckDB';
import { getPerspectiveWorker } from '../lib/perspective';
import { usePerspectiveConfig, type ViewConfig } from '../hooks/usePerspectiveConfig';
import {
  createPreset,
  mergeWithUserConfig,
  detectSignalFromSchema,
  type PerspectivePreset,
} from '../lib/perspectivePresets';
import { parseCommand, isTailCommand, type Signal } from '../lib/parseCommand';
import { useLiveTail } from '../hooks/useLiveTail';
import type { TailStatus, TailRecord } from '../hooks/useLiveTail';
import type { Table } from '@finos/perspective';
import type { HTMLPerspectiveViewerElement } from '@finos/perspective-viewer';
import { SpanDetailsPanel } from '../components/SpanDetailsPanel';
import type { LayoutSpan } from '../lib/perspective-waterfall';

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
 * Column names that should be converted to Date objects for Perspective.
 * Perspective requires Date objects (not numbers) to format as datetime.
 */
const TIMESTAMP_COLUMNS = new Set([
  'timestamp',
  '__ingest_ts',
  'observed_timestamp',
  'end_timestamp',
  'start_timestamp',
]);

/**
 * Convert generic records to column-oriented format for Perspective.
 * Handles BigInt conversion to Number for Perspective compatibility.
 * Converts timestamp columns to Date objects for proper datetime formatting.
 */
function toColumnarData(records: Record<string, unknown>[]): Record<string, PerspectiveValue[]> | null {
  if (!records || records.length === 0) return null;

  const columns = Object.keys(records[0]);
  const columnar: Record<string, PerspectiveValue[]> = {};

  for (const col of columns) {
    columnar[col] = [];
  }

  for (const record of records) {
    for (const col of columns) {
      let value = record[col];
      // Convert BigInt to Number for Perspective compatibility
      if (typeof value === 'bigint') {
        value = Number(value);
      }
      // Convert null/undefined to empty string for Perspective compatibility
      if (value === undefined || value === null) {
        value = '';
      }
      // Convert timestamp columns to Date objects for Perspective datetime formatting
      if (TIMESTAMP_COLUMNS.has(col) && typeof value === 'number') {
        value = new Date(value);
      }
      columnar[col].push(value as PerspectiveValue);
    }
  }

  return columnar;
}

/**
 * Check if query result represents a single trace (all rows have same trace_id)
 * and has the required columns for waterfall visualization.
 */
function isSingleTrace(rows: Record<string, unknown>[]): boolean {
  if (rows.length === 0) return false;

  // Check for required columns
  const firstRow = rows[0];
  if (!('trace_id' in firstRow) || !('span_id' in firstRow)) {
    return false;
  }

  // Check if all rows have same trace_id
  const traceIds = new Set(rows.map(r => r.trace_id));
  return traceIds.size === 1;
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

  // Waterfall state
  const [selectedSpan, setSelectedSpan] = useState<LayoutSpan | null>(null);

  // Detect if current input looks like a TAIL command (for UI hints)
  const inputLooksTail = isTailCommand(sql);

  // Live tail hook - only active when we have a tail config
  const {
    start: startTail,
    stop: stopTail,
    status: tailStatus,
    records: tailRecords,
    droppedCount,
  } = useLiveTail(
    credentials?.workerUrl ?? null,
    tailConfig?.service ?? '',
    tailConfig?.signal ?? 'logs',
    tailConfig?.limit ?? 500
  );

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

  // Debounce refs for tail updates
  const tailUpdateTimeoutRef = useRef<number | null>(null);
  const pendingTailRecordsRef = useRef<TailRecord[]>([]);

  // Track previous tail config to detect changes
  const prevTailConfigRef = useRef<typeof tailConfig>(null);

  // Unified run handler for both query and tail modes
  const handleRun = useCallback(async () => {
    setParseError(null);

    // If currently tailing, stop
    if (mode === 'tail' && tailStatus.state !== 'idle') {
      stopTail();
      setMode('query');
      return;
    }

    // Parse the input
    const result = parseCommand(sql);

    if (result.type === 'error') {
      setParseError(result.message);
      return;
    }

    if (result.type === 'query') {
      // SQL query mode
      if (!isConnected || queryLoading) return;

      setMode('query');
      setTailConfig(null);
      setQueryLoading(true);
      setQueryError(null);
      setQueryTimeMs(null);

      const startTime = performance.now();

      try {
        const queryResult = await executeQuery(result.sql);
        const endTime = performance.now();
        setQueryTimeMs(Math.round(endTime - startTime));
        setQueryResult(queryResult);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Query execution failed';
        setQueryError(message);
        setQueryResult(null);
      } finally {
        setQueryLoading(false);
      }
    } else {
      // Tail mode
      setMode('tail');
      setTailConfig({ service: result.service, signal: result.signal, limit: result.limit });
      setQueryResult(null);
      setQueryError(null);
      setQueryTimeMs(null);

      // Start will be triggered by effect when tailConfig changes
    }
  }, [sql, mode, tailStatus.state, stopTail, isConnected, queryLoading, executeQuery]);

  // Handle keyboard shortcut (Cmd/Ctrl+Enter)
  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
        event.preventDefault();
        handleRun();
      }
    },
    [handleRun]
  );

  // Start tail when config changes or when entering tail mode
  useEffect(() => {
    if (mode !== 'tail' || !tailConfig) {
      prevTailConfigRef.current = null;
      return;
    }

    const configChanged = prevTailConfigRef.current !== null &&
      (prevTailConfigRef.current.service !== tailConfig.service ||
       prevTailConfigRef.current.signal !== tailConfig.signal ||
       prevTailConfigRef.current.limit !== tailConfig.limit);

    if (tailStatus.state === 'idle' || configChanged) {
      // Stop existing connection if config changed while connected
      if (configChanged && tailStatus.state !== 'idle') {
        stopTail();
      }
      prevTailConfigRef.current = tailConfig;
      startTail();
    }
  }, [mode, tailConfig, tailStatus.state, startTail, stopTail]);

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
        const columnarData = toColumnarData(queryResult.rows);
        if (!columnarData) return;

        // Get schema columns from the data
        const schemaColumns = Object.keys(columnarData);

        // Detect signal type from schema (logs vs traces)
        const detectedSignal = detectSignalFromSchema(schemaColumns);

        // Create preset for query mode
        const preset = createPreset(
          { signal: detectedSignal, mode: 'query' },
          schemaColumns
        );

        // Merge with user's saved config (user preferences take precedence)
        // Pass schemaColumns to filter out invalid columns from saved config
        const savedConfig = load();
        const finalConfig = mergeWithUserConfig(
          preset,
          savedConfig as Partial<PerspectivePreset> | null,
          schemaColumns
        );

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

        // Apply merged config
        await viewer.restore(finalConfig as unknown as Parameters<typeof viewer.restore>[0]);

        // Auto-switch to waterfall if single trace detected
        if (isSingleTrace(queryResult.rows)) {
          // Small delay to let viewer initialize
          setTimeout(async () => {
            try {
              // Check current plugin by saving config
              const config = await viewer.save();
              if ((config as { plugin?: string }).plugin !== 'perspective-waterfall') {
                await viewer.restore({ plugin: 'perspective-waterfall' } as any);
              }
            } catch (err) {
              // Waterfall plugin might not be available, that's ok
              console.debug('Could not switch to waterfall:', err);
            }
          }, 100);
        }
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

  // Update Perspective viewer when tail records change (debounced)
  useEffect(() => {
    if (mode !== 'tail' || tailRecords.length === 0 || !tailConfig) {
      return;
    }

    // Store pending records
    pendingTailRecordsRef.current = tailRecords;

    // Debounce updates to every 250ms
    if (tailUpdateTimeoutRef.current !== null) {
      return; // Update already scheduled
    }

    // Capture tailConfig.signal for use in async callback
    const signal = tailConfig.signal;

    tailUpdateTimeoutRef.current = window.setTimeout(async () => {
      tailUpdateTimeoutRef.current = null;
      const records = pendingTailRecordsRef.current;
      const viewer = viewerRef.current;

      if (!viewer || records.length === 0) return;

      try {
        await customElements.whenDefined('perspective-viewer');
        const worker = await getPerspectiveWorker();

        // Convert records to columnar format using shared function
        const columnar = toColumnarData(records);
        if (!columnar) return;

        // Get schema columns from the data
        const schemaColumns = Object.keys(columnar);

        // Create preset for tail mode (ephemeral - not merged with user config)
        // Uses strict sorting and prioritized column order
        const preset = createPreset(
          { signal, mode: 'tail' },
          schemaColumns
        );

        // Create new table first, then load into viewer, then delete old table
        // (Must load before delete - viewer holds a View on the old table)
        const oldTable = tableRef.current;
        const newTable = await worker.table(columnar);
        tableRef.current = newTable;
        await viewer.load(newTable);
        if (oldTable) {
          await oldTable.delete();
        }

        // Apply tail preset (no user config merge - tail mode is ephemeral)
        await viewer.restore(preset as unknown as Parameters<typeof viewer.restore>[0]);
      } catch (err) {
        console.error('Failed to update Perspective for tail:', err);
      }
    }, 250);

    return () => {
      if (tailUpdateTimeoutRef.current !== null) {
        clearTimeout(tailUpdateTimeoutRef.current);
        tailUpdateTimeoutRef.current = null;
      }
      // Note: Don't delete table here - handled by unmount effect below
    };
  }, [mode, tailRecords, tailConfig]);

  // Listen for waterfall span selection events
  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer) return;

    const handleSpanSelect = (e: Event) => {
      const customEvent = e as CustomEvent<{ span: LayoutSpan }>;
      setSelectedSpan(customEvent.detail.span);
    };

    viewer.addEventListener('span-select', handleSpanSelect);
    return () => {
      viewer.removeEventListener('span-select', handleSpanSelect);
    };
  }, []);

  // Cleanup table on unmount
  useEffect(() => {
    return () => {
      if (tableRef.current) {
        tableRef.current.delete().catch(console.error);
        tableRef.current = null;
      }
    };
  }, []);

  const isTailing = mode === 'tail' && tailStatus.state !== 'idle' && tailStatus.state !== 'error';
  const canRun = inputLooksTail
    ? (credentials?.workerUrl && !queryLoading) // Tail mode: need worker URL
    : (isConnected && !queryLoading && !duckdbLoading); // Query mode: need DuckDB

  // Determine button text and style
  const getButtonConfig = () => {
    if (isTailing) {
      return { text: 'Stop', className: 'bg-red-500 hover:bg-red-600' };
    }
    if (duckdbLoading && !inputLooksTail) {
      return { text: 'Connecting...', className: '' };
    }
    if (queryLoading) {
      return { text: 'Running...', className: '' };
    }
    if (inputLooksTail) {
      return { text: 'Start Tail', className: '' };
    }
    return { text: 'Run Query', className: '' };
  };

  const buttonConfig = getButtonConfig();

  return (
    <div className="flex flex-col h-full space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <p className="mt-1 text-sm" style={{ color: 'var(--color-text-muted)' }}>
            Explore your telemetry data with SQL or stream live with TAIL
          </p>
        </div>
        <div className="flex items-center gap-4">
          {/* Tail status indicator */}
          {mode === 'tail' && tailStatus.state !== 'idle' && (
            <div className="flex items-center gap-2 text-sm">
              {tailStatus.state === 'connected' && (
                <>
                  <span className="relative flex h-2 w-2">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75" />
                    <span className="relative inline-flex rounded-full h-2 w-2 bg-red-500" />
                  </span>
                  <span style={{ color: 'var(--color-text-secondary)' }}>
                    Live · {tailRecords.length} records
                    {droppedCount > 0 && ` · ${droppedCount} dropped`}
                  </span>
                </>
              )}
              {tailStatus.state === 'connecting' && (
                <span style={{ color: 'var(--color-text-muted)' }}>Connecting...</span>
              )}
              {tailStatus.state === 'reconnecting' && (
                <span style={{ color: 'var(--color-warning)' }}>
                  Reconnecting ({tailStatus.attempt}/3)...
                </span>
              )}
            </div>
          )}
          {/* Query time indicator */}
          {mode === 'query' && queryTimeMs !== null && (
            <span
              className="text-sm font-medium mono"
              style={{ color: 'var(--color-text-tertiary)' }}
            >
              {queryTimeMs}ms
              {queryResult && ` · ${queryResult.rows.length} rows`}
            </span>
          )}
        </div>
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
            {inputLooksTail
              ? 'Press Cmd+Enter to start tail'
              : 'Press Cmd+Enter to run query'}
          </span>
          <button
            type="button"
            onClick={handleRun}
            disabled={!canRun && !isTailing}
            className={`relative px-4 py-2 text-sm font-medium rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors ${buttonConfig.className}`}
            style={{
              backgroundColor: isTailing ? undefined : 'var(--color-accent)',
              color: 'white',
              minWidth: '180px',
            }}
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.span
                key={buttonConfig.text}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.15 }}
                className="block"
              >
                {buttonConfig.text}
              </motion.span>
            </AnimatePresence>
          </button>
        </div>
      </div>

      {/* Parse Error Display */}
      {parseError && (
        <div
          className="rounded-lg p-4"
          style={{
            backgroundColor: 'var(--color-error-bg)',
            border: '1px solid var(--color-error)',
          }}
        >
          <p className="font-mono text-sm" style={{ color: 'var(--color-error)' }}>
            {parseError}
          </p>
        </div>
      )}

      {/* Tail Error Display */}
      {tailStatus.state === 'error' && (
        <div
          className="rounded-lg p-4"
          style={{
            backgroundColor: 'var(--color-error-bg)',
            border: '1px solid var(--color-error)',
          }}
        >
          <p className="font-mono text-sm" style={{ color: 'var(--color-error)' }}>
            {tailStatus.message}
          </p>
        </div>
      )}

      {/* Query Error Display */}
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

      {/* Perspective Viewer - show when we have data in either mode */}
      {isConfigured && (
        <div
          className="flex-1 min-h-[400px] rounded-lg overflow-hidden"
          style={{
            display:
              (mode === 'query' && queryResult && queryResult.rows.length > 0) ||
              (mode === 'tail' && tailRecords.length > 0)
                ? 'flex'
                : 'none',
            border: '1px solid var(--color-border)',
            boxShadow: 'var(--shadow-sm)',
          }}
        >
          <div className="relative flex-1" style={{ minHeight: '400px' }}>
            <perspective-viewer
              ref={viewerRef}
              style={{ width: '100%', height: '100%' }}
            />
            <SpanDetailsPanel
              span={selectedSpan}
              onClose={() => setSelectedSpan(null)}
            />
          </div>
        </div>
      )}

      {/* No Results Message - only for query mode */}
      {mode === 'query' && isConnected && queryResult && queryResult.rows.length === 0 && (
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

      {/* Waiting for data - tail mode */}
      {mode === 'tail' && tailStatus.state === 'connected' && tailRecords.length === 0 && (
        <div
          className="rounded-lg p-8 text-center"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
          }}
        >
          <p style={{ color: 'var(--color-text-secondary)' }}>
            Waiting for records...
          </p>
        </div>
      )}
    </div>
  );
}
