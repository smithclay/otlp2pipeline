/**
 * Span details panel - slides in from right when a span is selected
 */

import { useMemo, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import type { LayoutSpan } from '../lib/perspective-waterfall';
import { getServiceColor } from '../lib/perspective-waterfall';

interface SpanDetailsPanelProps {
  span: LayoutSpan | null;
  onClose: () => void;
}

export function SpanDetailsPanel({ span, onClose }: SpanDetailsPanelProps) {
  return (
    <AnimatePresence>
      {span && (
        <motion.div
          initial={{ x: '100%' }}
          animate={{ x: 0 }}
          exit={{ x: '100%' }}
          transition={{ type: 'spring', damping: 25, stiffness: 200 }}
          className="absolute top-0 right-0 h-full w-80 overflow-y-auto"
          style={{
            backgroundColor: 'var(--color-paper)',
            borderLeft: '1px solid var(--color-border)',
            boxShadow: '-4px 0 16px rgba(0,0,0,0.08)',
            zIndex: 10,
          }}
        >
          <SpanDetailContent span={span} onClose={onClose} />
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function SpanDetailContent({ span, onClose }: { span: LayoutSpan; onClose: () => void }) {
  const serviceColor = getServiceColor(span.service_name);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div
        className="flex items-start justify-between p-4 border-b"
        style={{ borderColor: 'var(--color-border)' }}
      >
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span
              className="w-3 h-3 rounded-full flex-shrink-0"
              style={{ backgroundColor: serviceColor }}
            />
            <span
              className="text-xs font-medium truncate"
              style={{ color: 'var(--color-text-secondary)' }}
            >
              {span.service_name}
            </span>
          </div>
          <h3
            className="text-sm font-semibold truncate"
            style={{ color: 'var(--color-text-primary)' }}
          >
            {span.span_name}
          </h3>
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

      {/* Summary */}
      <div className="p-4 border-b" style={{ borderColor: 'var(--color-border)' }}>
        <dl className="grid grid-cols-2 gap-3 text-sm">
          <div>
            <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
              Duration
            </dt>
            <dd style={{ color: 'var(--color-text-primary)' }} className="font-medium font-mono">
              {formatDuration(span.duration)}
            </dd>
          </div>
          <div>
            <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
              Status
            </dt>
            <dd className="flex items-center gap-1.5">
              <span
                className="w-2 h-2 rounded-full"
                style={{
                  backgroundColor: span.is_error
                    ? 'var(--color-error)'
                    : 'var(--color-healthy)',
                }}
              />
              <span
                style={{
                  color: span.is_error
                    ? 'var(--color-error)'
                    : 'var(--color-text-primary)',
                }}
                className="font-medium"
              >
                {span.status_code || 'UNSET'}
              </span>
            </dd>
          </div>
          <div className="col-span-2">
            <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
              Span ID
            </dt>
            <dd
              style={{ color: 'var(--color-text-secondary)' }}
              className="font-mono text-xs truncate"
            >
              {span.span_id}
            </dd>
          </div>
          {span.parent_span_id && (
            <div className="col-span-2">
              <dt style={{ color: 'var(--color-text-muted)' }} className="text-xs mb-0.5">
                Parent Span ID
              </dt>
              <dd
                style={{ color: 'var(--color-text-secondary)' }}
                className="font-mono text-xs truncate"
              >
                {span.parent_span_id}
              </dd>
            </div>
          )}
        </dl>
      </div>

      {/* Attributes */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        <AttributeSection
          title="Span Attributes"
          json={span.span_attributes}
          defaultExpanded
        />
        <AttributeSection
          title="Resource Attributes"
          json={span.resource_attributes}
        />
        <AttributeSection
          title="Scope Attributes"
          json={span.scope_attributes}
        />
      </div>
    </div>
  );
}

interface AttributeSectionProps {
  title: string;
  json?: string;
  defaultExpanded?: boolean;
}

function AttributeSection({ title, json, defaultExpanded = false }: AttributeSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  const parsed = useMemo(() => {
    if (!json) return null;
    try {
      return JSON.parse(json);
    } catch {
      return null;
    }
  }, [json]);

  if (!parsed || Object.keys(parsed).length === 0) {
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
      </button>
      {expanded && (
        <div className="mt-2 pl-6">
          <JsonTree data={parsed} />
        </div>
      )}
    </div>
  );
}

function JsonTree({ data, depth = 0 }: { data: unknown; depth?: number }) {
  if (data === null) {
    return <span style={{ color: 'var(--color-text-muted)' }}>null</span>;
  }

  if (typeof data !== 'object') {
    return (
      <span
        style={{
          color: typeof data === 'string'
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

function formatDuration(ms: number): string {
  if (ms < 1) {
    return `${(ms * 1000).toFixed(0)}μs`;
  }
  if (ms < 1000) {
    return `${ms.toFixed(1)}ms`;
  }
  return `${(ms / 1000).toFixed(2)}s`;
}
