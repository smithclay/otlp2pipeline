/**
 * Mock data for UI component stories
 */

import type { Service, LogStats, TraceStats } from '../lib/api'
import type { ServiceWithStats, ServiceDetailStats } from '../components/ServiceHealthCards'
import type { CatalogStats, TableStats } from '../hooks/useCatalogStats'
import type { LayoutSpan } from '../lib/perspective-waterfall/types'

// ============================================================================
// Service Mocks
// ============================================================================

export const mockServices: Service[] = [
  { name: 'api-gateway', has_logs: true, has_traces: true, has_metrics: false },
  { name: 'user-service', has_logs: true, has_traces: true, has_metrics: true },
  { name: 'payment-service', has_logs: true, has_traces: true, has_metrics: false },
  { name: 'notification-worker', has_logs: true, has_traces: false, has_metrics: false },
  { name: 'analytics-pipeline', has_logs: true, has_traces: true, has_metrics: true },
  { name: 'auth-service', has_logs: true, has_traces: true, has_metrics: false },
  { name: 'search-indexer', has_logs: true, has_traces: false, has_metrics: true },
]

export const mockServicesWithStats: ServiceWithStats[] = [
  {
    service: mockServices[0],
    errorRate: 0.2,
    totalCount: 15420,
    errorCount: 31,
  },
  {
    service: mockServices[1],
    errorRate: 0,
    totalCount: 8750,
    errorCount: 0,
  },
  {
    service: mockServices[2],
    errorRate: 7.5,
    totalCount: 3200,
    errorCount: 240,
  },
  {
    service: mockServices[3],
    errorRate: 1.2,
    totalCount: 950,
    errorCount: 11,
  },
  {
    service: mockServices[4],
    errorRate: 0,
    totalCount: 12000,
    errorCount: 0,
  },
]

// ============================================================================
// Stats Mocks
// ============================================================================

function generateMinutes(count: number): string[] {
  const now = new Date()
  return Array.from({ length: count }, (_, i) => {
    const d = new Date(now.getTime() - (count - i - 1) * 60 * 1000)
    return d.toISOString().slice(0, 16)
  })
}

const minutes = generateMinutes(15)

export const mockLogStats: LogStats[] = minutes.map((minute, i) => ({
  minute,
  count: 100 + Math.floor(Math.random() * 50) + (i % 3 === 0 ? 30 : 0),
  error_count: Math.floor(Math.random() * 5),
}))

export const mockTraceStats: TraceStats[] = minutes.map((minute, i) => ({
  minute,
  count: 80 + Math.floor(Math.random() * 40),
  error_count: Math.floor(Math.random() * 3),
  latency_sum_us: (50 + Math.random() * 100) * 1000 * (80 + Math.floor(Math.random() * 40)),
  latency_min_us: 5000 + Math.floor(Math.random() * 10000),
  latency_max_us: 200000 + Math.floor(Math.random() * 300000),
}))

export const mockDetailStats: ServiceDetailStats = {
  logStats: mockLogStats,
  traceStats: mockTraceStats,
}

// ============================================================================
// Catalog Stats Mocks
// ============================================================================

const mockTableStats: TableStats[] = [
  {
    namespace: 'default',
    name: 'logs',
    fileCount: 42,
    recordCount: 1_234_567,
    snapshotCount: 15,
    lastUpdatedMs: Date.now() - 5 * 60 * 1000,
    partitionSpec: 'day(__ingest_ts)',
    totalSizeBytes: 256 * 1024 * 1024,
    schemaFields: [
      { name: 'timestamp', type: 'timestamptz' },
      { name: 'service_name', type: 'string' },
      { name: 'severity_number', type: 'int' },
      { name: 'body', type: 'string' },
      { name: 'trace_id', type: 'string' },
      { name: 'span_id', type: 'string' },
    ],
  },
  {
    namespace: 'default',
    name: 'traces',
    fileCount: 28,
    recordCount: 567_890,
    snapshotCount: 12,
    lastUpdatedMs: Date.now() - 2 * 60 * 1000,
    partitionSpec: 'day(__ingest_ts)',
    totalSizeBytes: 128 * 1024 * 1024,
    schemaFields: [
      { name: 'timestamp', type: 'timestamptz' },
      { name: 'trace_id', type: 'string' },
      { name: 'span_id', type: 'string' },
      { name: 'parent_span_id', type: 'string' },
      { name: 'service_name', type: 'string' },
      { name: 'span_name', type: 'string' },
      { name: 'duration_ms', type: 'double' },
      { name: 'status_code', type: 'int' },
    ],
  },
]

export const mockCatalogStats: CatalogStats = {
  tables: mockTableStats,
  totals: {
    tableCount: mockTableStats.length,
    fileCount: mockTableStats.reduce((sum, t) => sum + t.fileCount, 0),
    recordCount: mockTableStats.reduce((sum, t) => sum + t.recordCount, 0),
    snapshotCount: mockTableStats.reduce((sum, t) => sum + t.snapshotCount, 0),
  },
}

export const mockEmptyCatalogStats: CatalogStats = {
  tables: [],
  totals: {
    tableCount: 0,
    fileCount: 0,
    recordCount: 0,
    snapshotCount: 0,
  },
}

// ============================================================================
// Span/Layout Mocks for SpanDetailsPanel
// ============================================================================

export const mockLayoutSpan: LayoutSpan = {
  trace_id: 'abc123def456',
  span_id: 'span001',
  parent_span_id: null,
  service_name: 'api-gateway',
  span_name: 'POST /api/users',
  timestamp: Date.now() - 150,
  end_timestamp: Date.now(),
  duration: 150,
  status_code: 'OK',
  span_attributes: JSON.stringify({
    'http.method': 'POST',
    'http.url': '/api/users',
    'http.status_code': 201,
    'user.id': 'usr_12345',
  }),
  resource_attributes: JSON.stringify({
    'service.name': 'api-gateway',
    'service.version': '1.2.3',
    'deployment.environment': 'production',
  }),
  scope_attributes: JSON.stringify({
    'otel.library.name': '@opentelemetry/instrumentation-http',
    'otel.library.version': '0.41.0',
  }),
  depth: 0,
  row_index: 0,
  is_error: false,
  children: [],
}

export const mockErrorSpan: LayoutSpan = {
  ...mockLayoutSpan,
  span_id: 'span002',
  span_name: 'POST /api/payment',
  service_name: 'payment-service',
  status_code: 'ERROR',
  duration: 2500,
  is_error: true,
  span_attributes: JSON.stringify({
    'http.method': 'POST',
    'http.url': '/api/payment',
    'http.status_code': 500,
    'error.message': 'Payment gateway timeout',
  }),
}
