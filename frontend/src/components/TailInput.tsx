import { motion, AnimatePresence } from 'framer-motion';

export type TailSignal = 'logs' | 'traces';

export interface TailInputProps {
  /** Selected service name */
  service: string;
  /** Selected signal type */
  signal: TailSignal;
  /** Whether currently streaming */
  isStreaming: boolean;
  /** Available services to choose from */
  services?: string[];
  /** Called when service selection changes */
  onServiceChange: (service: string) => void;
  /** Called when signal selection changes */
  onSignalChange: (signal: TailSignal) => void;
  /** Called when Start/Stop button is clicked */
  onStartStop: () => void;
  /** Number of records received during streaming */
  recordCount?: number;
  /** Number of dropped records */
  droppedCount?: number;
}

const DEFAULT_SERVICES = [
  'api-gateway',
  'auth-service',
  'payment-service',
  'user-service',
];

/**
 * TailInput component for configuring live streaming (tail) parameters.
 * Provides service selection, signal type toggle, and start/stop controls.
 */
export function TailInput({
  service,
  signal,
  isStreaming,
  services = DEFAULT_SERVICES,
  onServiceChange,
  onSignalChange,
  onStartStop,
  recordCount = 0,
  droppedCount = 0,
}: TailInputProps) {
  const canStart = service.length > 0;

  return (
    <div className="space-y-4">
      {/* Status indicator */}
      <div className="flex items-center justify-end gap-4 min-h-[24px]">
        {isStreaming && (
          <div className="flex items-center gap-2 text-sm">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-red-500" />
            </span>
            <span style={{ color: 'var(--color-text-secondary)' }}>
              Live · {recordCount} records
              {droppedCount > 0 && ` · ${droppedCount} dropped`}
            </span>
          </div>
        )}
      </div>

      {/* Input form */}
      <div
        className="rounded-lg p-5"
        style={{
          backgroundColor: 'white',
          border: '1px solid var(--color-border)',
          boxShadow: 'var(--shadow-sm)',
        }}
      >
        <div className="flex flex-col gap-4 sm:flex-row sm:items-end">
          {/* Service selection */}
          <div className="flex-1">
            <label
              className="data-label block mb-2"
              htmlFor="tail-service-select"
            >
              Service
            </label>
            <select
              id="tail-service-select"
              value={service}
              onChange={(e) => onServiceChange(e.target.value)}
              disabled={isStreaming}
              className="w-full px-3 py-2 text-sm rounded-md appearance-none cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
              style={{
                backgroundColor: 'var(--color-paper-warm)',
                border: '1px solid var(--color-border)',
                color: service ? 'var(--color-text-primary)' : 'var(--color-text-muted)',
                backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%236b6b6b' d='M3 4.5L6 7.5L9 4.5'/%3E%3C/svg%3E")`,
                backgroundRepeat: 'no-repeat',
                backgroundPosition: 'right 12px center',
                paddingRight: '36px',
              }}
            >
              <option value="" disabled>
                Select a service...
              </option>
              {services.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </div>

          {/* Signal toggle (radio pills) */}
          <div>
            <label className="data-label block mb-2">Signal</label>
            <div
              className="inline-flex rounded-md p-1"
              style={{
                backgroundColor: 'var(--color-paper-warm)',
                border: '1px solid var(--color-border)',
              }}
              role="radiogroup"
              aria-label="Signal type"
            >
              <SignalPill
                value="logs"
                label="Logs"
                selected={signal === 'logs'}
                disabled={isStreaming}
                onChange={() => onSignalChange('logs')}
              />
              <SignalPill
                value="traces"
                label="Traces"
                selected={signal === 'traces'}
                disabled={isStreaming}
                onChange={() => onSignalChange('traces')}
              />
            </div>
          </div>

          {/* Start/Stop button */}
          <div>
            <button
              type="button"
              onClick={onStartStop}
              disabled={!canStart && !isStreaming}
              className="px-4 py-2 text-sm font-medium rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              style={{
                backgroundColor: isStreaming ? '#dc2626' : 'var(--color-accent)',
                color: 'white',
                minWidth: '100px',
              }}
            >
              <AnimatePresence mode="wait" initial={false}>
                <motion.span
                  key={isStreaming ? 'stop' : 'start'}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -8 }}
                  transition={{ duration: 0.15 }}
                  className="block"
                >
                  {isStreaming ? 'Stop' : 'Start'}
                </motion.span>
              </AnimatePresence>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

interface SignalPillProps {
  value: TailSignal;
  label: string;
  selected: boolean;
  disabled: boolean;
  onChange: () => void;
}

function SignalPill({ value, label, selected, disabled, onChange }: SignalPillProps) {
  return (
    <label
      className={`relative px-4 py-1.5 text-sm font-medium rounded cursor-pointer transition-colors ${
        disabled ? 'opacity-50 cursor-not-allowed' : ''
      }`}
      style={{
        backgroundColor: selected ? 'white' : 'transparent',
        color: selected ? 'var(--color-text-primary)' : 'var(--color-text-tertiary)',
        boxShadow: selected ? 'var(--shadow-sm)' : 'none',
      }}
    >
      <input
        type="radio"
        name="tail-signal"
        value={value}
        checked={selected}
        disabled={disabled}
        onChange={onChange}
        className="sr-only"
      />
      {label}
    </label>
  );
}
