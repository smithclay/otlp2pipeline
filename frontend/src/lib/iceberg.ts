/**
 * Iceberg REST Catalog API client.
 *
 * Low-level functions for interacting with the Iceberg REST Catalog API.
 * Used by useCatalogStats hook to fetch catalog metadata for the Settings page.
 *
 * See: https://github.com/apache/iceberg/blob/main/open-api/rest-catalog-open-api.yaml
 */

/**
 * Table identifier in the catalog.
 */
export interface TableIdentifier {
  namespace: string[];
  name: string;
}

/**
 * Snapshot summary information.
 */
export interface SnapshotSummary {
  operation?: string;
  'added-data-files'?: string;
  'added-records'?: string;
  'added-files-size'?: string;
  'total-data-files'?: string;
  'total-records'?: string;
  'total-files-size'?: string;
  [key: string]: string | undefined;
}

/**
 * Iceberg table snapshot.
 */
export interface Snapshot {
  'snapshot-id': number;
  'timestamp-ms': number;
  'manifest-list'?: string;
  summary?: SnapshotSummary;
  'sequence-number'?: number;
  'schema-id'?: number;
}

/**
 * Iceberg table metadata.
 * Contains the essential fields needed for catalog stats.
 */
export interface TableMetadata {
  'format-version': number;
  'table-uuid': string;
  location: string;
  'last-updated-ms'?: number;
  'current-snapshot-id'?: number;
  snapshots?: Snapshot[];
}

/**
 * Response from load table endpoint.
 */
export interface LoadTableResponse {
  metadata: TableMetadata;
  'metadata-location'?: string;
  config?: Record<string, string>;
}

/**
 * Response from list namespaces endpoint.
 */
interface ListNamespacesResponse {
  namespaces: string[][];
  'next-page-token'?: string;
}

/**
 * Response from list tables endpoint.
 */
interface ListTablesResponse {
  identifiers: TableIdentifier[];
  'next-page-token'?: string;
}

/**
 * Type guard for namespace arrays.
 * Validates that each namespace is an array of strings.
 */
function isNamespaceArray(obj: unknown): obj is string[][] {
  if (!Array.isArray(obj)) return false;
  return obj.every(
    (ns) => Array.isArray(ns) && ns.every((n) => typeof n === 'string')
  );
}

/**
 * Type guard for TableIdentifier objects.
 */
function isTableIdentifier(obj: unknown): obj is TableIdentifier {
  if (typeof obj !== 'object' || obj === null) return false;
  const t = obj as Record<string, unknown>;
  return (
    Array.isArray(t.namespace) &&
    t.namespace.every((n) => typeof n === 'string') &&
    typeof t.name === 'string'
  );
}

/**
 * Build the catalog base URL from worker URL.
 * The worker proxies Iceberg REST Catalog requests at /v1/iceberg.
 */
function buildCatalogUrl(workerUrl: string): string {
  // Remove trailing slash if present
  const baseUrl = workerUrl.replace(/\/$/, '');
  return `${baseUrl}/v1/iceberg`;
}

/**
 * Make an authenticated request to the Iceberg REST Catalog.
 */
async function catalogFetch<T>(
  url: string,
  r2Token: string
): Promise<T> {
  const response = await fetch(url, {
    headers: {
      Authorization: `Bearer ${r2Token}`,
      Accept: 'application/json',
    },
  });

  if (!response.ok) {
    const text = await response.text().catch(() => '');
    throw new Error(
      `Iceberg catalog request failed: ${response.status} ${response.statusText}${text ? ` - ${text}` : ''}`
    );
  }

  return response.json();
}

/**
 * List all namespaces in the catalog.
 *
 * @param workerUrl - Worker URL (catalog proxy)
 * @param warehouse - Warehouse identifier (accountId_bucketName)
 * @param r2Token - R2 API token for authentication
 * @returns Array of namespace arrays (e.g., [["logs"], ["traces"]])
 */
export async function listNamespaces(
  workerUrl: string,
  warehouse: string,
  r2Token: string
): Promise<string[][]> {
  const catalogUrl = buildCatalogUrl(workerUrl);
  const url = `${catalogUrl}/v1/${encodeURIComponent(warehouse)}/namespaces`;

  const data = await catalogFetch<ListNamespacesResponse>(url, r2Token);

  // Warn about pagination - not currently implemented
  if (data['next-page-token']) {
    console.warn('Pagination detected but not implemented. Results may be incomplete.');
  }

  if (!data.namespaces || !Array.isArray(data.namespaces)) {
    console.error('Invalid namespaces response:', data);
    throw new Error('API returned data in unexpected format. Check API version compatibility.');
  }

  // Validate each namespace is an array of strings
  if (!isNamespaceArray(data.namespaces)) {
    console.error('Invalid namespace format in response:', data.namespaces);
    throw new Error('API returned data in unexpected format. Check API version compatibility.');
  }

  return data.namespaces;
}

/**
 * List all tables in a namespace.
 *
 * @param workerUrl - Worker URL (catalog proxy)
 * @param warehouse - Warehouse identifier (accountId_bucketName)
 * @param r2Token - R2 API token for authentication
 * @param namespace - Namespace to list tables from
 * @returns Array of table identifiers
 */
export async function listTables(
  workerUrl: string,
  warehouse: string,
  r2Token: string,
  namespace: string
): Promise<TableIdentifier[]> {
  const catalogUrl = buildCatalogUrl(workerUrl);
  const url = `${catalogUrl}/v1/${encodeURIComponent(warehouse)}/namespaces/${encodeURIComponent(namespace)}/tables`;

  const data = await catalogFetch<ListTablesResponse>(url, r2Token);

  // Warn about pagination - not currently implemented
  if (data['next-page-token']) {
    console.warn('Pagination detected but not implemented. Results may be incomplete.');
  }

  if (!data.identifiers || !Array.isArray(data.identifiers)) {
    console.error('Invalid tables response:', data);
    throw new Error('API returned data in unexpected format. Check API version compatibility.');
  }

  // Validate and filter table identifiers
  const validTables: TableIdentifier[] = [];
  const invalidIndices: number[] = [];

  for (let i = 0; i < data.identifiers.length; i++) {
    if (isTableIdentifier(data.identifiers[i])) {
      validTables.push(data.identifiers[i]);
    } else {
      console.warn('Invalid table identifier at index', i, ':', data.identifiers[i]);
      invalidIndices.push(i);
    }
  }

  // If ALL items were invalid, this indicates an API compatibility issue
  if (data.identifiers.length > 0 && validTables.length === 0) {
    console.error('All table identifiers failed validation:', data.identifiers);
    throw new Error('API returned data in unexpected format. Check API version compatibility.');
  }

  // Log prominently if significant portion dropped
  if (invalidIndices.length > 0) {
    console.error(`Dropped ${invalidIndices.length} of ${data.identifiers.length} table identifiers due to validation failure`);
  }

  return validTables;
}

/**
 * Load full table metadata.
 *
 * @param workerUrl - Worker URL (catalog proxy)
 * @param warehouse - Warehouse identifier (accountId_bucketName)
 * @param r2Token - R2 API token for authentication
 * @param namespace - Namespace containing the table
 * @param tableName - Name of the table
 * @returns Table metadata including snapshots
 */
export async function loadTable(
  workerUrl: string,
  warehouse: string,
  r2Token: string,
  namespace: string,
  tableName: string
): Promise<LoadTableResponse> {
  const catalogUrl = buildCatalogUrl(workerUrl);
  const url = `${catalogUrl}/v1/${encodeURIComponent(warehouse)}/namespaces/${encodeURIComponent(namespace)}/tables/${encodeURIComponent(tableName)}`;

  return catalogFetch<LoadTableResponse>(url, r2Token);
}
