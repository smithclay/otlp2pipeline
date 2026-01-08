import { useState, useCallback } from 'react';
import {
  Credentials,
  getStoredCredentials,
  setStoredCredentials,
  clearStoredCredentials,
} from '../lib/storage';

// Re-export Credentials type for consumers
export type { Credentials };

export interface UseCredentialsResult {
  credentials: Credentials | null;
  setCredentials: (creds: Credentials) => void;
  clearCredentials: () => void;
  isConfigured: boolean;
}

export function useCredentials(): UseCredentialsResult {
  const [credentials, setCredentialsState] = useState<Credentials | null>(() =>
    getStoredCredentials()
  );

  const setCredentials = useCallback((creds: Credentials) => {
    setStoredCredentials(creds);
    setCredentialsState(creds);
  }, []);

  const clearCredentials = useCallback(() => {
    clearStoredCredentials();
    setCredentialsState(null);
  }, []);

  // Simple computation - no memoization needed
  // Only workerUrl and r2Token are required - accountId/bucketName come from /v1/config
  const isConfigured =
    credentials !== null &&
    credentials.workerUrl.length > 0 &&
    credentials.r2Token.length > 0;

  return {
    credentials,
    setCredentials,
    clearCredentials,
    isConfigured,
  };
}
