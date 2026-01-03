/**
 * localStorage wrapper for credential storage.
 *
 * workerUrl and r2Token are stored locally. R2 catalog config (accountId, bucketName)
 * is fetched from the worker's /v1/config endpoint.
 *
 * The r2Token is needed client-side because DuckDB's Iceberg extension makes direct
 * requests to R2 for parquet data files (catalog requests go through the proxy).
 */

const STORAGE_KEY = 'frostbit:credentials';

export interface Credentials {
  workerUrl: string;
  r2Token: string;
}

export function getStoredCredentials(): Credentials | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return null;

    const parsed = JSON.parse(stored);

    // Validate shape
    if (
      typeof parsed.workerUrl === 'string' &&
      parsed.workerUrl.length > 0 &&
      typeof parsed.r2Token === 'string' &&
      parsed.r2Token.length > 0
    ) {
      return { workerUrl: parsed.workerUrl, r2Token: parsed.r2Token };
    }

    // Invalid shape - clear corrupted data
    console.warn('Invalid credentials shape in localStorage, clearing');
    localStorage.removeItem(STORAGE_KEY);
    return null;
  } catch (error) {
    console.error('Failed to parse stored credentials:', error);
    // Attempt to clear corrupted data
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch (clearError) {
      console.error('Failed to clear corrupted credentials:', clearError);
    }
    return null;
  }
}

export function setStoredCredentials(credentials: Credentials): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(credentials));
}

export function clearStoredCredentials(): void {
  localStorage.removeItem(STORAGE_KEY);
}
