/**
 * ErrorAlert Component
 *
 * A simple error message display with consistent styling.
 */

export interface ErrorAlertProps {
  message: string;
  mono?: boolean;
}

export function ErrorAlert({ message, mono = true }: ErrorAlertProps): JSX.Element {
  return (
    <div
      className="rounded-lg p-4"
      style={{
        backgroundColor: 'var(--color-error-bg)',
        border: '1px solid var(--color-error)',
      }}
    >
      <p
        className={mono ? 'font-mono text-sm' : 'text-sm'}
        style={{ color: 'var(--color-error)' }}
      >
        {message}
      </p>
    </div>
  );
}
