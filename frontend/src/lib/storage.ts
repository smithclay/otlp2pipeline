/**
 * localStorage wrapper for credential storage
 */

const STORAGE_KEY = 'frostbit:credentials';

export interface Credentials {
  workerUrl: string;
  r2Token: string;
  bucketName: string;
  accountId: string;
}

/** @deprecated Use Credentials instead */
export type StoredCredentials = Credentials;

export function getStoredCredentials(): Credentials | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return null;

    const parsed = JSON.parse(stored);

    // Validate shape
    if (
      typeof parsed.workerUrl === 'string' &&
      typeof parsed.r2Token === 'string' &&
      typeof parsed.bucketName === 'string' &&
      typeof parsed.accountId === 'string'
    ) {
      return parsed as Credentials;
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
