import { useState, useEffect, useCallback, useMemo } from 'react';
import { useCredentials } from '../hooks/useCredentials';
import {
  useDuckDB,
  buildRecordsQuery,
  generateMockResult,
  type QueryResult,
} from '../hooks/useDuckDB';

/**
 * Props for the RecordsPanel component.
 */
export interface RecordsPanelProps {
  /** Service name to query records for */
  service: string;
  /** Time range to query */
  timeRange: { from: Date; to: Date };
  /** Callback when panel is closed */
  onClose: () => void;
}

/**
 * Badge component for record type (LOG/SPAN).
 */
function TypeBadge({ type }: { type: string }) {
  const isLog = type === 'LOG';

  return (
    <span
      className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-medium ${
        isLog
          ? 'bg-cyan-900 text-cyan-300'
          : 'bg-violet-900 text-violet-300'
      }`}
    >
      {type}
    </span>
  );
}

/**
 * Severity badge for logs or status for spans.
 */
function SeverityBadge({ severity, type }: { severity: string; type: string }) {
  // For spans, severity is actually status_code
  if (type === 'SPAN') {
    const isError = severity === '2';
    return (
      <span
        className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-medium ${
          isError
            ? 'bg-red-900 text-red-300'
            : 'bg-green-900 text-green-300'
        }`}
      >
        {isError ? 'ERROR' : 'OK'}
      </span>
    );
  }

  // For logs, show severity level
  const isError = severity === 'ERROR' || severity === 'FATAL' || severity === 'CRITICAL';
  const isWarn = severity === 'WARN' || severity === 'WARNING';

  let colorClasses = 'bg-slate-700 text-slate-300';
  if (isError) {
    colorClasses = 'bg-red-900 text-red-300';
  } else if (isWarn) {
    colorClasses = 'bg-yellow-900 text-yellow-300';
  }

  return (
    <span
      className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-medium ${colorClasses}`}
    >
      {severity || 'INFO'}
    </span>
  );
}

/**
 * Format timestamp for display.
 */
function formatTimestamp(timestampMs: bigint | number): string {
  const ms = typeof timestampMs === 'bigint' ? Number(timestampMs) : timestampMs;
  const date = new Date(ms);

  // Format as HH:MM:SS.mmm
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');
  const seconds = date.getSeconds().toString().padStart(2, '0');
  const millis = date.getMilliseconds().toString().padStart(3, '0');

  return `${hours}:${minutes}:${seconds}.${millis}`;
}

/**
 * Record row type from DuckDB query.
 */
interface RecordRow {
  type: string;
  timestamp_ms: bigint | number;
  message: string;
  severity_text: string;
}

/**
 * Records drilldown panel component.
 * Displays logs and traces from DuckDB/Iceberg for a given time range.
 */
export function RecordsPanel({ service, timeRange, onClose }: RecordsPanelProps) {
  const { credentials } = useCredentials();
  const bucketName = credentials?.bucketName ?? null;
  const r2Token = credentials?.r2Token ?? null;

  const { executeQuery, loading: dbLoading, error: dbError, isConnected } = useDuckDB(
    bucketName,
    r2Token
  );

  const [filter, setFilter] = useState<string>('');
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryLoading, setQueryLoading] = useState<boolean>(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [useMockData, setUseMockData] = useState<boolean>(false);

  // Execute query when connection is ready or when filter changes
  const runQuery = useCallback(async () => {
    if (!bucketName) {
      // Use mock data if no bucket configured
      setUseMockData(true);
      setQueryResult(generateMockResult(service, timeRange.from, timeRange.to));
      return;
    }

    if (!isConnected) {
      return;
    }

    setQueryLoading(true);
    setQueryError(null);
    setUseMockData(false);

    try {
      const sql = buildRecordsQuery(
        bucketName,
        service,
        timeRange.from.getTime(),
        timeRange.to.getTime(),
        filter || undefined
      );

      const result = await executeQuery(sql);
      setQueryResult(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Query failed';
      setQueryError(message);

      // Fall back to mock data on error
      setUseMockData(true);
      setQueryResult(generateMockResult(service, timeRange.from, timeRange.to));
    } finally {
      setQueryLoading(false);
    }
  }, [bucketName, isConnected, executeQuery, service, timeRange, filter]);

  // Run query when connected
  useEffect(() => {
    if (isConnected || !bucketName) {
      runQuery();
    }
  }, [isConnected, runQuery, bucketName]);

  // Handle filter submission
  const handleFilterSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      runQuery();
    },
    [runQuery]
  );

  // Clear filter
  const handleClearFilter = useCallback(() => {
    setFilter('');
    // Trigger re-query with empty filter
    setTimeout(runQuery, 0);
  }, [runQuery]);

  // Format time range for display
  const timeRangeLabel = useMemo(() => {
    const fromStr = timeRange.from.toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
    });
    const toStr = timeRange.to.toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
    });
    return `${fromStr} - ${toStr}`;
  }, [timeRange]);

  const isLoading = dbLoading || queryLoading;
  const error = dbError || queryError;

  return (
    <div className="rounded-lg border border-slate-700 bg-slate-800">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-slate-700 px-4 py-3">
        <div className="flex items-center gap-3">
          <h3 className="text-sm font-medium text-slate-100">Records</h3>
          <span className="text-xs text-slate-500">{timeRangeLabel}</span>
          {useMockData && (
            <span className="text-xs text-yellow-500">(Demo data)</span>
          )}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-slate-100 transition-colors"
          aria-label="Close records panel"
        >
          <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Filter Bar */}
      <div className="border-b border-slate-700 px-4 py-2">
        <form onSubmit={handleFilterSubmit} className="flex items-center gap-2">
          <label className="text-xs text-slate-400">Filter:</label>
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="severity_text = 'ERROR'"
            className="flex-1 rounded bg-slate-900 px-3 py-1.5 text-sm text-slate-100 placeholder-slate-500 border border-slate-600 focus:border-cyan-500 focus:outline-none"
          />
          <button
            type="button"
            onClick={handleClearFilter}
            className="rounded bg-slate-700 px-3 py-1.5 text-xs text-slate-300 hover:bg-slate-600 transition-colors"
          >
            Clear
          </button>
          <button
            type="submit"
            className="rounded bg-slate-700 px-3 py-1.5 text-xs text-slate-300 hover:bg-slate-600 transition-colors"
            title="Apply SQL WHERE clause filter"
          >
            SQL
          </button>
        </form>
      </div>

      {/* Content */}
      <div className="max-h-80 overflow-auto">
        {isLoading && (
          <div className="flex items-center justify-center py-8">
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-cyan-500" />
          </div>
        )}

        {error && !useMockData && (
          <div className="p-4 text-center text-red-400 text-sm">
            {error}
          </div>
        )}

        {!isLoading && queryResult && queryResult.rows.length === 0 && (
          <div className="p-8 text-center text-slate-500 text-sm">
            No records found for this time range
          </div>
        )}

        {!isLoading && queryResult && queryResult.rows.length > 0 && (
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-slate-800">
              <tr className="text-left text-xs text-slate-400">
                <th className="px-4 py-2 font-medium">Type</th>
                <th className="px-4 py-2 font-medium">Timestamp</th>
                <th className="px-4 py-2 font-medium">Message / Operation</th>
                <th className="px-4 py-2 font-medium">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700">
              {queryResult.rows.map((row: Record<string, unknown>, index: number) => {
                const record = row as unknown as RecordRow;
                return (
                  <tr
                    key={`${record.timestamp_ms}-${index}`}
                    className="hover:bg-slate-750 transition-colors"
                  >
                    <td className="px-4 py-2">
                      <TypeBadge type={record.type} />
                    </td>
                    <td className="px-4 py-2 text-slate-300 font-mono text-xs">
                      {formatTimestamp(record.timestamp_ms)}
                    </td>
                    <td className="px-4 py-2 text-slate-100 max-w-md truncate" title={record.message}>
                      {record.message}
                    </td>
                    <td className="px-4 py-2">
                      <SeverityBadge
                        severity={record.severity_text}
                        type={record.type}
                      />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Footer with record count */}
      {queryResult && queryResult.rows.length > 0 && (
        <div className="border-t border-slate-700 px-4 py-2 text-xs text-slate-500">
          Showing {queryResult.rows.length} records
        </div>
      )}
    </div>
  );
}
