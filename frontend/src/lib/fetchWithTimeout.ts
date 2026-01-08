/**
 * Fetch utilities with timeout support.
 */

/** Default timeout for API requests (5 minutes) */
export const DEFAULT_TIMEOUT_MS = 300000;

/**
 * Fetch with timeout and abort support.
 * @param url - URL to fetch
 * @param options - Fetch options
 * @param timeoutMs - Timeout in milliseconds (default: 300000)
 * @returns Response from fetch
 * @throws Error if request times out or fails
 */
export async function fetchWithTimeout(
  url: string,
  options: RequestInit = {},
  timeoutMs: number = DEFAULT_TIMEOUT_MS
): Promise<Response> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    return response;
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error(`Request timed out after ${timeoutMs}ms`);
    }
    throw error;
  } finally {
    clearTimeout(timeoutId);
  }
}

/**
 * Config response from worker's /v1/config endpoint.
 */
export interface WorkerConfig {
  accountId: string | null;
  bucketName: string | null;
  icebergProxyEnabled: boolean;
}

/**
 * Fetch worker config from /v1/config endpoint.
 */
export async function fetchWorkerConfig(workerUrl: string): Promise<WorkerConfig> {
  const response = await fetchWithTimeout(`${workerUrl}/v1/config`);
  if (!response.ok) {
    throw new Error(`Failed to fetch worker config: ${response.status}`);
  }
  return response.json();
}
