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
 * @param bucketName - R2 bucket name for Iceberg queries
 * @param r2Token - R2 API token for authentication
 * @param accountId - Optional Cloudflare account ID
 */
export function useDuckDB(
  bucketName: string | null,
  r2Token: string | null,
  accountId?: string
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
        if (bucketName && r2Token) {
          const config: R2Config = {
            bucketName,
            r2Token,
            accountId,
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
          const message = err instanceof Error ? err.message : 'Failed to initialize DuckDB';
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
  }, [bucketName, r2Token, accountId]);

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

/**
 * Validate and sanitize a SQL WHERE clause fragment.
 *
 * WARNING: This provides basic validation only. The filter is intended for
 * trusted users querying their own data. Do not expose to untrusted input.
 *
 * @param clause - The WHERE clause fragment to validate
 * @returns The validated clause or null if invalid
 */
export function validateWhereClause(clause: string): string | null {
  if (!clause || clause.trim().length === 0) {
    return null;
  }

  const trimmed = clause.trim();

  // Block obvious SQL injection patterns
  const dangerousPatterns = [
    /;\s*(DROP|DELETE|UPDATE|INSERT|ALTER|CREATE|TRUNCATE)/i,
    /--/,  // SQL comments
    /\/\*/,  // Block comments
    /UNION\s+SELECT/i,
    /INTO\s+OUTFILE/i,
    /LOAD_FILE/i,
  ];

  for (const pattern of dangerousPatterns) {
    if (pattern.test(trimmed)) {
      console.warn('Blocked potentially dangerous SQL pattern:', pattern);
      return null;
    }
  }

  // Basic length limit
  if (trimmed.length > 500) {
    console.warn('WHERE clause too long, max 500 characters');
    return null;
  }

  return trimmed;
}

/**
 * Build a query to fetch records for a service within a time range.
 *
 * WARNING: The whereClause parameter accepts raw SQL. Basic validation is
 * performed but this should only be used with trusted input. The filter
 * is intended for power users querying their own observability data.
 *
 * @param bucketName - R2 bucket name
 * @param service - Service name to filter by
 * @param from - Start time in milliseconds
 * @param to - End time in milliseconds
 * @param whereClause - Additional SQL WHERE clause filter (validated, max 500 chars)
 * @param limit - Maximum number of records to return
 */
export function buildRecordsQuery(
  bucketName: string,
  service: string,
  from: number,
  to: number,
  whereClause?: string,
  limit: number = 100
): string {
  const logsPath = `s3://${bucketName}/logs/**/*.parquet`;
  const tracesPath = `s3://${bucketName}/traces/**/*.parquet`;

  // Escape service name for SQL
  const escapedService = service.replace(/'/g, "''");

  // Validate and build additional filter clause
  const validatedClause = whereClause ? validateWhereClause(whereClause) : null;
  const additionalFilter = validatedClause ? ` AND (${validatedClause})` : '';

  return `
    SELECT 'LOG' as type, timestamp_ms, body as message, severity_text
    FROM read_parquet('${logsPath}')
    WHERE service = '${escapedService}'
      AND timestamp_ms BETWEEN ${from} AND ${to}
      ${additionalFilter}
    UNION ALL
    SELECT 'SPAN' as type, timestamp_ms, name as message, status_code::VARCHAR as severity_text
    FROM read_parquet('${tracesPath}')
    WHERE service = '${escapedService}'
      AND timestamp_ms BETWEEN ${from} AND ${to}
      ${additionalFilter}
    ORDER BY timestamp_ms DESC
    LIMIT ${limit}
  `;
}

/**
 * Generate a mock query result for testing/demo purposes.
 * Used when DuckDB connection is not available.
 */
export function generateMockResult(
  _service: string,
  from: Date,
  to: Date
): QueryResult {
  const columns = ['type', 'timestamp_ms', 'message', 'severity_text'];
  const rows: Record<string, unknown>[] = [];

  // Generate mock log entries
  const logMessages = [
    'Request received from client',
    'Connection established',
    'Processing payment transaction',
    'Database query executed',
    'Response sent successfully',
    'Connection timeout occurred',
    'Retry attempt 1 of 3',
    'Cache miss, fetching from origin',
  ];

  const errorMessages = [
    'Connection refused: ECONNREFUSED',
    'Timeout waiting for response',
    'Invalid authentication token',
    'Rate limit exceeded',
  ];

  const spanNames = [
    'POST /api/checkout',
    'GET /api/products',
    'POST /api/auth/login',
    'GET /api/users/:id',
    'PUT /api/cart/items',
  ];

  const startMs = from.getTime();
  const endMs = to.getTime();
  const range = endMs - startMs;

  // Generate 10-20 mock records
  const count = 10 + Math.floor(Math.random() * 11);

  for (let i = 0; i < count; i++) {
    const timestampMs = startMs + Math.floor(Math.random() * range);
    const isError = Math.random() < 0.2; // 20% chance of error

    if (Math.random() < 0.6) {
      // 60% logs
      rows.push({
        type: 'LOG',
        timestamp_ms: BigInt(timestampMs),
        message: isError
          ? errorMessages[Math.floor(Math.random() * errorMessages.length)]
          : logMessages[Math.floor(Math.random() * logMessages.length)],
        severity_text: isError ? 'ERROR' : 'INFO',
      });
    } else {
      // 40% spans
      const statusCode = isError ? '2' : '0';
      rows.push({
        type: 'SPAN',
        timestamp_ms: BigInt(timestampMs),
        message: spanNames[Math.floor(Math.random() * spanNames.length)],
        severity_text: statusCode,
      });
    }
  }

  // Sort by timestamp descending
  rows.sort((a, b) => {
    const aTs = Number(a.timestamp_ms as bigint);
    const bTs = Number(b.timestamp_ms as bigint);
    return bTs - aTs;
  });

  return { columns, rows };
}
