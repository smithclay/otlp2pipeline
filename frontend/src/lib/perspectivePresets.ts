/**
 * Perspective Datagrid Preset Configuration
 *
 * Provides opinionated initial configurations for the Perspective datagrid
 * based on data type (logs, traces) and mode (query, tail).
 */

import type { Signal } from './parseCommand';

/**
 * Column style configuration for Perspective datagrid plugin.
 * Maps to plugin_config.columns entries.
 */
export interface ColumnStyleConfig {
  // Number styling
  number_fg_mode?: 'disabled' | 'color' | 'bar' | 'gradient';
  number_bg_mode?: 'disabled' | 'color' | 'gradient' | 'pulse';
  pos_fg_color?: string;
  neg_fg_color?: string;
  pos_bg_color?: string;
  neg_bg_color?: string;
  fg_gradient?: number;
  bg_gradient?: number;
  fixed?: number;

  // Date/time styling
  dateStyle?: 'short' | 'medium' | 'long' | 'full';
  timeStyle?: 'short' | 'medium' | 'long' | 'full';
  timeZone?: string;
  datetime_color_mode?: 'foreground' | 'background';
  color?: string;

  // String styling
  string_color_mode?: 'foreground' | 'background' | 'series';
  format?: 'link' | 'bold' | 'italics';
}

/**
 * Complete Perspective viewer configuration including plugin config.
 */
export interface PerspectivePreset {
  plugin: 'Datagrid';
  theme: string;
  settings: boolean;
  columns?: string[];
  sort?: [string, 'asc' | 'desc'][];
  plugin_config?: {
    columns: Record<string, ColumnStyleConfig>;
  };
}

/**
 * Data context for preset selection.
 */
export interface DataContext {
  signal: Signal;
  mode: 'query' | 'tail';
}

// ============================================================================
// Column Style Definitions
// ============================================================================

/**
 * Common timestamp column styles (UTC formatted).
 * Applied to both logs and traces.
 */
const COMMON_TIMESTAMP_STYLES: Record<string, ColumnStyleConfig> = {
  timestamp: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },
  __ingest_ts: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },
  observed_timestamp: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },
  end_timestamp: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },
};

/**
 * Styling for log-specific columns.
 * Applied when these columns are present in the schema.
 */
const LOG_COLUMN_STYLES: Record<string, ColumnStyleConfig> = {
  // Include common timestamp styles
  ...COMMON_TIMESTAMP_STYLES,

  // Legacy timestamp column names (nanoseconds)
  timestamp_nanos: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },
  observed_timestamp_nanos: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },

  // Severity - color coded (higher = more severe)
  severity_number: {
    number_fg_mode: 'color',
    pos_fg_color: '#dc2626', // red for high severity
    neg_fg_color: '#16a34a', // green for low severity
    fixed: 0,
  },
  severity_text: {
    string_color_mode: 'series',
  },

  // Body - emphasized
  body: {
    format: 'bold',
  },

  // Service identification - series coloring for visual grouping
  service_name: {
    string_color_mode: 'series',
  },
  resource_attributes: {
    // JSON column - no special styling
  },
  scope_name: {
    string_color_mode: 'series',
  },

  // Trace context
  trace_id: {
    color: '#6366f1', // indigo
  },
  span_id: {
    color: '#8b5cf6', // violet
  },
};

/**
 * Styling for trace/span-specific columns.
 * Applied when these columns are present in the schema.
 */
const TRACE_COLUMN_STYLES: Record<string, ColumnStyleConfig> = {
  // Include common timestamp styles
  ...COMMON_TIMESTAMP_STYLES,

  // Legacy timestamp column names (nanoseconds)
  start_time_nanos: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },
  end_time_nanos: {
    dateStyle: 'short',
    timeStyle: 'medium',
    timeZone: 'UTC',
  },

  // Duration - bar visualization for latency
  duration_us: {
    number_fg_mode: 'bar',
    pos_fg_color: '#3b82f6', // blue
    fg_gradient: 100000, // 100ms scale
    fixed: 0,
  },
  duration_ms: {
    number_fg_mode: 'bar',
    pos_fg_color: '#3b82f6',
    fg_gradient: 1000, // 1s scale
    fixed: 2,
  },

  // Status - color coded (0=unset, 1=ok, 2=error)
  status_code: {
    number_fg_mode: 'color',
    pos_fg_color: '#dc2626', // red for error (2)
    neg_fg_color: '#16a34a', // green for ok (1) / unset (0)
    fixed: 0,
  },
  status_message: {
    string_color_mode: 'foreground',
    color: '#dc2626', // red - usually only set on errors
  },

  // Span identification - emphasized
  span_name: {
    format: 'bold',
  },
  span_kind: {
    string_color_mode: 'series',
  },

  // Service identification
  service_name: {
    string_color_mode: 'series',
  },

  // Trace context - subtle coloring
  trace_id: {
    color: '#6366f1', // indigo
  },
  span_id: {
    color: '#8b5cf6', // violet
  },
  parent_span_id: {
    color: '#a78bfa', // lighter violet
  },

  // Counts
  events_count: {
    number_fg_mode: 'color',
    pos_fg_color: '#f59e0b', // amber when > 0
    fixed: 0,
  },
  links_count: {
    number_fg_mode: 'color',
    pos_fg_color: '#06b6d4', // cyan when > 0
    fixed: 0,
  },
};

/**
 * Priority order for log columns in tail mode.
 * Columns are reordered to show most important first.
 */
const LOG_TAIL_PRIORITY = [
  'timestamp',
  'severity_text',
  'service_name',
  'body',
  'trace_id',
  'span_id',
];

/**
 * Priority order for trace columns in tail mode.
 */
const TRACE_TAIL_PRIORITY = [
  'timestamp',
  'service_name',
  'span_name',
  'duration',
  'status_code',
  'trace_id',
  'span_id',
];

// ============================================================================
// Preset Factory
// ============================================================================

/**
 * Get column style definitions for a signal type.
 */
function getColumnStyles(signal: Signal): Record<string, ColumnStyleConfig> {
  switch (signal) {
    case 'logs':
      return LOG_COLUMN_STYLES;
    case 'traces':
      return TRACE_COLUMN_STYLES;
    default:
      return {};
  }
}

/**
 * Get column priority order for tail mode.
 */
function getTailPriority(signal: Signal): string[] {
  switch (signal) {
    case 'logs':
      return LOG_TAIL_PRIORITY;
    case 'traces':
      return TRACE_TAIL_PRIORITY;
    default:
      return [];
  }
}

/**
 * Get the timestamp column name for a signal type.
 */
function getTimestampColumn(_signal: Signal): string {
  // Both logs and traces use 'timestamp' as the primary timestamp column
  return 'timestamp';
}

/**
 * Reorder columns based on priority, keeping unlisted columns at the end.
 */
function prioritizeColumns(schemaColumns: string[], priority: string[]): string[] {
  const prioritized: string[] = [];
  const remaining: string[] = [];

  // Add priority columns that exist in schema
  for (const col of priority) {
    if (schemaColumns.includes(col)) {
      prioritized.push(col);
    }
  }

  // Add remaining columns
  for (const col of schemaColumns) {
    if (!priority.includes(col)) {
      remaining.push(col);
    }
  }

  return [...prioritized, ...remaining];
}

/**
 * Create a Perspective preset configuration based on data context.
 *
 * @param context - The data context (signal type and mode)
 * @param schemaColumns - Column names from the data schema (optional)
 * @returns Complete preset configuration for Perspective viewer
 *
 * @example
 * ```ts
 * const preset = createPreset(
 *   { signal: 'logs', mode: 'tail' },
 *   ['timestamp_nanos', 'severity_text', 'body', 'service_name']
 * );
 * await viewer.restore(preset);
 * ```
 */
export function createPreset(
  context: DataContext,
  schemaColumns?: string[]
): PerspectivePreset {
  const { signal, mode } = context;
  const columnStyles = getColumnStyles(signal);
  const timestampCol = getTimestampColumn(signal);

  // Build plugin_config.columns from styles that match schema
  const columnsConfig: Record<string, ColumnStyleConfig> = {};

  if (schemaColumns) {
    for (const col of schemaColumns) {
      if (col in columnStyles) {
        columnsConfig[col] = columnStyles[col];
      }
    }
  } else {
    // No schema provided - include all styles for the signal type
    Object.assign(columnsConfig, columnStyles);
  }

  // Base configuration
  const preset: PerspectivePreset = {
    plugin: 'Datagrid',
    theme: 'Pro Light',
    settings: mode === 'query', // hide settings panel in tail mode
    plugin_config: {
      columns: columnsConfig,
    },
  };

  // Mode-specific adjustments
  if (mode === 'tail') {
    // Always sort by timestamp descending in tail mode (newest first)
    preset.sort = [[timestampCol, 'desc']];

    // Reorder columns to prioritize important ones
    if (schemaColumns) {
      const priority = getTailPriority(signal);
      preset.columns = prioritizeColumns(schemaColumns, priority);
    }
  }

  return preset;
}

/**
 * Merge a user's saved configuration with a preset.
 * User preferences take precedence for sort, columns, etc.
 * Plugin config is deep-merged to preserve column styles.
 * Invalid columns (not in schema) are filtered out to prevent errors.
 *
 * @param preset - Base preset configuration
 * @param userConfig - User's saved configuration (partial)
 * @param schemaColumns - Valid column names from current data (optional)
 * @returns Merged configuration
 */
export function mergeWithUserConfig(
  preset: PerspectivePreset,
  userConfig: Partial<PerspectivePreset> | null,
  schemaColumns?: string[]
): PerspectivePreset {
  if (!userConfig) {
    return preset;
  }

  // For columns: Don't use saved columns if schema has significantly changed.
  // This prevents old column lists from hiding new schema columns (like 'body').
  // We detect schema mismatch when >20% of saved columns are missing from schema.
  let finalColumns = preset.columns;
  if (userConfig.columns && schemaColumns) {
    const validSavedColumns = userConfig.columns.filter((col) => schemaColumns.includes(col));
    const missingRatio = 1 - validSavedColumns.length / userConfig.columns.length;

    // If schema has changed significantly (>20% columns missing), use preset columns
    // Otherwise, use saved columns but append any new schema columns not in the saved list
    if (missingRatio > 0.2) {
      // Schema changed significantly - use preset
      finalColumns = preset.columns;
    } else {
      // Schema similar - use saved columns but add any new columns from schema
      const newColumns = schemaColumns.filter((col) => !validSavedColumns.includes(col));
      finalColumns = [...validSavedColumns, ...newColumns];
    }
  }

  // Filter user's plugin_config.columns to only include valid columns
  let filteredPluginConfigColumns = userConfig.plugin_config?.columns;
  if (filteredPluginConfigColumns && schemaColumns) {
    filteredPluginConfigColumns = Object.fromEntries(
      Object.entries(filteredPluginConfigColumns).filter(([col]) =>
        schemaColumns.includes(col)
      )
    );
  }

  // Filter sort columns to only include valid columns
  let filteredSort = userConfig.sort;
  if (filteredSort && schemaColumns) {
    filteredSort = filteredSort.filter(([col]) => schemaColumns.includes(col));
    if (filteredSort.length === 0) {
      filteredSort = undefined;
    }
  }

  // Deep merge plugin_config.columns
  const mergedPluginConfig = {
    columns: {
      ...preset.plugin_config?.columns,
      ...filteredPluginConfigColumns,
    },
  };

  return {
    ...preset,
    ...userConfig,
    columns: finalColumns,
    sort: filteredSort ?? userConfig.sort,
    plugin_config: mergedPluginConfig,
  };
}

/**
 * Detect the signal type from column names.
 * Useful when the signal isn't explicitly known.
 */
export function detectSignalFromSchema(columns: string[]): Signal {
  // Check for trace-specific columns
  const traceIndicators = ['span_name', 'span_kind', 'duration', 'parent_span_id'];
  if (traceIndicators.some((col) => columns.includes(col))) {
    return 'traces';
  }

  // Check for log-specific columns
  const logIndicators = ['severity_number', 'severity_text', 'body'];
  if (logIndicators.some((col) => columns.includes(col))) {
    return 'logs';
  }

  // Default to logs
  return 'logs';
}
