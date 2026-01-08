import { useState, useEffect, useCallback, useRef } from 'react';
import type { AsyncDuckDB, AsyncDuckDBConnection } from '@duckdb/duckdb-wasm';
import {
  initDuckDB,
  connectToR2,
  executeQuery as execQuery,
  type R2Config,
  type QueryResult,
} from '../lib/duckdb';

// Re-export QueryResult for consumers
export type { QueryResult } from '../lib/duckdb';

/**
 * Result returned by the useDuckDB hook.
 */
export interface UseDuckDBResult {
  /** Execute a SQL query against the database */
  executeQuery: (sql: string) => Promise<QueryResult>;
  /** Whether the database is currently loading */
  loading: boolean;
  /** Current error message, if any */
  error: string | null;
  /** Whether the database is connected and ready */
  isConnected: boolean;
}

/**
 * Hook to manage DuckDB WASM connection and queries.
 *
 * @param workerUrl - Worker URL for API and proxying R2 catalog requests
 * @param r2Token - R2 API token for direct data access (parquet files)
 */
export function useDuckDB(
  workerUrl: string | null,
  r2Token: string | null
): UseDuckDBResult {
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [isConnected, setIsConnected] = useState<boolean>(false);

  const dbRef = useRef<AsyncDuckDB | null>(null);
  const connRef = useRef<AsyncDuckDBConnection | null>(null);

  // Initialize DuckDB and establish connection
  useEffect(() => {
    let mounted = true;

    async function initialize() {
      setLoading(true);
      setError(null);

      try {
        // Initialize DuckDB WASM
        const db = await initDuckDB();
        if (!mounted) return;
        dbRef.current = db;

        // Create connection with R2 config if credentials are provided
        if (workerUrl && r2Token) {
          const config: R2Config = {
            workerUrl,
            r2Token,
          };
          const status = await connectToR2(db, config);
          if (!mounted) {
            await status.connection.close();
            return;
          }
          connRef.current = status.connection;

          // Surface any warnings from connection setup
          if (status.warnings.length > 0) {
            console.warn('DuckDB connection warnings:', status.warnings);
          }
        } else {
          // Basic connection without R2
          const conn = await db.connect();
          if (!mounted) {
            await conn.close();
            return;
          }
          connRef.current = conn;
        }

        setIsConnected(true);
        setError(null);
      } catch (err) {
        if (mounted) {
          let message: string;

          if (err instanceof Error) {
            const errMsg = err.message.toLowerCase();
            // Categorize errors for actionable messages
            if (errMsg.includes('secret') || errMsg.includes('credentials') || errMsg.includes('token') || errMsg.includes('permission')) {
              message = err.message; // Already has good context from duckdb.ts
            } else if (errMsg.includes('catalog') || errMsg.includes('attach')) {
              message = err.message; // Already has good context from duckdb.ts
            } else if (errMsg.includes('fetch') || errMsg.includes('network') || errMsg.includes('cors') || errMsg.includes('failed to fetch')) {
              message = 'Network error connecting to DuckDB. Check your internet connection and Worker URL.';
            } else if (errMsg.includes('webassembly') || errMsg.includes('wasm')) {
              message = 'Browser does not support WebAssembly. Try a modern browser like Chrome, Firefox, or Edge.';
            } else {
              message = `DuckDB initialization failed: ${err.message}`;
            }
          } else {
            message = 'Failed to initialize DuckDB';
          }

          console.error('DuckDB error:', err);
          setError(message);
          setIsConnected(false);
        }
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    initialize();

    return () => {
      mounted = false;
      // Cleanup connection on unmount
      if (connRef.current) {
        connRef.current.close().catch(console.error);
        connRef.current = null;
      }
    };
  }, [workerUrl, r2Token]);

  // Execute a query against the database
  const executeQuery = useCallback(async (sql: string): Promise<QueryResult> => {
    if (!connRef.current) {
      throw new Error('Database not connected');
    }

    return execQuery(connRef.current, sql);
  }, []);

  return {
    executeQuery,
    loading,
    error,
    isConnected,
  };
}
