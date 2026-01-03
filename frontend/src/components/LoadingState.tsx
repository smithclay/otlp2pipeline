/**
 * Shared loading and error state components.
 */

/**
 * Spinner component for loading states.
 */
export function LoadingSpinner() {
  return (
    <div className="flex items-center justify-center py-12">
      <div
        className="h-8 w-8 animate-spin rounded-full border-2"
        style={{
          borderColor: 'var(--color-border)',
          borderTopColor: 'var(--color-accent)',
        }}
      />
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
