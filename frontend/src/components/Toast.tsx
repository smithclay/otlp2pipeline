import { createContext, useContext, useState, useCallback, useEffect, type ReactNode } from 'react';
import { Link } from 'react-router-dom';

interface Toast {
  id: string;
  message: string;
  type: 'info' | 'error' | 'success';
  action?: {
    label: string;
    to: string;
  };
}

interface ToastContextValue {
  showToast: (toast: Omit<Toast, 'id'>) => void;
  dismissToast: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast(): ToastContextValue {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider');
  }
  return context;
}

interface ToastProviderProps {
  children: ReactNode;
}

export function ToastProvider({ children }: ToastProviderProps) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const showToast = useCallback((toast: Omit<Toast, 'id'>) => {
    const id = `toast-${Date.now()}`;
    setToasts((prev) => [...prev, { ...toast, id }]);
  }, []);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ showToast, dismissToast }}>
      {children}
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />
    </ToastContext.Provider>
  );
}

interface ToastContainerProps {
  toasts: Toast[];
  onDismiss: (id: string) => void;
}

function ToastContainer({ toasts, onDismiss }: ToastContainerProps) {
  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </div>
  );
}

interface ToastItemProps {
  toast: Toast;
  onDismiss: (id: string) => void;
}

function ToastItem({ toast, onDismiss }: ToastItemProps) {
  // Auto-dismiss after 8 seconds (longer for actionable toasts)
  useEffect(() => {
    const timeout = setTimeout(() => {
      onDismiss(toast.id);
    }, toast.action ? 10000 : 6000);
    return () => clearTimeout(timeout);
  }, [toast.id, toast.action, onDismiss]);

  const bgColor = {
    info: 'var(--color-paper)',
    error: 'var(--color-error-bg)',
    success: 'var(--color-healthy-bg)',
  }[toast.type];

  const borderColor = {
    info: 'var(--color-border)',
    error: 'var(--color-error)',
    success: 'var(--color-healthy)',
  }[toast.type];

  const iconColor = {
    info: 'var(--color-accent)',
    error: 'var(--color-error)',
    success: 'var(--color-healthy)',
  }[toast.type];

  return (
    <div
      className="flex items-start gap-3 rounded-lg p-4 shadow-lg animate-slide-in max-w-sm"
      style={{
        backgroundColor: bgColor,
        border: `1px solid ${borderColor}`,
      }}
      role="alert"
    >
      {/* Icon */}
      <div className="flex-shrink-0 mt-0.5" style={{ color: iconColor }}>
        {toast.type === 'error' ? (
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        ) : toast.type === 'success' ? (
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        ) : (
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0">
        <p className="text-sm" style={{ color: 'var(--color-text-primary)' }}>
          {toast.message}
        </p>
        {toast.action && (
          <Link
            to={toast.action.to}
            onClick={() => onDismiss(toast.id)}
            className="inline-block mt-2 text-sm font-medium hover:underline"
            style={{ color: 'var(--color-accent)' }}
          >
            {toast.action.label} →
          </Link>
        )}
      </div>

      {/* Dismiss button */}
      <button
        onClick={() => onDismiss(toast.id)}
        className="flex-shrink-0 p-1 rounded hover:bg-black/5 transition-colors"
        style={{ color: 'var(--color-text-muted)' }}
        aria-label="Dismiss"
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}
