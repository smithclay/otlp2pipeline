/**
 * Shared loading and error state components.
 */

/**
 * Spinner component for loading states.
 */
export function LoadingSpinner() {
  return (
    <div className="flex items-center justify-center py-12">
      <div className="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-cyan-500" />
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
    <div className="rounded-lg border border-red-900 bg-red-950 p-4">
      <p className="text-red-400">{message}</p>
      <button
        type="button"
        onClick={onRetry}
        className="mt-3 rounded-md bg-red-900 px-3 py-1.5 text-sm text-red-200 hover:bg-red-800 transition-colors"
      >
        Retry
      </button>
    </div>
  );
}
