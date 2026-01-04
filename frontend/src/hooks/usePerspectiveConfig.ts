import { useCallback } from 'react';
import type { ColumnStyleConfig } from '../lib/perspectivePresets';

/**
 * Plugin-specific configuration for the datagrid.
 */
export interface PluginConfig {
  columns?: Record<string, ColumnStyleConfig>;
}

/**
 * Perspective viewer configuration type.
 * Matches the restore/save format from perspective-viewer.
 */
export interface ViewConfig {
  plugin?: string;
  columns?: string[];
  group_by?: string[];
  split_by?: string[];
  sort?: [string, 'asc' | 'desc'][];
  filter?: [string, string, unknown][];
  expressions?: string[];
  aggregates?: Record<string, string>;
  settings?: boolean;
  theme?: string;
  title?: string;
  plugin_config?: PluginConfig;
}

/**
 * Hook for persisting Perspective viewer configurations to localStorage.
 *
 * @param id - Unique identifier for the config (e.g., "service-logs-rate")
 */
export function usePerspectiveConfig(id: string) {
  const key = `perspective-config-${id}`;

  const save = useCallback(
    (config: ViewConfig) => {
      try {
        localStorage.setItem(key, JSON.stringify(config));
      } catch (err) {
        console.warn('Failed to save Perspective config:', err);
      }
    },
    [key]
  );

  const load = useCallback((): ViewConfig | null => {
    try {
      const stored = localStorage.getItem(key);
      return stored ? (JSON.parse(stored) as ViewConfig) : null;
    } catch (err) {
      console.warn('Failed to load Perspective config:', err);
      return null;
    }
  }, [key]);

  const clear = useCallback(() => {
    try {
      localStorage.removeItem(key);
    } catch (err) {
      console.warn('Failed to clear Perspective config:', err);
    }
  }, [key]);

  return { save, load, clear };
}
