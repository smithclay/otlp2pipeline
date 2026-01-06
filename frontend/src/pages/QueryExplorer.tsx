import { useState, useCallback, useRef, useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { useCredentials } from '../hooks/useCredentials';
import { useDuckDB, type QueryResult } from '../hooks/useDuckDB';
import { getPerspectiveWorker } from '../lib/perspective';
import {
  createPreset,
  detectSignalFromSchema,
} from '../lib/perspectivePresets';
import type { Signal } from '../lib/parseCommand';
import { useLiveTail } from '../hooks/useLiveTail';
import type { TailRecord } from '../hooks/useLiveTail';
import type { Table } from '@finos/perspective';
import type { HTMLPerspectiveViewerElement } from '@finos/perspective-viewer';
import { SpanDetailsPanel } from '../components/SpanDetailsPanel';
import { LogDetailPanel, type LogRecord } from '../components/LogDetailPanel';
import type { LayoutSpan } from '../lib/perspective-waterfall';
import { TabBar, type TabId } from '../components/TabBar';
import { TailInput, type TailSignal } from '../components/TailInput';
import { QueryInput } from '../components/QueryInput';
import { useServices } from '../hooks/useServices';
import { OverviewBar, type OverviewData } from '../components/OverviewBar';
import { ViewToggle, type ViewType } from '../components/ViewToggle';

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
 * Columns that contain IDs which should be converted to strings
 * to preserve precision for values larger than Number.MAX_SAFE_INTEGER.
 */
const ID_COLUMNS = new Set([
  'trace_id',
  'span_id',
  'parent_span_id',
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
      // Handle BigInt values
      if (typeof value === 'bigint') {
        // ID columns should be converted to strings to preserve precision
        if (ID_COLUMNS.has(col)) {
          value = value.toString();
        } else {
          // Warn if precision will be lost for non-ID columns
          if (value > Number.MAX_SAFE_INTEGER || value < Number.MIN_SAFE_INTEGER) {
            console.warn(
              `Precision loss: column "${col}" has BigInt value ${value} outside safe integer range`
            );
          }
          value = Number(value);
        }
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

/**
 * Compute overview data based on the current tab and data.
 */
function computeOverviewData(
  activeTab: TabId,
  queryResult: QueryResult | null,
  tailRecords: TailRecord[],
  tailStatus: { state: string },
  queryTimeMs: number | null,
  droppedCount: number
): OverviewData | null {
  if (activeTab === 'tail' && tailStatus.state !== 'idle') {
    // Estimate rate from record count (simplified)
    const rate = tailRecords.length > 0 ? Math.round(tailRecords.length / 10) : 0;
    return {
      type: 'tail',
      recordCount: tailRecords.length,
      rate,
      droppedCount,
    };
  }

  if (activeTab === 'query' && queryResult && queryResult.rows.length > 0) {
    const rows = queryResult.rows;
    const columns = Object.keys(rows[0]);

    // Detect if this is logs data
    if (columns.includes('severity') && columns.includes('message')) {
      const errorCount = rows.filter(r => {
        const sev = String(r.severity).toUpperCase();
        return sev === 'ERROR' || sev === 'FATAL' || sev === 'CRITICAL';
      }).length;
      return {
        type: 'logs',
        recordCount: rows.length,
        errorCount,
      };
    }

    // Detect if this is traces data
    if (columns.includes('trace_id') && columns.includes('span_id')) {
      // Check if single trace
      const traceIds = new Set(rows.map(r => r.trace_id));
      if (traceIds.size === 1) {
        const errorCount = rows.filter(r => r.status_code === 2).length;
        const durations = rows.map(r => Number(r.duration) || 0);
        const totalMs = Math.max(...durations) / 1000; // Assuming microseconds
        return {
          type: 'single-trace',
          traceId: String(rows[0].trace_id),
          spanCount: rows.length,
          totalMs,
          errorCount,
        };
      }

      // Multiple traces
      const errorCount = rows.filter(r => r.status_code === 2).length;
      const durations = rows.map(r => Number(r.duration) || 0).sort((a, b) => a - b);
      const p50 = durations[Math.floor(durations.length * 0.5)] / 1000;
      const p99 = durations[Math.floor(durations.length * 0.99)] / 1000;
      return {
        type: 'traces',
        traceCount: rows.length,
        errorCount,
        p50Ms: p50,
        p99Ms: p99,
      };
    }

    // Generic SQL result
    return {
      type: 'sql',
      rowCount: rows.length,
      columnCount: columns.length,
      queryTimeMs: queryTimeMs ?? 0,
    };
  }

  return null;
}

interface LocationState {
  initialQuery?: string;
  initialTab?: TabId;
  initialService?: string;
  initialSignal?: TailSignal;
}

export function QueryExplorer() {
  const location = useLocation();
  const { credentials, isConfigured } = useCredentials();
  const { executeQuery, loading: duckdbLoading, error: duckdbError, isConnected } = useDuckDB(
    credentials?.workerUrl ?? null,
    credentials?.r2Token ?? null
  );
  const { services } = useServices(credentials?.workerUrl ?? null);
  const serviceNames = services.map(s => s.name);

  // Get initial query from navigation state
  const locationState = location.state as LocationState | null;
  const initialQuery = locationState?.initialQuery ?? DEFAULT_QUERY;

  // Query state
  const [sql, setSql] = useState(initialQuery);
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [queryTimeMs, setQueryTimeMs] = useState<number | null>(null);

  // Live tail state
  const [tailConfig, setTailConfig] = useState<{ service: string; signal: Signal; limit: number } | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>(locationState?.initialTab ?? 'query');

  // Waterfall state
  const [selectedSpan, setSelectedSpan] = useState<LayoutSpan | null>(null);
  const [viewMode, setViewMode] = useState<ViewType>('table');

  // Log detail state
  const [selectedLog, setSelectedLog] = useState<LogRecord | null>(null);

  // Tail form state
  const [tailService, setTailService] = useState(locationState?.initialService ?? '');
  const [tailSignal, setTailSignal] = useState<TailSignal>(locationState?.initialSignal ?? 'logs');

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
  // Track tables pending deletion to prevent leaks during unmount
  const pendingDeleteRef = useRef<Table | null>(null);

  // Track previous tail config to detect changes
  const prevTailConfigRef = useRef<typeof tailConfig>(null);

  // Handle SQL query execution
  const handleRun = useCallback(async () => {
    if (!isConnected || queryLoading) return;

    setQueryLoading(true);
    setQueryError(null);
    setQueryTimeMs(null);

    const startTime = performance.now();

    try {
      const queryResult = await executeQuery(sql);
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
  }, [sql, isConnected, queryLoading, executeQuery]);

  // Handle tail start/stop from TailInput component
  const handleTailStartStop = useCallback(() => {
    if (tailStatus.state !== 'idle') {
      stopTail();
      return;
    }

    if (!tailService) return;

    setTailConfig({
      service: tailService,
      signal: tailSignal,
      limit: 500,
    });
  }, [tailService, tailSignal, tailStatus.state, stopTail]);

  // Compute whether waterfall is available
  const canShowWaterfall = queryResult && isSingleTrace(queryResult.rows);

  // Handle view toggle between table and waterfall
  const handleViewChange = useCallback(async (view: ViewType) => {
    setViewMode(view);
    const viewer = viewerRef.current;
    if (!viewer) return;

    try {
      if (view === 'waterfall') {
        await viewer.restore({ plugin: 'perspective-waterfall' } as any);
      } else {
        await viewer.restore({ plugin: 'perspective-viewer-datagrid' } as any);
      }
    } catch (err) {
      console.debug('Could not switch view:', err);
    }
  }, []);

  // Start tail when config changes or when entering tail mode
  useEffect(() => {
    if (activeTab !== 'tail' || !tailConfig) {
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
  }, [activeTab, tailConfig, tailStatus.state, startTail, stopTail]);

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

        // Create preset for query mode (no persistence - schema varies per query)
        const preset = createPreset(
          { signal: detectedSignal, mode: 'query' },
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

        // Apply preset config
        await viewer.restore(preset as unknown as Parameters<typeof viewer.restore>[0]);

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

    updatePerspective();

    return () => {
      mounted = false;
      // Note: We don't delete tables here - viewer manages the lifecycle
      tableRef.current = null;
    };
  }, [queryResult]);

  // Update Perspective viewer when tail records change (debounced)
  useEffect(() => {
    if (activeTab !== 'tail' || tailRecords.length === 0 || !tailConfig) {
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
        // Track oldTable in ref to handle cleanup if component unmounts mid-transition
        const oldTable = tableRef.current;
        pendingDeleteRef.current = oldTable;

        const newTable = await worker.table(columnar);
        tableRef.current = newTable;
        await viewer.load(newTable);

        // Delete old table and clear pending ref
        if (oldTable) {
          await oldTable.delete();
        }
        pendingDeleteRef.current = null;

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
  }, [activeTab, tailRecords, tailConfig]);

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

  // Listen for row clicks on the perspective viewer (for log details)
  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer) return;

    // Helper to safely parse JSON that may be double-encoded or empty
    const safeParseJson = (value: unknown): Record<string, unknown> | undefined => {
      if (!value || value === '') return undefined;
      if (typeof value === 'object') return value as Record<string, unknown>;
      if (typeof value !== 'string') return undefined;
      try {
        let parsed = JSON.parse(value);
        // Handle double-encoded JSON (e.g., '"{\"key\":\"value\"}"')
        if (typeof parsed === 'string') {
          parsed = JSON.parse(parsed);
        }
        return typeof parsed === 'object' ? parsed : undefined;
      } catch {
        return undefined;
      }
    };

    const handleRowClick = (e: Event) => {
      const customEvent = e as CustomEvent;
      const row = customEvent.detail?.row;
      if (!row) return;

      // Check if this looks like a log record (use actual column names from schema)
      if ('severity_text' in row && 'body' in row) {
        setSelectedLog({
          timestamp: row.timestamp ? new Date(row.timestamp).toISOString() : new Date().toISOString(),
          severity: String(row.severity_text || 'INFO'),
          message: String(row.body || ''),
          service: String(row.service_name || 'unknown'),
          trace_id: row.trace_id ? String(row.trace_id) : undefined,
          span_id: row.span_id ? String(row.span_id) : undefined,
          host: row.host ? String(row.host) : undefined,
          attributes: safeParseJson(row.log_attributes),
          resource_attributes: safeParseJson(row.resource_attributes),
        });
      }
    };

    viewer.addEventListener('perspective-click', handleRowClick);
    return () => viewer.removeEventListener('perspective-click', handleRowClick);
  }, []);

  // Cleanup tables on unmount
  useEffect(() => {
    return () => {
      // Delete current table
      if (tableRef.current) {
        tableRef.current.delete().catch(console.error);
        tableRef.current = null;
      }
      // Delete any table pending deletion (handles race during unmount)
      if (pendingDeleteRef.current) {
        pendingDeleteRef.current.delete().catch(console.error);
        pendingDeleteRef.current = null;
      }
    };
  }, []);

  return (
    <div className="flex flex-col h-full space-y-6">
      {/* Header */}
      <div>
        <p className="mt-1 text-sm" style={{ color: 'var(--color-text-muted)' }}>
          Explore your telemetry data with SQL or stream live with tail
        </p>
      </div>

      {/* Tab Bar */}
      <TabBar
        activeTab={activeTab}
        onTabChange={(tab) => {
          // Stop tail if switching away from tail mode
          if (activeTab === 'tail' && tab === 'query' && tailStatus.state !== 'idle') {
            stopTail();
          }
          setActiveTab(tab);
        }}
      />

      {/* Input Area - varies by tab */}
      {activeTab === 'query' ? (
        <QueryInput
          value={sql}
          onChange={setSql}
          onRun={handleRun}
          state={queryLoading ? 'running' : 'idle'}
          canRun={isConnected && !queryLoading}
          queryTimeMs={queryTimeMs}
          rowCount={queryResult?.rows.length ?? null}
        />
      ) : (
        <TailInput
          service={tailService}
          signal={tailSignal}
          isStreaming={tailStatus.state !== 'idle'}
          services={serviceNames}
          onServiceChange={setTailService}
          onSignalChange={setTailSignal}
          onStartStop={handleTailStartStop}
          recordCount={tailRecords.length}
          droppedCount={droppedCount}
        />
      )}

      {/* Overview Bar */}
      {(() => {
        const data = computeOverviewData(
          activeTab,
          queryResult,
          tailRecords,
          tailStatus,
          queryTimeMs,
          droppedCount
        );
        return data ? <OverviewBar data={data} /> : null;
      })()}

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
              (activeTab === 'query' && queryResult && queryResult.rows.length > 0) ||
              (activeTab === 'tail' && tailRecords.length > 0)
                ? 'flex'
                : 'none',
            border: '1px solid var(--color-border)',
            boxShadow: 'var(--shadow-sm)',
          }}
        >
          <div className="relative flex-1" style={{ minHeight: '400px' }}>
            {/* View Toggle - top right */}
            {canShowWaterfall && (
              <div className="absolute top-3 right-3 z-10">
                <ViewToggle view={viewMode} onViewChange={handleViewChange} />
              </div>
            )}

            <perspective-viewer
              ref={viewerRef}
              style={{ width: '100%', height: '100%' }}
            />
            <SpanDetailsPanel
              span={selectedSpan}
              onClose={() => setSelectedSpan(null)}
            />
            <LogDetailPanel
              log={selectedLog}
              onClose={() => setSelectedLog(null)}
              onTraceClick={(traceId) => {
                // Navigate to query with trace filter
                setSql(`SELECT * FROM r2_catalog.default.traces WHERE trace_id = '${traceId}'`);
                setActiveTab('query');
                setSelectedLog(null);
                handleRun();
              }}
            />
          </div>
        </div>
      )}

      {/* No Results Message - only for query mode */}
      {activeTab === 'query' && isConnected && queryResult && queryResult.rows.length === 0 && (
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
      {activeTab === 'tail' && tailStatus.state === 'connected' && tailRecords.length === 0 && (
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
