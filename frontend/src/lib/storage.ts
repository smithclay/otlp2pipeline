/**
 * localStorage wrapper for credential storage
 */

const STORAGE_KEY = 'frostbit:credentials';

export interface StoredCredentials {
  workerUrl: string;
  r2Token: string;
  bucketName: string;
}

export function getStoredCredentials(): StoredCredentials | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return null;

    const parsed = JSON.parse(stored);

    // Validate shape
    if (
      typeof parsed.workerUrl === 'string' &&
      typeof parsed.r2Token === 'string' &&
      typeof parsed.bucketName === 'string'
    ) {
      return parsed as StoredCredentials;
    }

    return null;
  } catch {
    return null;
  }
}

export function setStoredCredentials(credentials: StoredCredentials): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(credentials));
}

export function clearStoredCredentials(): void {
  localStorage.removeItem(STORAGE_KEY);
}
