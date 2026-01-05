/**
 * OverviewBar Component
 *
 * A compact horizontal strip (~40px) showing contextual stats
 * that adapts to the current data type (logs, traces, single trace, tail, SQL).
 */

import { formatNumber, formatMs } from '../lib/format';

/**
 * Discriminated union of all overview data types.
 */
export type OverviewData =
  | { type: 'logs'; recordCount: number; errorCount: number; histogram?: number[] }
  | { type: 'traces'; traceCount: number; errorCount: number; p50Ms: number; p99Ms: number }
  | { type: 'single-trace'; traceId: string; spanCount: number; totalMs: number; errorCount: number }
  | { type: 'tail'; recordCount: number; rate: number; droppedCount: number }
  | { type: 'sql'; rowCount: number; columnCount: number; queryTimeMs: number };

export interface OverviewBarProps {
  data: OverviewData;
}

/**
 * Calculate error percentage.
 */
function errorPercent(errorCount: number, total: number): string {
  if (total === 0) return '0%';
  const pct = (errorCount / total) * 100;
  return pct < 0.1 && pct > 0 ? '<0.1%' : `${pct.toFixed(1)}%`;
}

/**
 * Truncate trace ID for display.
 */
function truncateTraceId(traceId: string): string {
  if (traceId.length <= 12) return traceId;
  return `${traceId.slice(0, 8)}...`;
}

/**
 * Simple bar sparkline using SVG.
 * Renders as small vertical bars representing relative values.
 */
function BarSparkline({ data }: { data: number[] }) {
  if (!data || data.length === 0) return null;

  const max = Math.max(...data, 1);
  const barWidth = 4;
  const gap = 2;
  const height = 16;
  const width = data.length * (barWidth + gap) - gap;

  return (
    <svg
      width={width}
      height={height}
      className="inline-block align-middle"
      style={{ marginLeft: 8 }}
      aria-label="Activity histogram"
    >
      {data.map((value, i) => {
        const barHeight = Math.max(2, (value / max) * height);
        return (
          <rect
            key={i}
            x={i * (barWidth + gap)}
            y={height - barHeight}
            width={barWidth}
            height={barHeight}
            fill="var(--color-text-muted)"
            opacity={0.6}
            rx={1}
          />
        );
      })}
    </svg>
  );
}

/**
 * Pulsing live indicator dot.
 */
function LiveIndicator() {
  return (
    <span
      className="inline-flex items-center gap-1.5"
      style={{ color: 'var(--color-healthy)' }}
    >
      <span
        className="relative flex h-2 w-2"
        aria-label="Live streaming"
      >
        <span
          className="absolute inline-flex h-full w-full rounded-full opacity-75"
          style={{
            backgroundColor: 'var(--color-healthy)',
            animation: 'pulse 1.5s cubic-bezier(0.4, 0, 0.6, 1) infinite',
          }}
        />
        <span
          className="relative inline-flex rounded-full h-2 w-2"
          style={{ backgroundColor: 'var(--color-healthy)' }}
        />
      </span>
      <span className="font-medium">Live</span>
    </span>
  );
}

/**
 * Separator dot between stats.
 */
function Separator() {
  return (
    <span
      className="mx-2"
      style={{ color: 'var(--color-text-muted)' }}
      aria-hidden="true"
    >
      ·
    </span>
  );
}

/**
 * Error count display with red highlighting when errors exist.
 */
function ErrorStat({ count, total, label = 'errors' }: { count: number; total?: number; label?: string }) {
  const hasErrors = count > 0;
  const pct = total !== undefined ? ` (${errorPercent(count, total)})` : '';

  return (
    <span style={{ color: hasErrors ? 'var(--color-error)' : 'var(--color-text-secondary)' }}>
      <span className="mono">{formatNumber(count)}</span>
      {' '}
      {count === 1 ? label.replace(/s$/, '') : label}
      {pct && <span className="mono">{pct}</span>}
    </span>
  );
}

/**
 * Logs overview content.
 * Example: "1,247 records · 23 errors (1.8%) · ▁▂▃▅▂▁"
 */
function LogsOverview({ data }: { data: Extract<OverviewData, { type: 'logs' }> }) {
  return (
    <>
      <span style={{ color: 'var(--color-text-secondary)' }}>
        <span className="mono">{formatNumber(data.recordCount)}</span>
        {' records'}
      </span>
      <Separator />
      <ErrorStat count={data.errorCount} total={data.recordCount} />
      {data.histogram && data.histogram.length > 0 && (
        <BarSparkline data={data.histogram} />
      )}
    </>
  );
}

/**
 * Traces list overview content.
 * Example: "89 traces · 12 errors (13%) · p50: 45ms p99: 320ms"
 */
function TracesOverview({ data }: { data: Extract<OverviewData, { type: 'traces' }> }) {
  return (
    <>
      <span style={{ color: 'var(--color-text-secondary)' }}>
        <span className="mono">{formatNumber(data.traceCount)}</span>
        {' traces'}
      </span>
      <Separator />
      <ErrorStat count={data.errorCount} total={data.traceCount} />
      <Separator />
      <span style={{ color: 'var(--color-text-secondary)' }}>
        p50:{' '}
        <span className="mono">{formatMs(data.p50Ms)}</span>
      </span>
      <span className="mx-1.5" style={{ color: 'var(--color-text-muted)' }}>/</span>
      <span style={{ color: 'var(--color-text-secondary)' }}>
        p99:{' '}
        <span className="mono">{formatMs(data.p99Ms)}</span>
      </span>
    </>
  );
}

/**
 * Single trace overview content.
 * Example: "trace abc123... · 14 spans · total: 847ms · 1 error"
 */
function SingleTraceOverview({ data }: { data: Extract<OverviewData, { type: 'single-trace' }> }) {
  return (
    <>
      <span style={{ color: 'var(--color-text-secondary)' }}>
        trace{' '}
        <span className="mono" title={data.traceId}>
          {truncateTraceId(data.traceId)}
        </span>
      </span>
      <Separator />
      <span style={{ color: 'var(--color-text-secondary)' }}>
        <span className="mono">{data.spanCount}</span>
        {' spans'}
      </span>
      <Separator />
      <span style={{ color: 'var(--color-text-secondary)' }}>
        total:{' '}
        <span className="mono">{formatMs(data.totalMs)}</span>
      </span>
      <Separator />
      <ErrorStat count={data.errorCount} label="errors" />
    </>
  );
}

/**
 * Live tail overview content.
 * Example: "● Live · 142 records · ~12/sec · 0 dropped"
 */
function TailOverview({ data }: { data: Extract<OverviewData, { type: 'tail' }> }) {
  const hasDropped = data.droppedCount > 0;

  return (
    <>
      <LiveIndicator />
      <Separator />
      <span style={{ color: 'var(--color-text-secondary)' }}>
        <span className="mono">{formatNumber(data.recordCount)}</span>
        {' records'}
      </span>
      <Separator />
      <span style={{ color: 'var(--color-text-secondary)' }}>
        ~<span className="mono">{formatNumber(data.rate)}</span>/sec
      </span>
      <Separator />
      <span style={{ color: hasDropped ? 'var(--color-warning)' : 'var(--color-text-muted)' }}>
        <span className="mono">{formatNumber(data.droppedCount)}</span>
        {' dropped'}
      </span>
    </>
  );
}

/**
 * SQL query result overview content.
 * Example: "500 rows · 12 columns · queried in 234ms"
 */
function SqlOverview({ data }: { data: Extract<OverviewData, { type: 'sql' }> }) {
  return (
    <>
      <span style={{ color: 'var(--color-text-secondary)' }}>
        <span className="mono">{formatNumber(data.rowCount)}</span>
        {' rows'}
      </span>
      <Separator />
      <span style={{ color: 'var(--color-text-secondary)' }}>
        <span className="mono">{data.columnCount}</span>
        {' columns'}
      </span>
      <Separator />
      <span style={{ color: 'var(--color-text-muted)' }}>
        queried in{' '}
        <span className="mono">{formatMs(data.queryTimeMs)}</span>
      </span>
    </>
  );
}

/**
 * Render the appropriate overview content based on data type.
 */
function renderOverviewContent(data: OverviewData) {
  switch (data.type) {
    case 'logs':
      return <LogsOverview data={data} />;
    case 'traces':
      return <TracesOverview data={data} />;
    case 'single-trace':
      return <SingleTraceOverview data={data} />;
    case 'tail':
      return <TailOverview data={data} />;
    case 'sql':
      return <SqlOverview data={data} />;
  }
}

/**
 * OverviewBar - A compact horizontal strip showing contextual stats.
 *
 * Sits between the input area and the evidence area (main data grid).
 * Adapts its content based on the current data type being displayed.
 */
export function OverviewBar({ data }: OverviewBarProps) {
  return (
    <div
      className="flex items-center px-4 text-sm select-none"
      style={{
        height: 40,
        backgroundColor: 'var(--color-paper-cool)',
        borderBottom: '1px solid var(--color-border-light)',
      }}
      role="status"
      aria-live="polite"
    >
      {renderOverviewContent(data)}
    </div>
  );
}
