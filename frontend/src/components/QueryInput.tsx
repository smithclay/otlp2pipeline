import { useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';

export type QueryInputMode = 'query' | 'tail';

export type QueryInputState =
  | 'idle'
  | 'connecting'
  | 'running'
  | 'tailing'
  | 'reconnecting';

export interface QueryInputProps {
  /** Current SQL or TAIL command */
  value: string;
  /** Called when input changes */
  onChange: (value: string) => void;
  /** Called when user runs the query/tail */
  onRun: () => void;
  /** Current state of the query/tail */
  state: QueryInputState;
  /** Whether the input looks like a TAIL command */
  isTailCommand?: boolean;
  /** Whether the run action is available */
  canRun?: boolean;
  /** Reconnection attempt number (for reconnecting state) */
  reconnectAttempt?: number;
  /** Query execution time in ms (shown after query completes) */
  queryTimeMs?: number | null;
  /** Number of result rows (shown after query completes) */
  rowCount?: number | null;
  /** Number of tail records (shown during tail) */
  tailRecordCount?: number;
  /** Number of dropped tail records */
  droppedCount?: number;
  /** Placeholder text */
  placeholder?: string;
}

/**
 * SQL/TAIL command input component with run button.
 * Supports Cmd/Ctrl+Enter keyboard shortcut.
 */
export function QueryInput({
  value,
  onChange,
  onRun,
  state,
  isTailCommand = false,
  canRun = true,
  reconnectAttempt = 0,
  queryTimeMs = null,
  rowCount = null,
  tailRecordCount = 0,
  droppedCount = 0,
  placeholder = 'Enter SQL query or TAIL command...',
}: QueryInputProps) {
  // Handle keyboard shortcut (Cmd/Ctrl+Enter)
  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
        event.preventDefault();
        if (canRun || state === 'tailing') {
          onRun();
        }
      }
    },
    [onRun, canRun, state]
  );

  // Determine button text and style based on state
  const getButtonConfig = () => {
    switch (state) {
      case 'tailing':
        return { text: 'Stop', className: 'bg-red-500 hover:bg-red-600' };
      case 'connecting':
        return { text: 'Connecting...', className: '' };
      case 'running':
        return { text: 'Running...', className: '' };
      case 'reconnecting':
        return { text: `Reconnecting (${reconnectAttempt}/3)...`, className: '' };
      default:
        return {
          text: isTailCommand ? 'Start Tail' : 'Run Query',
          className: ''
        };
    }
  };

  const buttonConfig = getButtonConfig();
  const isActive = state === 'tailing' || state === 'running';

  return (
    <div className="space-y-4">
      {/* Status indicator */}
      <div className="flex items-center justify-end gap-4 min-h-[24px]">
        {/* Tail status */}
        {state === 'tailing' && (
          <div className="flex items-center gap-2 text-sm">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-red-500" />
            </span>
            <span style={{ color: 'var(--color-text-secondary)' }}>
              Live · {tailRecordCount} records
              {droppedCount > 0 && ` · ${droppedCount} dropped`}
            </span>
          </div>
        )}

        {state === 'connecting' && (
          <span className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
            Connecting...
          </span>
        )}

        {state === 'reconnecting' && (
          <span className="text-sm" style={{ color: 'var(--color-warning)' }}>
            Reconnecting ({reconnectAttempt}/3)...
          </span>
        )}

        {/* Query time indicator */}
        {state === 'idle' && queryTimeMs !== null && (
          <span
            className="text-sm font-medium mono"
            style={{ color: 'var(--color-text-tertiary)' }}
          >
            {queryTimeMs}ms
            {rowCount !== null && ` · ${rowCount} rows`}
          </span>
        )}
      </div>

      {/* SQL Input */}
      <div
        className="rounded-lg p-5"
        style={{
          backgroundColor: 'white',
          border: '1px solid var(--color-border)',
          boxShadow: 'var(--shadow-sm)',
        }}
      >
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          className="w-full h-32 px-3 py-2 font-mono text-sm rounded-md resize-y"
          style={{
            backgroundColor: 'var(--color-paper-warm)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-primary)',
          }}
          spellCheck={false}
        />
        <div className="flex items-center justify-between mt-3">
          <span className="text-xs" style={{ color: 'var(--color-text-muted)' }}>
            {isTailCommand
              ? 'Press Cmd+Enter to start tail'
              : 'Press Cmd+Enter to run query'}
          </span>
          <button
            type="button"
            onClick={onRun}
            disabled={!canRun && !isActive}
            className={`relative px-4 py-2 text-sm font-medium rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors ${buttonConfig.className}`}
            style={{
              backgroundColor: state === 'tailing' ? undefined : 'var(--color-accent)',
              color: 'white',
              minWidth: '180px',
            }}
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.span
                key={buttonConfig.text}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.15 }}
                className="block"
              >
                {buttonConfig.text}
              </motion.span>
            </AnimatePresence>
          </button>
        </div>
      </div>
    </div>
  );
}
