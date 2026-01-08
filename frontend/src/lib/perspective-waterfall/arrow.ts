/**
 * Parse Arrow table data into RawSpan objects
 */

import type { RawSpan } from './types';

/**
 * Column name mappings - handle variations in column names
 */
const COLUMN_ALIASES: Record<keyof RawSpan, string[]> = {
  trace_id: ['trace_id'],
  span_id: ['span_id'],
  parent_span_id: ['parent_span_id'],
  service_name: ['service_name'],
  span_name: ['span_name'],
  timestamp: ['timestamp', 'start_timestamp'],
  end_timestamp: ['end_timestamp'],
  duration: ['duration', 'duration_ms'],
  status_code: ['status_code'],
  span_attributes: ['span_attributes'],
  resource_attributes: ['resource_attributes'],
  scope_attributes: ['scope_attributes'],
};

/**
 * Find actual column name from aliases
 */
function findColumn(columns: string[], field: keyof RawSpan): string | null {
  const aliases = COLUMN_ALIASES[field];
  for (const alias of aliases) {
    if (columns.includes(alias)) {
      return alias;
    }
  }
  return null;
}

/**
 * Parse columnar data (from Perspective table.view().to_columns())
 * into RawSpan array
 */
export function parseColumnarData(
  data: Record<string, unknown[]>
): RawSpan[] {
  const columns = Object.keys(data);

  // Find required columns
  const traceIdCol = findColumn(columns, 'trace_id');
  const spanIdCol = findColumn(columns, 'span_id');
  const parentSpanIdCol = findColumn(columns, 'parent_span_id');
  const serviceNameCol = findColumn(columns, 'service_name');
  const spanNameCol = findColumn(columns, 'span_name');
  const timestampCol = findColumn(columns, 'timestamp');
  const endTimestampCol = findColumn(columns, 'end_timestamp');
  const durationCol = findColumn(columns, 'duration');
  const statusCodeCol = findColumn(columns, 'status_code');
  const spanAttrsCol = findColumn(columns, 'span_attributes');
  const resourceAttrsCol = findColumn(columns, 'resource_attributes');
  const scopeAttrsCol = findColumn(columns, 'scope_attributes');

  // Validate required columns
  if (!traceIdCol || !spanIdCol || !spanNameCol) {
    console.warn('Missing required columns for waterfall', { columns });
    return [];
  }

  const rowCount = data[traceIdCol]?.length ?? 0;
  const spans: RawSpan[] = [];

  for (let i = 0; i < rowCount; i++) {
    const timestamp = timestampCol ? toNumber(data[timestampCol][i]) : 0;
    const endTimestamp = endTimestampCol ? toNumber(data[endTimestampCol][i]) : timestamp;
    const duration = durationCol
      ? toNumber(data[durationCol][i])
      : endTimestamp - timestamp;

    spans.push({
      trace_id: toString(data[traceIdCol][i]),
      span_id: toString(data[spanIdCol][i]),
      parent_span_id: parentSpanIdCol ? toStringOrNull(data[parentSpanIdCol][i]) : null,
      service_name: serviceNameCol ? toString(data[serviceNameCol][i]) : 'unknown',
      span_name: toString(data[spanNameCol][i]),
      timestamp,
      end_timestamp: endTimestamp,
      duration,
      status_code: statusCodeCol ? toString(data[statusCodeCol][i]) : 'UNSET',
      span_attributes: spanAttrsCol ? toString(data[spanAttrsCol][i]) : undefined,
      resource_attributes: resourceAttrsCol ? toString(data[resourceAttrsCol][i]) : undefined,
      scope_attributes: scopeAttrsCol ? toString(data[scopeAttrsCol][i]) : undefined,
    });
  }

  return spans;
}

function toString(value: unknown): string {
  if (value === null || value === undefined) return '';
  return String(value);
}

function toStringOrNull(value: unknown): string | null {
  if (value === null || value === undefined || value === '') return null;
  return String(value);
}

function toNumber(value: unknown): number {
  if (typeof value === 'number') return value;
  if (typeof value === 'bigint') return Number(value);
  if (value instanceof Date) return value.getTime();
  if (typeof value === 'string') {
    const n = parseFloat(value);
    return isNaN(n) ? 0 : n;
  }
  return 0;
}
