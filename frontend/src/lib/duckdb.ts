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
 */
export interface R2Config {
  bucketName: string;
  r2Token: string;
  accountId: string;
  /** Worker URL for proxying R2 Data Catalog requests (CORS workaround) */
  workerUrl?: string;
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

  // Load required extensions for R2 Data Catalog access
  try {
    await conn.query('INSTALL httpfs;');
    await conn.query('LOAD httpfs;');
    await conn.query('INSTALL iceberg;');
    await conn.query('LOAD iceberg;');
    icebergAvailable = true;
  } catch (error) {
    console.error('Failed to load extensions:', error);
    warnings.push('Required extensions not available - R2 queries may fail');
  }

  // Create Iceberg secret with R2 token
  // See: https://duckdb.org/2025/12/16/iceberg-in-the-browser
  try {
    const escapedToken = escapeSqlString(config.r2Token);
    await conn.query(`
      CREATE SECRET r2_secret (
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
  // If workerUrl is provided, use it as a CORS proxy, otherwise try direct access
  const escapedAccountId = escapeSqlString(config.accountId);
  const escapedBucketName = escapeSqlString(config.bucketName);
  const warehouse = `${escapedAccountId}_${escapedBucketName}`;
  const catalogEndpoint = config.workerUrl
    ? `${escapeSqlString(config.workerUrl)}/v1/iceberg`
    : `https://catalog.cloudflarestorage.com/${escapedAccountId}/${escapedBucketName}`;
  try {
    await conn.query(`
      ATTACH '${warehouse}' AS r2_catalog (
        TYPE ICEBERG,
        ENDPOINT '${catalogEndpoint}'
      );
    `);
    catalogAttached = true;
  } catch (error) {
    // Catalog attachment failure means queries won't work - fail explicitly
    console.error('Failed to attach R2 catalog:', error);
    await conn.close();
    const message = error instanceof Error ? error.message : 'Unknown error';
    if (message.includes('forbidden') || message.includes('401') || message.includes('403')) {
      throw new Error('R2 token does not have permission to access the catalog. Check token permissions.');
    } else if (message.includes('not found') || message.includes('404')) {
      throw new Error('R2 Data Catalog not found. Verify your Account ID and Bucket Name.');
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
  const result = await conn.query(sql);
  const columns = result.schema.fields.map((f) => f.name);

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

  return { columns, rows };
}
