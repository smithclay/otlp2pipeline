/**
 * Log details panel - slides in from right when a log is selected
 */

import { useMemo, useState, useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';

// ============================================================================
// Types
// ============================================================================

export interface LogRecord {
  timestamp: string; // ISO format
  severity: string; // 'INFO', 'WARN', 'ERROR', etc.
  message: string;
  service: string;
  trace_id?: string;
  span_id?: string;
  host?: string;
  attributes?: Record<string, unknown>;
  resource_attributes?: Record<string, unknown>;
}

export interface LogDetailPanelProps {
  log: LogRecord | null;
  onClose: () => void;
  onTraceClick?: (traceId: string) => void;
}

// ============================================================================
// Severity styling
// ============================================================================

function getSeverityColor(severity: string): string {
  const upper = severity.toUpperCase();
  switch (upper) {
    case 'ERROR':
    case 'FATAL':
    case 'CRITICAL':
      return 'var(--color-error)';
    case 'WARN':
    case 'WARNING':
      return '#f59e0b'; // amber-500
    case 'INFO':
      return 'var(--color-accent)';
    case 'DEBUG':
    case 'TRACE':
      return 'var(--color-text-muted)';
    default:
      return 'var(--color-text-secondary)';
  }
}

function getSeverityBg(severity: string): string {
  const upper = severity.toUpperCase();
  switch (upper) {
    case 'ERROR':
    case 'FATAL':
    case 'CRITICAL':
      return 'rgba(239, 68, 68, 0.1)';
    case 'WARN':
    case 'WARNING':
      return 'rgba(245, 158, 11, 0.1)';
    case 'INFO':
      return 'rgba(59, 130, 246, 0.1)';
    case 'DEBUG':
    case 'TRACE':
      return 'rgba(107, 114, 128, 0.1)';
    default:
      return 'rgba(107, 114, 128, 0.1)';
  }
}

// ============================================================================
// Main Component
// ============================================================================

export function LogDetailPanel({ log, onClose, onTraceClick }: LogDetailPanelProps) {
  // Handle Escape key to close
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape' && log) {
        onClose();
      }
    },
    [log, onClose]
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  // Handle click outside to close
  const handleBackdropClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.target === e.currentTarget) {
        onClose();
      }
    },
    [onClose]
  );

  return (
    <AnimatePresence>
      {log && (
        <>
          {/* Backdrop for click-outside-to-close */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="absolute inset-0"
            style={{ zIndex: 9 }}
            onClick={handleBackdropClick}
          />
          <motion.div
            initial={{ x: '100%', opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: '100%', opacity: 0 }}
            transition={{ type: 'spring', damping: 25, stiffness: 200 }}
            className="absolute top-0 right-0 h-full overflow-y-auto"
            style={{
              width: 'min(400px, 35%)',
              minWidth: '320px',
              backgroundColor: 'var(--color-paper)',
              borderLeft: '1px solid var(--color-border)',
              boxShadow: '-4px 0 16px rgba(0,0,0,0.08)',
              zIndex: 10,
            }}
          >
            <LogDetailContent log={log} onClose={onClose} onTraceClick={onTraceClick} />
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

// ============================================================================
// Content Component
// ============================================================================

function LogDetailContent({
  log,
  onClose,
  onTraceClick,
}: {
  log: LogRecord;
  onClose: () => void;
  onTraceClick?: (traceId: string) => void;
}) {
  const formattedTimestamp = useMemo(() => {
    try {
      const date = new Date(log.timestamp);
      // Format with milliseconds: "Jan 4, 2025, 10:30:45.123"
      const base = date.toLocaleString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      });
      const ms = date.getMilliseconds().toString().padStart(3, '0');
      return `${base}.${ms}`;
    } catch {
      return log.timestamp;
    }
  }, [log.timestamp]);

  return (
    <div className="flex flex-col h-full">
      {/* Header: Timestamp and Severity */}
      <div
        className="flex items-start justify-between p-4 border-b"
        style={{ borderColor: 'var(--color-border)' }}
      >
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span
              className="px-2 py-0.5 rounded text-xs font-semibold uppercase"
              style={{
                color: getSeverityColor(log.severity),
                backgroundColor: getSeverityBg(log.severity),
              }}
            >
              {log.severity}
            </span>
          </div>
          <p
            className="text-sm font-mono"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            {formattedTimestamp}
          </p>
        </div>
        <button
          onClick={onClose}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onClose();
            }
          }}
          className="p-1 rounded hover:bg-black/5 transition-colors flex-shrink-0"
          style={{ color: 'var(--color-text-muted)' }}
          aria-label="Close"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Message */}
      <div className="p-4 border-b" style={{ borderColor: 'var(--color-border)' }}>
        <dt
          style={{ color: 'var(--color-text-muted)' }}
          className="text-xs mb-1.5 uppercase tracking-wide font-medium"
        >
          Message
        </dt>
        <dd>
          <pre
            className="p-3 rounded text-sm font-mono whitespace-pre-wrap break-words overflow-x-auto"
            style={{
              backgroundColor: 'var(--color-paper-warm)',
              color: 'var(--color-text-primary)',
              maxHeight: '200px',
              overflowY: 'auto',
            }}
          >
            {log.message}
          </pre>
        </dd>
      </div>

      {/* Key Fields */}
      <div className="p-4 border-b" style={{ borderColor: 'var(--color-border)' }}>
        <dl className="space-y-3 text-sm">
          <div>
            <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
              Service
            </dt>
            <dd style={{ color: 'var(--color-text-primary)' }} className="font-medium">
              {log.service}
            </dd>
          </div>

          {log.trace_id && (
            <div>
              <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
                Trace ID
              </dt>
              <dd>
                {onTraceClick ? (
                  <button
                    onClick={() => onTraceClick(log.trace_id!)}
                    className="font-mono text-xs truncate hover:underline cursor-pointer text-left"
                    style={{ color: 'var(--color-accent)' }}
                    title="Click to view trace"
                  >
                    {log.trace_id}
                  </button>
                ) : (
                  <span
                    style={{ color: 'var(--color-text-secondary)' }}
                    className="font-mono text-xs truncate block"
                  >
                    {log.trace_id}
                  </span>
                )}
              </dd>
            </div>
          )}

          {log.span_id && (
            <div>
              <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
                Span ID
              </dt>
              <dd
                style={{ color: 'var(--color-text-secondary)' }}
                className="font-mono text-xs truncate"
              >
                {log.span_id}
              </dd>
            </div>
          )}

          {log.host && (
            <div>
              <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
                Host
              </dt>
              <dd
                style={{ color: 'var(--color-text-secondary)' }}
                className="font-mono text-xs truncate"
              >
                {log.host}
              </dd>
            </div>
          )}
        </dl>
      </div>

      {/* Expandable Attributes */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <AttributeSection
          title="Attributes"
          data={log.attributes}
          defaultExpanded
        />
        <AttributeSection
          title="Resource Attributes"
          data={log.resource_attributes}
        />
      </div>
    </div>
  );
}

// ============================================================================
// Attribute Section
// ============================================================================

interface AttributeSectionProps {
  title: string;
  data?: Record<string, unknown>;
  defaultExpanded?: boolean;
}

function AttributeSection({ title, data, defaultExpanded = false }: AttributeSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  if (!data || Object.keys(data).length === 0) {
    return null;
  }

  return (
    <div>
      <button
        onClick={() => setExpanded(!expanded)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setExpanded(!expanded);
          }
        }}
        aria-expanded={expanded}
        className="flex items-center gap-2 w-full text-left"
      >
        <svg
          className={`w-4 h-4 transition-transform ${expanded ? 'rotate-90' : ''}`}
          style={{ color: 'var(--color-text-muted)' }}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
        <span
          className="text-xs font-medium uppercase tracking-wide"
          style={{ color: 'var(--color-text-secondary)' }}
        >
          {title}
        </span>
        <span
          className="text-xs"
          style={{ color: 'var(--color-text-muted)' }}
        >
          ({Object.keys(data).length})
        </span>
      </button>
      {expanded && (
        <div className="mt-2 pl-6">
          <JsonTree data={data} />
        </div>
      )}
    </div>
  );
}

// ============================================================================
// JSON Tree Renderer
// ============================================================================

function JsonTree({ data, depth = 0 }: { data: unknown; depth?: number }) {
  if (data === null) {
    return <span style={{ color: 'var(--color-text-muted)' }}>null</span>;
  }

  if (typeof data !== 'object') {
    return (
      <span
        style={{
          color:
            typeof data === 'string'
              ? 'var(--color-accent)'
              : typeof data === 'number'
              ? '#0d9488'
              : typeof data === 'boolean'
              ? '#7c3aed'
              : 'var(--color-text-primary)',
        }}
        className="font-mono text-xs"
      >
        {typeof data === 'string' ? `"${data}"` : String(data)}
      </span>
    );
  }

  const entries = Object.entries(data as Record<string, unknown>);

  if (entries.length === 0) {
    return (
      <span style={{ color: 'var(--color-text-muted)' }} className="font-mono text-xs">
        {Array.isArray(data) ? '[]' : '{}'}
      </span>
    );
  }

  return (
    <div className="space-y-1">
      {entries.map(([key, value]) => (
        <div key={key} className="flex items-start gap-2">
          <span
            style={{ color: 'var(--color-text-secondary)' }}
            className="font-mono text-xs flex-shrink-0"
          >
            {key}:
          </span>
          <JsonTree data={value} depth={depth + 1} />
        </div>
      ))}
    </div>
  );
}
