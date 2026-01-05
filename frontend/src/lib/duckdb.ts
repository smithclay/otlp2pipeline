/**
 * DuckDB WASM initialization and Iceberg/R2 connection utilities.
 */
import * as duckdb from '@duckdb/duckdb-wasm';

let db: duckdb.AsyncDuckDB | null = null;
let initPromise: Promise<duckdb.AsyncDuckDB> | null = null;

/**
 * Initialize DuckDB WASM singleton.
 * Returns the same instance on subsequent calls.
 */
export async function initDuckDB(): Promise<duckdb.AsyncDuckDB> {
  // Return existing instance if available
  if (db) return db;

  // Prevent concurrent initialization
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      // Get the best available bundle for this browser
      const JSDELIVR_BUNDLES = duckdb.getJsDelivrBundles();
      const bundle = await duckdb.selectBundle(JSDELIVR_BUNDLES);

      if (!bundle.mainWorker) {
        throw new Error('DuckDB bundle does not include a worker');
      }

      // Create worker using Blob URL to avoid CORS issues with jsdelivr CDN
      // This wraps the remote worker script in a local blob that can load cross-origin scripts
      const workerUrl = URL.createObjectURL(
        new Blob([`importScripts("${bundle.mainWorker}");`], { type: 'text/javascript' })
      );
      const worker = new Worker(workerUrl);
      const logger = new duckdb.ConsoleLogger(duckdb.LogLevel.WARNING);

      // Clean up the blob URL after worker is created (it's already loaded)
      URL.revokeObjectURL(workerUrl);

      // Initialize DuckDB
      const instance = new duckdb.AsyncDuckDB(logger, worker);
      await instance.instantiate(bundle.mainModule, bundle.pthreadWorker);

      db = instance;
      return instance;
    } catch (error) {
      initPromise = null;
      throw error;
    }
  })();

  return initPromise;
}

/**
 * Get an existing DuckDB instance or null if not initialized.
 */
export function getDuckDB(): duckdb.AsyncDuckDB | null {
  return db;
}

/**
 * R2 Data Catalog connection configuration.
 * Only workerUrl and r2Token are required - accountId/bucketName are fetched from the worker.
 */
export interface R2Config {
  /** Worker URL for API and proxying R2 Data Catalog requests */
  workerUrl: string;
  /** R2 API token for direct data access (parquet files) */
  r2Token: string;
}

/**
 * Config response from worker's /v1/config endpoint.
 */
interface WorkerConfig {
  accountId: string | null;
  bucketName: string | null;
  icebergProxyEnabled: boolean;
}

/**
 * Fetch R2 catalog configuration from the worker.
 * @throws Error if the worker is not configured for R2 catalog access
 */
async function fetchWorkerConfig(workerUrl: string): Promise<WorkerConfig> {
  const response = await fetch(`${workerUrl}/v1/config`);
  if (!response.ok) {
    throw new Error(`Failed to fetch worker config: ${response.status}`);
  }
  return response.json();
}

/**
 * Escape single quotes for SQL string literals.
 * Prevents SQL injection when interpolating values into queries.
 */
function escapeSqlString(value: string): string {
  return value.replace(/'/g, "''");
}

/**
 * Connection status including extension availability.
 */
export interface ConnectionStatus {
  connection: duckdb.AsyncDuckDBConnection;
  icebergAvailable: boolean;
  catalogAttached: boolean;
  warnings: string[];
}

/**
 * Create a connection configured for R2 Data Catalog/Iceberg access.
 *
 * Uses DuckDB's Iceberg extension with Cloudflare R2 Data Catalog.
 * Fetches accountId/bucketName from the worker's /v1/config endpoint.
 * See: https://developers.cloudflare.com/r2/data-catalog/config-examples/duckdb/
 *
 * @throws Error if Iceberg configuration fails
 */
export async function connectToR2(
  database: duckdb.AsyncDuckDB,
  config: R2Config
): Promise<ConnectionStatus> {
  const conn = await database.connect();
  const warnings: string[] = [];
  let icebergAvailable = false;
  let catalogAttached = false;

  // Fetch R2 catalog config from worker
  let workerConfig: WorkerConfig;
  try {
    workerConfig = await fetchWorkerConfig(config.workerUrl);
  } catch (error) {
    await conn.close();
    throw new Error(
      `Failed to fetch config from worker. Ensure the worker is running and accessible.`
    );
  }

  if (!workerConfig.icebergProxyEnabled || !workerConfig.accountId || !workerConfig.bucketName) {
    await conn.close();
    throw new Error(
      'R2 Data Catalog is not configured on the worker. Set R2_CATALOG_ACCOUNT_ID and R2_CATALOG_BUCKET.'
    );
  }

  // Load required extensions for R2 Data Catalog access
  try {
    console.log('[DuckDB] Installing httpfs extension...');
    await conn.query('INSTALL httpfs;');
    await conn.query('LOAD httpfs;');
    console.log('[DuckDB] Installing iceberg extension...');
    await conn.query('INSTALL iceberg;');
    await conn.query('LOAD iceberg;');
    console.log('[DuckDB] Extensions loaded successfully');
    icebergAvailable = true;
  } catch (error) {
    console.error('[DuckDB] Failed to load extensions:', error);
    warnings.push('Required extensions not available - R2 queries may fail');
  }

  // Create Iceberg secret with R2 token
  // See: https://duckdb.org/2025/12/16/iceberg-in-the-browser
  // Use OR REPLACE to handle re-initialization (React StrictMode, navigation)
  try {
    const escapedToken = escapeSqlString(config.r2Token);
    await conn.query(`
      CREATE OR REPLACE SECRET r2_secret (
        TYPE ICEBERG,
        TOKEN '${escapedToken}'
      );
    `);
  } catch (error) {
    console.error('Failed to create Iceberg secret:', error);
    await conn.close();
    throw new Error('Failed to configure R2 credentials. Check your API token.');
  }

  // Attach R2 Data Catalog
  // Warehouse format is: <account_id>_<bucket_name>
  // Use worker as CORS proxy for catalog requests
  const escapedAccountId = escapeSqlString(workerConfig.accountId);
  const escapedBucketName = escapeSqlString(workerConfig.bucketName);
  const warehouse = `${escapedAccountId}_${escapedBucketName}`;
  const catalogEndpoint = `${escapeSqlString(config.workerUrl)}/v1/iceberg`;

  try {
    // Detach first if already attached (handles React StrictMode, navigation)
    console.log('[DuckDB] Attaching R2 catalog...');
    await conn.query(`DETACH DATABASE IF EXISTS r2_catalog;`);
    await conn.query(`
      ATTACH '${warehouse}' AS r2_catalog (
        TYPE ICEBERG,
        ENDPOINT '${catalogEndpoint}'
      );
    `);
    console.log('[DuckDB] R2 catalog attached successfully');
    catalogAttached = true;
  } catch (error) {
    // Catalog attachment failure means queries won't work - fail explicitly
    console.error('Failed to attach R2 catalog:', error);
    await conn.close();
    const message = error instanceof Error ? error.message : 'Unknown error';
    if (message.includes('forbidden') || message.includes('401') || message.includes('403')) {
      throw new Error('R2 token does not have permission to access the catalog. Check token permissions.');
    } else if (message.includes('not found') || message.includes('404')) {
      throw new Error('R2 Data Catalog not found. Verify the worker R2 configuration.');
    } else {
      throw new Error(`Failed to connect to R2 Data Catalog: ${message}`);
    }
  }

  return {
    connection: conn,
    icebergAvailable,
    catalogAttached,
    warnings,
  };
}

/**
 * Create a basic connection without R2 configuration.
 * Useful for querying local data or testing.
 */
export async function createConnection(
  database: duckdb.AsyncDuckDB
): Promise<duckdb.AsyncDuckDBConnection> {
  return database.connect();
}

/**
 * Query result type.
 */
export interface QueryResult {
  columns: string[];
  rows: Record<string, unknown>[];
}

/**
 * Execute a query and return results as an array of objects.
 */
export async function executeQuery(
  conn: duckdb.AsyncDuckDBConnection,
  sql: string
): Promise<QueryResult> {
  const startTime = performance.now();
  const queryPreview = sql.trim().slice(0, 100).replace(/\s+/g, ' ');
  console.log(`[DuckDB] Executing query: ${queryPreview}${sql.length > 100 ? '...' : ''}`);

  const result = await conn.query(sql);
  const queryTime = performance.now() - startTime;

  const columns = result.schema.fields.map((f) => f.name);
  console.log(`[DuckDB] Query completed in ${(queryTime / 1000).toFixed(2)}s, columns: [${columns.join(', ')}]`);

  // Convert Arrow table to array of objects
  const rows: Record<string, unknown>[] = [];
  const numRows = result.numRows;

  for (let i = 0; i < numRows; i++) {
    const row: Record<string, unknown> = {};
    for (const col of columns) {
      const column = result.getChild(col);
      row[col] = column?.get(i);
    }
    rows.push(row);
  }

  console.log(`[DuckDB] Processed ${rows.length} rows in ${((performance.now() - startTime) / 1000).toFixed(2)}s total`);

  return { columns, rows };
}
