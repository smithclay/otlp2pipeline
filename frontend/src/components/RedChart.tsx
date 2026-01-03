import { useEffect, useRef, useState } from 'react';
import { getPerspectiveWorker } from '../lib/perspective';
import type { Table } from '@finos/perspective';
import type { HTMLPerspectiveViewerElement } from '@finos/perspective-viewer';
import { usePerspectiveConfig, type ViewConfig } from '../hooks/usePerspectiveConfig';

// Import Perspective viewer element, plugins, and styles
import '@finos/perspective-viewer';
import '@finos/perspective-viewer-d3fc';
import '@finos/perspective-viewer/dist/css/themes.css';

export interface ChartDataPoint {
  minute: string;
  logs?: number;
  traces?: number;
}

interface RedChartProps {
  title: string;
  data: ChartDataPoint[];
  yLabel: string;
  /** Unique ID for config persistence (e.g., "service-logs-rate") */
  configId?: string;
  /** Show settings panel (default: false) */
  showSettings?: boolean;
  /** Callback when a data point is clicked */
  onPointClick?: (minute: string) => void;
}

/**
 * Chart component using Perspective viewer for RED metrics visualization.
 */
export function RedChart({ title, data, yLabel, configId, showSettings = false, onPointClick }: RedChartProps) {
  const viewerRef = useRef<HTMLPerspectiveViewerElement | null>(null);
  const tableRef = useRef<Table | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { save, load } = usePerspectiveConfig(configId ?? 'default');

  useEffect(() => {
    let mounted = true;
    // Capture refs at effect start to avoid stale closure in cleanup
    const viewer = viewerRef.current;

    async function initChart() {
      if (!viewer || data.length === 0) return;

      try {
        // Wait for the custom element to be defined
        await customElements.whenDefined('perspective-viewer');

        const worker = await getPerspectiveWorker();

        // Perspective expects column-oriented data: { column: [values] }
        // Convert ISO strings to Date objects for proper datetime handling
        const chartData: Record<string, (Date | number)[]> = {
          minute: data.map((d) => new Date(d.minute)),
          logs: data.map((d) => d.logs ?? 0),
          traces: data.map((d) => d.traces ?? 0),
        };

        // Create table from column-oriented data
        const table = await worker.table(chartData);

        if (!mounted) {
          await table.delete();
          return;
        }

        // Store reference (old table will be orphaned but GC'd)
        tableRef.current = table;

        // Load table into viewer - viewer manages view lifecycle automatically
        await viewer.load(table);

        // Determine which columns have data
        const hasLogs = data.some((d) => d.logs !== undefined && d.logs !== null);
        const hasTraces = data.some((d) => d.traces !== undefined && d.traces !== null);

        const columns: string[] = [];
        if (hasLogs) columns.push('logs');
        if (hasTraces) columns.push('traces');

        // Configure viewer for line chart
        const savedConfig = configId ? load() : null;
        const defaultConfig: ViewConfig = {
          plugin: 'Y Line',
          columns: columns.length > 0 ? columns : ['logs', 'traces'],
          group_by: ['minute'],
          settings: showSettings,
          theme: 'Pro Dark',
          title: yLabel,
        };
        // Cast to unknown first since ViewConfig is compatible but not identical to PerspectiveViewerConfig
        await viewer.restore((savedConfig ?? defaultConfig) as unknown as Parameters<typeof viewer.restore>[0]);

        setError(null);
      } catch (err) {
        console.error('Failed to initialize chart:', err);
        if (mounted) {
          setError(err instanceof Error ? err.message : 'Failed to load chart');
        }
      }
    }

    // Create click handler
    const handleClick = ((event: CustomEvent) => {
      const row = event.detail?.row;
      if (row && row.minute && onPointClick) {
        // Perspective returns Date objects as ISO strings or timestamps
        const minuteValue = row.minute instanceof Date
          ? row.minute.toISOString()
          : String(row.minute);
        onPointClick(minuteValue);
      }
    }) as EventListener;

    // Create config change handler for persistence
    const handleConfigChange = async () => {
      if (viewer && configId) {
        try {
          const config = await viewer.save();
          save(config as ViewConfig);
        } catch (err) {
          console.warn('Failed to save Perspective config:', err);
        }
      }
    };

    initChart().then(() => {
      if (viewer && onPointClick) {
        viewer.addEventListener('perspective-click', handleClick);
      }
      if (viewer) {
        viewer.addEventListener('perspective-config-update', handleConfigChange);
      }
    });

    return () => {
      mounted = false;
      // Remove click listener using captured ref
      if (viewer) {
        viewer.removeEventListener('perspective-click', handleClick);
        viewer.removeEventListener('perspective-config-update', handleConfigChange);
      }
      // Note: We don't manually delete tables here because the viewer manages
      // the view lifecycle. Deleting a table with an active view causes errors.
      // Tables will be garbage collected when no longer referenced.
      tableRef.current = null;
    };
  }, [data, yLabel, onPointClick, showSettings, configId, save, load]);

  // Handle clicking on the chart to view records for the middle time point
  const handleViewRecords = () => {
    if (data.length > 0 && onPointClick) {
      // Use the middle data point's timestamp
      const middleIndex = Math.floor(data.length / 2);
      onPointClick(data[middleIndex].minute);
    }
  };

  return (
    <div className="rounded-lg border border-slate-700 bg-slate-800 p-4">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-medium text-slate-300">{title}</h3>
        {data.length > 0 && onPointClick && (
          <button
            onClick={handleViewRecords}
            className="rounded bg-slate-700 px-2 py-1 text-xs text-slate-300 hover:bg-slate-600 transition-colors"
          >
            View Records
          </button>
        )}
      </div>
      {error ? (
        <div className="flex h-48 items-center justify-center text-red-400 text-sm">
          {error}
        </div>
      ) : data.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-slate-500 text-sm">
          No data available
        </div>
      ) : (
        <perspective-viewer
          ref={viewerRef}
          style={{ height: '200px', width: '100%' }}
        />
      )}
    </div>
  );
}
