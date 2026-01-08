import type { Story } from '@ladle/react';
import { OverviewBar, type OverviewData } from './OverviewBar';

export default {
  title: 'Components/OverviewBar',
};

// Sample histogram data for sparklines
const sampleHistogram = [2, 5, 8, 12, 7, 4, 9, 15, 11, 6, 3, 8];

// ============================================================================
// Logs Stories
// ============================================================================

export const LogsBasic: Story = () => {
  const data: OverviewData = {
    type: 'logs',
    recordCount: 1247,
    errorCount: 23,
    histogram: sampleHistogram,
  };

  return <OverviewBar data={data} />;
};
LogsBasic.meta = {
  description: 'Logs overview with record count, errors, and sparkline histogram',
};

export const LogsNoErrors: Story = () => {
  const data: OverviewData = {
    type: 'logs',
    recordCount: 5432,
    errorCount: 0,
    histogram: [3, 5, 4, 6, 7, 5, 4, 6, 5, 4],
  };

  return <OverviewBar data={data} />;
};
LogsNoErrors.meta = {
  description: 'Logs with zero errors - no red highlighting',
};

export const LogsHighVolume: Story = () => {
  const data: OverviewData = {
    type: 'logs',
    recordCount: 1_234_567,
    errorCount: 4521,
    histogram: [10, 25, 40, 35, 50, 45, 60, 55, 70, 65, 80, 75],
  };

  return <OverviewBar data={data} />;
};
LogsHighVolume.meta = {
  description: 'High volume logs with many records and errors',
};

export const LogsNoHistogram: Story = () => {
  const data: OverviewData = {
    type: 'logs',
    recordCount: 847,
    errorCount: 12,
  };

  return <OverviewBar data={data} />;
};
LogsNoHistogram.meta = {
  description: 'Logs without histogram data',
};

// ============================================================================
// Traces Stories
// ============================================================================

export const TracesBasic: Story = () => {
  const data: OverviewData = {
    type: 'traces',
    traceCount: 89,
    errorCount: 12,
    p50Ms: 45,
    p99Ms: 320,
  };

  return <OverviewBar data={data} />;
};
TracesBasic.meta = {
  description: 'Traces list with count, errors, and latency percentiles',
};

export const TracesNoErrors: Story = () => {
  const data: OverviewData = {
    type: 'traces',
    traceCount: 256,
    errorCount: 0,
    p50Ms: 28,
    p99Ms: 145,
  };

  return <OverviewBar data={data} />;
};
TracesNoErrors.meta = {
  description: 'Traces with zero errors',
};

export const TracesHighLatency: Story = () => {
  const data: OverviewData = {
    type: 'traces',
    traceCount: 1234,
    errorCount: 89,
    p50Ms: 850,
    p99Ms: 4500,
  };

  return <OverviewBar data={data} />;
};
TracesHighLatency.meta = {
  description: 'Traces with high latency values (shows seconds formatting)',
};

// ============================================================================
// Single Trace Stories
// ============================================================================

export const SingleTraceBasic: Story = () => {
  const data: OverviewData = {
    type: 'single-trace',
    traceId: 'abc123def456789012345678',
    spanCount: 14,
    totalMs: 847,
    errorCount: 1,
  };

  return <OverviewBar data={data} />;
};
SingleTraceBasic.meta = {
  description: 'Single trace view with span count and duration',
};

export const SingleTraceNoErrors: Story = () => {
  const data: OverviewData = {
    type: 'single-trace',
    traceId: 'fedcba987654321098765432',
    spanCount: 8,
    totalMs: 234,
    errorCount: 0,
  };

  return <OverviewBar data={data} />;
};
SingleTraceNoErrors.meta = {
  description: 'Single trace with no errors',
};

export const SingleTraceManySpans: Story = () => {
  const data: OverviewData = {
    type: 'single-trace',
    traceId: '0123456789abcdef01234567',
    spanCount: 127,
    totalMs: 3245,
    errorCount: 5,
  };

  return <OverviewBar data={data} />;
};
SingleTraceManySpans.meta = {
  description: 'Complex trace with many spans and errors',
};

export const SingleTraceShortId: Story = () => {
  const data: OverviewData = {
    type: 'single-trace',
    traceId: 'abc123',
    spanCount: 3,
    totalMs: 45,
    errorCount: 0,
  };

  return <OverviewBar data={data} />;
};
SingleTraceShortId.meta = {
  description: 'Trace with short ID (no truncation needed)',
};

// ============================================================================
// Live Tail Stories
// ============================================================================

export const TailBasic: Story = () => {
  const data: OverviewData = {
    type: 'tail',
    recordCount: 142,
    rate: 12,
    droppedCount: 0,
  };

  return <OverviewBar data={data} />;
};
TailBasic.meta = {
  description: 'Live tail with pulsing indicator and stats',
};

export const TailHighThroughput: Story = () => {
  const data: OverviewData = {
    type: 'tail',
    recordCount: 15834,
    rate: 245,
    droppedCount: 0,
  };

  return <OverviewBar data={data} />;
};
TailHighThroughput.meta = {
  description: 'High throughput live tail',
};

export const TailWithDrops: Story = () => {
  const data: OverviewData = {
    type: 'tail',
    recordCount: 8921,
    rate: 350,
    droppedCount: 127,
  };

  return <OverviewBar data={data} />;
};
TailWithDrops.meta = {
  description: 'Live tail with dropped records (warning color)',
};

// ============================================================================
// SQL Query Stories
// ============================================================================

export const SqlBasic: Story = () => {
  const data: OverviewData = {
    type: 'sql',
    rowCount: 500,
    columnCount: 12,
    queryTimeMs: 234,
  };

  return <OverviewBar data={data} />;
};
SqlBasic.meta = {
  description: 'SQL query result overview',
};

export const SqlFastQuery: Story = () => {
  const data: OverviewData = {
    type: 'sql',
    rowCount: 25,
    columnCount: 5,
    queryTimeMs: 12,
  };

  return <OverviewBar data={data} />;
};
SqlFastQuery.meta = {
  description: 'Fast SQL query with few results',
};

export const SqlSlowQuery: Story = () => {
  const data: OverviewData = {
    type: 'sql',
    rowCount: 10000,
    columnCount: 24,
    queryTimeMs: 3456,
  };

  return <OverviewBar data={data} />;
};
SqlSlowQuery.meta = {
  description: 'Slow SQL query with many results (shows seconds)',
};

export const SqlEmptyResult: Story = () => {
  const data: OverviewData = {
    type: 'sql',
    rowCount: 0,
    columnCount: 8,
    queryTimeMs: 45,
  };

  return <OverviewBar data={data} />;
};
SqlEmptyResult.meta = {
  description: 'SQL query with no matching rows',
};

// ============================================================================
// Comparison View
// ============================================================================

export const AllVariants: Story = () => {
  const variants: OverviewData[] = [
    { type: 'logs', recordCount: 1247, errorCount: 23, histogram: sampleHistogram },
    { type: 'traces', traceCount: 89, errorCount: 12, p50Ms: 45, p99Ms: 320 },
    { type: 'single-trace', traceId: 'abc123def456', spanCount: 14, totalMs: 847, errorCount: 1 },
    { type: 'tail', recordCount: 142, rate: 12, droppedCount: 0 },
    { type: 'sql', rowCount: 500, columnCount: 12, queryTimeMs: 234 },
  ];

  return (
    <div className="space-y-4">
      {variants.map((data, i) => (
        <div key={i}>
          <div className="text-xs uppercase tracking-wider mb-1 px-1" style={{ color: 'var(--color-text-muted)' }}>
            {data.type}
          </div>
          <OverviewBar data={data} />
        </div>
      ))}
    </div>
  );
};
AllVariants.meta = {
  description: 'All overview bar variants displayed together for comparison',
};
