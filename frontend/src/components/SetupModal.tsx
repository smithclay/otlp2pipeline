import { Credentials } from '../hooks/useCredentials';
import { ConnectionForm } from './ConnectionForm';

interface SetupModalProps {
  onSave: (credentials: Credentials) => void;
  onClose?: () => void;
  initialValues?: Credentials;
}

export function SetupModal({ onSave, onClose, initialValues }: SetupModalProps) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center backdrop-blur-sm"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.4)' }}
    >
      <div
        className="w-full max-w-md rounded-lg p-8 relative"
        style={{
          backgroundColor: 'white',
          boxShadow: 'var(--shadow-lg)',
        }}
      >
        {/* Close button (only when editing existing settings) */}
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            className="absolute top-4 right-4 rounded-full p-1.5 transition-colors"
            style={{ color: 'var(--color-text-muted)' }}
            onMouseOver={(e) => e.currentTarget.style.backgroundColor = 'rgba(0,0,0,0.05)'}
            onMouseOut={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
            aria-label="Close settings"
          >
            <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        )}

        {/* Logo */}
        <div className="mb-6 text-center">
          <span className="headline text-2xl" style={{ color: 'var(--color-text-primary)' }}>
            frostbit
          </span>
        </div>

        {/* Headline */}
        <h1
          className="mb-6 text-center text-lg font-medium"
          style={{ color: 'var(--color-text-secondary)' }}
        >
          {onClose ? 'Update Settings' : 'Connect to your Cloudflare environment'}
        </h1>

        {/* Connection Form */}
        <ConnectionForm
          onSave={onSave}
          initialValues={initialValues}
          submitLabel={onClose ? 'Save Settings' : 'Connect'}
        />
      </div>
    </div>
  );
}
