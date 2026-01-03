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

  if (!data.namespaces || !Array.isArray(data.namespaces)) {
    console.warn('Unexpected namespaces response:', data);
    return [];
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

  if (!data.identifiers || !Array.isArray(data.identifiers)) {
    console.warn('Unexpected tables response:', data);
    return [];
  }

  return data.identifiers;
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
