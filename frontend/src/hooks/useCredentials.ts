import { useState, useCallback, useMemo } from 'react';
import {
  getStoredCredentials,
  setStoredCredentials,
  clearStoredCredentials,
} from '../lib/storage';

export interface Credentials {
  workerUrl: string;
  r2Token: string;
  bucketName: string;
}

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

  const isConfigured = useMemo(
    () =>
      credentials !== null &&
      credentials.workerUrl.length > 0 &&
      credentials.r2Token.length > 0 &&
      credentials.bucketName.length > 0,
    [credentials]
  );

  return {
    credentials,
    setCredentials,
    clearCredentials,
    isConfigured,
  };
}
