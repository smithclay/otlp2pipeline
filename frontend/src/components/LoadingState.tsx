/**
 * Shared loading and error state components.
 */

/**
 * Minimal snowflake spinner for loading states.
 * True 6-fold hexagonal symmetry.
 */
export function LoadingSpinner() {
  return (
    <div className="flex items-center justify-center py-12">
      <svg
        className="h-8 w-8 animate-spin"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        style={{ color: 'var(--color-accent)' }}
      >
        {/* 6 arms at 60° intervals from center */}
        <line x1="12" y1="12" x2="12" y2="3" />
        <line x1="12" y1="12" x2="19.79" y2="7.5" />
        <line x1="12" y1="12" x2="19.79" y2="16.5" />
        <line x1="12" y1="12" x2="12" y2="21" />
        <line x1="12" y1="12" x2="4.21" y2="16.5" />
        <line x1="12" y1="12" x2="4.21" y2="7.5" />
        {/* Small V branches near tips */}
        <path d="M12 3L10 5.5M12 3l2 2.5" />
        <path d="M12 21l-2-2.5M12 21l2-2.5" />
        <path d="M19.79 7.5l-2.5 1M19.79 7.5l-1.5 2.2" />
        <path d="M4.21 16.5l2.5-1M4.21 16.5l1.5-2.2" />
        <path d="M19.79 16.5l-2.5-1M19.79 16.5l-1.5-2.2" />
        <path d="M4.21 7.5l2.5 1M4.21 7.5l1.5 2.2" />
      </svg>
    </div>
  );
}

export interface ErrorMessageProps {
  message: string;
  onRetry: () => void;
}

/**
 * Error message component with retry button.
 */
export function ErrorMessage({ message, onRetry }: ErrorMessageProps) {
  return (
    <div
      className="rounded-lg p-4"
      style={{
        backgroundColor: 'var(--color-error-bg)',
        border: '1px solid var(--color-error)',
      }}
    >
      <p style={{ color: 'var(--color-error)' }}>{message}</p>
      <button
        type="button"
        onClick={onRetry}
        className="mt-3 rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
        style={{
          backgroundColor: 'var(--color-error)',
          color: 'white',
        }}
      >
        Retry
      </button>
    </div>
  );
}
