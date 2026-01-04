/**
 * Waterfall trace viewer types
 */

/**
 * Raw span data from Arrow/Perspective
 */
export interface RawSpan {
  trace_id: string;
  span_id: string;
  parent_span_id: string | null;
  service_name: string;
  span_name: string;
  timestamp: number;        // Start time in ms
  end_timestamp: number;    // End time in ms
  duration: number;         // Duration in ms
  status_code: string;      // 'OK' | 'ERROR' | 'UNSET'
  span_attributes?: string; // JSON string
  resource_attributes?: string;
  scope_attributes?: string;
}

/**
 * Span with computed layout metadata
 */
export interface LayoutSpan extends RawSpan {
  depth: number;           // Tree depth (0 = root)
  row_index: number;       // Y position in waterfall
  is_error: boolean;       // Derived from status_code
  children: LayoutSpan[];  // Child spans
}

/**
 * Trace layout result
 */
export interface TraceLayout {
  spans: LayoutSpan[];     // Flat list ordered by row_index
  roots: LayoutSpan[];     // Root spans (for tree traversal)
  trace_start: number;     // Min timestamp
  trace_end: number;       // Max timestamp
  total_duration: number;  // trace_end - trace_start
}

/**
 * Rendering constants
 */
export const LAYOUT = {
  ROW_HEIGHT: 44,           // Increased for two-line rows
  TREE_PANEL_WIDTH: 280,
  INDENT_PER_DEPTH: 16,
  BAR_HEIGHT: 16,
  BAR_PADDING: 8,           // Center bar in taller row
  TIME_AXIS_HEIGHT: 32,
  TIMELINE_PADDING_RIGHT: 60, // Space for duration labels
} as const;
