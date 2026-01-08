import { useState, FormEvent } from 'react';
import { Credentials } from '../hooks/useCredentials';

export interface ConnectionFormProps {
  onSave: (credentials: Credentials) => void;
  initialValues?: Credentials;
  /** Label for the submit button. Defaults to 'Connect' */
  submitLabel?: string;
}

type ConnectionStatus = 'idle' | 'testing' | 'success' | 'error';

/**
 * Categorized connection test result.
 */
interface ConnectionTestResult {
  success: boolean;
  errorType?: 'network' | 'cors' | 'server' | 'timeout' | 'unknown';
  statusCode?: number;
  message: string;
}

/**
 * Test connection to worker and return categorized result.
 */
async function testConnection(url: string): Promise<ConnectionTestResult> {
  // Normalize URL - remove trailing slash
  const normalizedUrl = url.replace(/\/+$/, '');

  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 10000);

    const response = await fetch(`${normalizedUrl}/v1/services`, {
      method: 'GET',
      headers: {
        Accept: 'application/json',
      },
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    if (response.ok) {
      return { success: true, message: 'Connection successful!' };
    }

    // Server responded but with an error status
    if (response.status >= 500) {
      return {
        success: false,
        errorType: 'server',
        statusCode: response.status,
        message: `Server error (${response.status}). The worker may be misconfigured or experiencing issues.`,
      };
    }

    if (response.status === 401 || response.status === 403) {
      return {
        success: false,
        errorType: 'server',
        statusCode: response.status,
        message: `Authentication failed (${response.status}). Check your credentials.`,
      };
    }

    if (response.status === 404) {
      return {
        success: false,
        errorType: 'server',
        statusCode: response.status,
        message: 'Endpoint not found (404). Check that the worker URL is correct and the /v1/services endpoint exists.',
      };
    }

    return {
      success: false,
      errorType: 'server',
      statusCode: response.status,
      message: `Unexpected response (${response.status}). Check the worker URL and configuration.`,
    };
  } catch (error) {
    // Categorize the error
    if (error instanceof Error) {
      if (error.name === 'AbortError') {
        return {
          success: false,
          errorType: 'timeout',
          message: 'Connection timed out. Check the URL and ensure the worker is running.',
        };
      }

      if (error.name === 'TypeError' && error.message.includes('Failed to fetch')) {
        // This is typically a CORS or network error
        return {
          success: false,
          errorType: 'cors',
          message: 'Could not connect. This may be a CORS issue - ensure the worker has CORS headers configured, or the URL may be unreachable.',
        };
      }
    }

    return {
      success: false,
      errorType: 'unknown',
      message: `Connection failed: ${error instanceof Error ? error.message : 'Unknown error'}`,
    };
  }
}

/**
 * Reusable connection form for configuring worker URL and R2 token.
 * Used by SetupModal and Settings page.
 */
export function ConnectionForm({ onSave, initialValues, submitLabel = 'Connect' }: ConnectionFormProps) {
  const [workerUrl, setWorkerUrl] = useState(initialValues?.workerUrl ?? '');
  const [r2Token, setR2Token] = useState(initialValues?.r2Token ?? '');
  const [status, setStatus] = useState<ConnectionStatus>('idle');
  const [errorMessage, setErrorMessage] = useState('');

  const isFormValid = workerUrl.trim().length > 0 && r2Token.trim().length > 0;

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();

    if (!isFormValid) return;

    setStatus('testing');
    setErrorMessage('');

    const normalizedUrl = workerUrl.trim().replace(/\/+$/, '');

    // Test connection to worker
    const result = await testConnection(normalizedUrl);

    if (!result.success) {
      setStatus('error');
      setErrorMessage(result.message);
      return;
    }

    // For now, DuckDB validation is deferred to Step 5
    // Just show success and save credentials
    setStatus('success');

    // Brief delay to show success state before dismissing
    setTimeout(() => {
      onSave({
        workerUrl: normalizedUrl,
        r2Token: r2Token.trim(),
      });
    }, 500);
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {/* Worker URL */}
      <div>
        <label
          htmlFor="workerUrl"
          className="mb-1.5 block text-sm font-medium"
          style={{ color: 'var(--color-text-secondary)' }}
        >
          Worker URL
        </label>
        <input
          id="workerUrl"
          type="url"
          value={workerUrl}
          onChange={(e) => setWorkerUrl(e.target.value)}
          placeholder="https://frostbit.example.workers.dev"
          className="w-full rounded-md px-3 py-2 text-sm"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-primary)',
          }}
          required
        />
      </div>

      {/* R2 API Token */}
      <div>
        <div className="flex items-center justify-between mb-1.5">
          <label
            htmlFor="r2Token"
            className="text-sm font-medium"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            R2 API Token
          </label>
          <a
            href="https://dash.cloudflare.com/?to=/:account/r2/api-tokens"
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs hover:underline"
            style={{ color: 'var(--color-accent)' }}
          >
            Get token →
          </a>
        </div>
        <input
          id="r2Token"
          type="password"
          value={r2Token}
          onChange={(e) => setR2Token(e.target.value)}
          placeholder="Enter your R2 API token"
          className="w-full rounded-md px-3 py-2 text-sm"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-primary)',
          }}
          required
        />
        <p
          className="mt-1.5 text-xs"
          style={{ color: 'var(--color-text-muted)' }}
        >
          Stored locally in your browser. Use Settings to clear.
        </p>
      </div>

      {/* Error Message */}
      {status === 'error' && (
        <div
          className="rounded-md p-3 text-sm"
          style={{
            backgroundColor: 'var(--color-error-bg)',
            border: '1px solid var(--color-error)',
            color: 'var(--color-error)',
          }}
        >
          {errorMessage}
        </div>
      )}

      {/* Success Message */}
      {status === 'success' && (
        <div
          className="rounded-md p-3 text-sm"
          style={{
            backgroundColor: 'var(--color-healthy-bg)',
            border: '1px solid var(--color-healthy)',
            color: 'var(--color-healthy)',
          }}
        >
          Connection successful!
        </div>
      )}

      {/* Submit Button */}
      <button
        type="submit"
        disabled={!isFormValid || status === 'testing'}
        className="w-full rounded-md px-4 py-2.5 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        style={{ backgroundColor: 'var(--color-accent)' }}
      >
        {status === 'testing' ? (
          <span className="flex items-center justify-center gap-2">
            <svg
              className="h-4 w-4 animate-spin"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
              />
            </svg>
            Testing Connection...
          </span>
        ) : (
          submitLabel
        )}
      </button>
    </form>
  );
}
