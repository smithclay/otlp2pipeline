import { useState, FormEvent } from 'react';
import { Credentials } from '../hooks/useCredentials';

interface SetupModalProps {
  onSave: (credentials: Credentials) => void;
  onClose?: () => void;
  initialValues?: Credentials;
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

export function SetupModal({ onSave, onClose, initialValues }: SetupModalProps) {
  const [workerUrl, setWorkerUrl] = useState(initialValues?.workerUrl ?? '');
  const [r2Token, setR2Token] = useState(initialValues?.r2Token ?? '');
  const [bucketName, setBucketName] = useState(initialValues?.bucketName ?? '');
  const [accountId, setAccountId] = useState(initialValues?.accountId ?? '');
  const [status, setStatus] = useState<ConnectionStatus>('idle');
  const [errorMessage, setErrorMessage] = useState('');

  const isFormValid =
    workerUrl.trim().length > 0 &&
    r2Token.trim().length > 0 &&
    bucketName.trim().length > 0 &&
    accountId.trim().length > 0;

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
        bucketName: bucketName.trim(),
        accountId: accountId.trim(),
      });
    }, 500);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/80 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-lg bg-slate-800 p-8 shadow-xl relative">
        {/* Close button (only when editing existing settings) */}
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            className="absolute top-4 right-4 rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-slate-100 transition-colors"
            aria-label="Close settings"
          >
            <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        )}

        {/* Logo */}
        <div className="mb-6 text-center">
          <span className="text-2xl font-semibold text-cyan-500">frostbit</span>
        </div>

        {/* Headline */}
        <h1 className="mb-6 text-center text-xl font-medium text-slate-100">
          {onClose ? 'Update Settings' : 'Connect to your Cloudflare environment'}
        </h1>

        {/* Form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Worker URL */}
          <div>
            <label
              htmlFor="workerUrl"
              className="mb-1 block text-sm font-medium text-slate-300"
            >
              Worker URL
            </label>
            <input
              id="workerUrl"
              type="url"
              value={workerUrl}
              onChange={(e) => setWorkerUrl(e.target.value)}
              placeholder="https://frostbit.example.workers.dev"
              className="w-full rounded-md border border-slate-600 bg-slate-700 px-3 py-2 text-slate-100 placeholder-slate-400 focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500"
              required
            />
          </div>

          {/* Account ID */}
          <div>
            <label
              htmlFor="accountId"
              className="mb-1 block text-sm font-medium text-slate-300"
            >
              Cloudflare Account ID
            </label>
            <input
              id="accountId"
              type="text"
              value={accountId}
              onChange={(e) => setAccountId(e.target.value)}
              placeholder="e.g. 1a2b3c4d5e6f7g8h9i0j"
              className="w-full rounded-md border border-slate-600 bg-slate-700 px-3 py-2 text-slate-100 placeholder-slate-400 focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500"
              required
            />
          </div>

          {/* R2 API Token */}
          <div>
            <label
              htmlFor="r2Token"
              className="mb-1 block text-sm font-medium text-slate-300"
            >
              R2 API Token
            </label>
            <input
              id="r2Token"
              type="password"
              value={r2Token}
              onChange={(e) => setR2Token(e.target.value)}
              placeholder="Enter your R2 API token"
              className="w-full rounded-md border border-slate-600 bg-slate-700 px-3 py-2 text-slate-100 placeholder-slate-400 focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500"
              required
            />
          </div>

          {/* Bucket Name */}
          <div>
            <label
              htmlFor="bucketName"
              className="mb-1 block text-sm font-medium text-slate-300"
            >
              Bucket Name
            </label>
            <input
              id="bucketName"
              type="text"
              value={bucketName}
              onChange={(e) => setBucketName(e.target.value)}
              placeholder="frostbit-prod"
              className="w-full rounded-md border border-slate-600 bg-slate-700 px-3 py-2 text-slate-100 placeholder-slate-400 focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500"
              required
            />
          </div>

          {/* Error Message */}
          {status === 'error' && (
            <div className="rounded-md border border-red-500/50 bg-red-500/10 p-3 text-sm text-red-400">
              {errorMessage}
            </div>
          )}

          {/* Success Message */}
          {status === 'success' && (
            <div className="rounded-md border border-green-500/50 bg-green-500/10 p-3 text-sm text-green-400">
              Connection successful!
            </div>
          )}

          {/* Submit Button */}
          <button
            type="submit"
            disabled={!isFormValid || status === 'testing'}
            className="w-full rounded-md bg-cyan-600 px-4 py-2 font-medium text-white transition-colors hover:bg-cyan-500 focus:outline-none focus:ring-2 focus:ring-cyan-500 focus:ring-offset-2 focus:ring-offset-slate-800 disabled:cursor-not-allowed disabled:opacity-50"
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
            ) : onClose ? (
              'Save Settings'
            ) : (
              'Connect'
            )}
          </button>
        </form>
      </div>
    </div>
  );
}
