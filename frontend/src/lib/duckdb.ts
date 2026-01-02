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

      // Create worker and logger
      const worker = new Worker(bundle.mainWorker);
      const logger = new duckdb.ConsoleLogger(duckdb.LogLevel.WARNING);

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
 * R2/S3 connection configuration.
 */
export interface R2Config {
  bucketName: string;
  r2Token: string;
  accountId?: string;
  endpoint?: string;
}

/**
 * Connection status including extension availability.
 */
export interface ConnectionStatus {
  connection: duckdb.AsyncDuckDBConnection;
  httpfsAvailable: boolean;
  s3Configured: boolean;
  warnings: string[];
}

/**
 * Create a connection configured for R2/Iceberg access.
 *
 * Note: DuckDB WASM's S3/Iceberg support has limitations.
 * The connection is set up with basic S3 configuration but actual
 * R2 connectivity may require additional configuration or may not
 * be fully supported in the browser environment.
 *
 * @throws Error if S3 configuration fails (required for R2 access)
 */
export async function connectToR2(
  database: duckdb.AsyncDuckDB,
  config: R2Config
): Promise<ConnectionStatus> {
  const conn = await database.connect();
  const warnings: string[] = [];
  let httpfsAvailable = false;

  // Attempt to load httpfs extension for remote file access
  // In WASM builds, httpfs may already be bundled or may not be available
  try {
    await conn.query('INSTALL httpfs;');
    await conn.query('LOAD httpfs;');
    httpfsAvailable = true;
  } catch (error) {
    console.error('Failed to load httpfs extension:', error);
    warnings.push('httpfs extension not available - R2/S3 queries may fail');
  }

  // Configure S3 credentials for R2
  // R2 uses S3-compatible API with Cloudflare-specific endpoint
  const endpoint = config.endpoint ?? `https://${config.accountId}.r2.cloudflarestorage.com`;

  try {
    await conn.query(`
      SET s3_region = 'auto';
      SET s3_endpoint = '${endpoint}';
      SET s3_access_key_id = '${config.r2Token}';
      SET s3_use_ssl = true;
    `);
  } catch (error) {
    console.error('Failed to configure R2/S3 settings:', error);
    // Close the connection since it's not usable without S3 config
    await conn.close();
    throw new Error(
      'Failed to configure R2 connection. Check your credentials and try again.'
    );
  }

  return {
    connection: conn,
    httpfsAvailable,
    s3Configured: true,
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

/**
 * Build an R2 parquet path for a given table.
 */
export function buildR2Path(bucketName: string, tableName: string): string {
  return `s3://${bucketName}/${tableName}/**/*.parquet`;
}
