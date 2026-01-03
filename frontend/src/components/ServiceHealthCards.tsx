import { useMemo, forwardRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import type { Service } from '../lib/api';
import type { LogStats, TraceStats } from '../hooks/useStats';

/**
 * Service with aggregated stats for display.
 */
export interface ServiceWithStats {
  service: Service;
  errorRate: number;
  totalCount: number;
  errorCount: number;
}

/**
 * Detailed stats for RED charts in expanded view.
 */
export interface ServiceDetailStats {
  logStats: LogStats[];
  traceStats: TraceStats[];
}

export interface ServiceHealthCardsProps {
  services: ServiceWithStats[];
  selectedService: string | null;
  onSelectService: (name: string | null) => void;
  /** Detailed stats for the selected service */
  detailStats?: ServiceDetailStats;
  /** Loading state for detail stats */
  detailLoading?: boolean;
}

/**
 * Health status derived from error rate.
 */
type HealthStatus = 'healthy' | 'warning' | 'critical';

function getHealthStatus(errorRate: number): HealthStatus {
  if (errorRate >= 5) return 'critical';
  if (errorRate > 0) return 'warning';
  return 'healthy';
}

function getHealthColor(status: HealthStatus): string {
  switch (status) {
    case 'critical':
      return 'var(--color-error)';
    case 'warning':
      return 'var(--color-warning)';
    case 'healthy':
      return 'var(--color-healthy)';
  }
}


/**
 * Format large numbers with K/M suffixes.
 */
function formatCount(count: number): string {
  if (count >= 1_000_000) {
    return `${(count / 1_000_000).toFixed(1)}M`;
  }
  if (count >= 1_000) {
    return `${(count / 1_000).toFixed(1)}k`;
  }
  return count.toString();
}

/**
 * Mini sparkline component using SVG.
 */
function Sparkline({
  data,
  color = 'var(--color-accent)',
  width: propWidth,
  height: propHeight,
}: {
  data: number[];
  color?: string;
  width?: number;
  height?: number;
}) {
  const width = propWidth ?? 96;
  const height = propHeight ?? 24;

  if (data.length < 2) {
    return <div style={{ width, height }} />;
  }

  const max = Math.max(...data, 1);
  const padding = 2;

  const points = data.map((value, i) => {
    const x = padding + (i / (data.length - 1)) * (width - padding * 2);
    const y = height - padding - (value / max) * (height - padding * 2);
    return `${x},${y}`;
  });

  return (
    <svg width={width} height={height} className="overflow-visible">
      <polyline
        points={points.join(' ')}
        fill="none"
        stroke={color}
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        opacity="0.8"
      />
    </svg>
  );
}

/**
 * Inline RED metric with sparkline.
 */
function MetricSparkline({
  label,
  value,
  unit,
  color,
  data,
}: {
  label: string;
  value: number;
  unit: string;
  color: string;
  data: number[];
}) {
  return (
    <div className="flex items-center gap-3">
      <div className="w-20 shrink-0">
        <span
          className="text-xs font-medium uppercase tracking-wider"
          style={{ color: 'var(--color-text-tertiary)' }}
        >
          {label}
        </span>
      </div>
      <div className="flex-1 min-w-0 overflow-hidden mr-3">
        <Sparkline data={data} color={color} width={160} height={28} />
      </div>
      <div className="w-24 shrink-0 text-right">
        <span className="mono text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
          {typeof value === 'number' && !isNaN(value)
            ? value >= 1000
              ? formatCount(value)
              : value.toFixed(1)
            : '—'}
        </span>
        <span className="mono text-xs" style={{ color: 'var(--color-text-muted)' }}> {unit}</span>
      </div>
    </div>
  );
}

/**
 * Query suggestion chip.
 */
function QueryChip({
  icon,
  label,
  description,
  onClick,
}: {
  icon: string;
  label: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <motion.button
      onClick={onClick}
      className="flex flex-col items-start gap-1 p-3 rounded-lg text-left transition-colors"
      style={{
        backgroundColor: 'var(--color-paper-warm)',
        border: '1px solid var(--color-border)',
      }}
      whileHover={{
        backgroundColor: 'var(--color-paper-cool)',
        borderColor: 'var(--color-accent-light)',
      }}
      whileTap={{ scale: 0.98 }}
    >
      <div className="flex items-center gap-2">
        <span className="text-base">{icon}</span>
        <span className="text-sm font-medium" style={{ color: 'var(--color-text-primary)' }}>
          {label}
        </span>
      </div>
      <span className="text-xs mono" style={{ color: 'var(--color-text-muted)' }}>
        {description}
      </span>
    </motion.button>
  );
}

interface ServiceCardProps {
  item: ServiceWithStats;
  isSelected: boolean;
  isOtherSelected: boolean;
  onSelect: () => void;
  detailStats?: ServiceDetailStats;
  detailLoading?: boolean;
}

/**
 * Individual service card with expand/collapse.
 */
const ServiceCard = forwardRef<HTMLDivElement, ServiceCardProps>(function ServiceCard(
  { item, isSelected, isOtherSelected, onSelect, detailStats, detailLoading },
  ref
) {
  const navigate = useNavigate();
  const status = getHealthStatus(item.errorRate);
  const statusColor = getHealthColor(status);

  // Calculate throughput (requests per minute, assuming 15-min window)
  const throughputPerMin = item.totalCount / 15;

  // Calculate RED metrics from detail stats
  const redMetrics = useMemo(() => {
    if (!detailStats || !isSelected) return null;

    const { logStats, traceStats } = detailStats;

    // Rate: total requests per minute
    const totalRate = [...logStats, ...traceStats].reduce((sum, s) => sum + s.count, 0);
    const ratePerMin = logStats.length > 0 || traceStats.length > 0
      ? totalRate / Math.max(logStats.length, traceStats.length)
      : 0;
    const rateData = [...logStats, ...traceStats]
      .sort((a, b) => a.minute.localeCompare(b.minute))
      .map((s) => s.count);

    // Errors: error count per minute
    const totalErrors = [...logStats, ...traceStats].reduce((sum, s) => sum + s.error_count, 0);
    const errorsPerMin = logStats.length > 0 || traceStats.length > 0
      ? totalErrors / Math.max(logStats.length, traceStats.length)
      : 0;
    const errorData = [...logStats, ...traceStats]
      .sort((a, b) => a.minute.localeCompare(b.minute))
      .map((s) => s.error_count);

    // Duration: average latency from traces
    const latencyStats = traceStats.filter((s) => s.latency_sum_us !== undefined);
    const totalLatency = latencyStats.reduce((sum, s) => sum + (s.latency_sum_us ?? 0), 0);
    const totalTraceCount = latencyStats.reduce((sum, s) => sum + s.count, 0);
    const avgLatencyMs = totalTraceCount > 0 ? totalLatency / totalTraceCount / 1000 : 0;
    const latencyData = latencyStats
      .sort((a, b) => a.minute.localeCompare(b.minute))
      .map((s) => (s.count > 0 ? (s.latency_sum_us ?? 0) / s.count / 1000 : 0));

    return {
      rate: ratePerMin,
      rateData,
      errors: errorsPerMin,
      errorData,
      latency: avgLatencyMs,
      latencyData,
    };
  }, [detailStats, isSelected]);

  // Build query for Records Explorer
  const buildQuery = (type: 'logs' | 'traces', filter?: string) => {
    const table = type === 'logs' ? 'logs' : 'traces';
    let query = `SELECT * FROM r2_catalog.default.${table}\nWHERE service_name = '${item.service.name}'`;
    if (filter) {
      query += `\n  AND ${filter}`;
    }
    query += '\nORDER BY timestamp DESC\nLIMIT 100';
    return query;
  };

  const handleQueryClick = (type: 'logs' | 'traces', filter?: string) => {
    const query = buildQuery(type, filter);
    // Navigate to records with query as state
    navigate('/records', { state: { initialQuery: query } });
  };

  // Suggested queries based on service state
  const suggestedQueries = useMemo(() => {
    const queries: Array<{
      icon: string;
      label: string;
      description: string;
      type: 'logs' | 'traces';
      filter?: string;
    }> = [];

    if (status === 'critical' || status === 'warning') {
      queries.push({
        icon: '🚨',
        label: 'Recent Errors',
        description: 'severity >= ERROR',
        type: 'logs',
        filter: 'severity_number >= 17',
      });
    }

    if (item.service.has_traces) {
      queries.push({
        icon: '🐢',
        label: 'Slow Requests',
        description: 'duration > 500ms',
        type: 'traces',
        filter: 'duration_ms > 500',
      });
      queries.push({
        icon: '🔗',
        label: 'All Traces',
        description: 'last 100',
        type: 'traces',
      });
    }

    if (item.service.has_logs) {
      queries.push({
        icon: '📋',
        label: 'All Logs',
        description: 'last 100',
        type: 'logs',
      });
    }

    return queries.slice(0, 4);
  }, [status, item.service]);

  return (
    <motion.div
      ref={ref}
      layout
      className="rounded-xl overflow-hidden"
      style={{
        backgroundColor: 'white',
        border: `1px solid ${isSelected ? 'var(--color-accent)' : 'var(--color-border)'}`,
        boxShadow: isSelected ? '0 4px 20px rgba(30, 64, 175, 0.12)' : 'var(--shadow-sm)',
      }}
      animate={{
        opacity: isOtherSelected ? 0.6 : 1,
      }}
      transition={{ duration: 0.2 }}
    >
      {/* Collapsed card header - always visible */}
      <motion.button
        onClick={onSelect}
        className="w-full text-left p-5 cursor-pointer"
        whileHover={{ backgroundColor: 'rgba(0, 0, 0, 0.02)' }}
      >
        <div className="flex items-start justify-between gap-4">
          <div className="flex-1 min-w-0">
            {/* Service name */}
            <h3
              className="headline text-xl truncate"
              style={{ color: 'var(--color-text-primary)' }}
            >
              {item.service.name}
            </h3>

            {/* Health summary */}
            <p className="mt-1 text-sm" style={{ color: 'var(--color-text-secondary)' }}>
              <span className="mono" style={{ color: statusColor }}>
                {item.errorRate.toFixed(1)}%
              </span>
              {' errors · '}
              <span className="mono">{formatCount(Math.round(throughputPerMin))}</span>
              {' req/min'}
            </p>
          </div>

          {/* Status indicator and signal badges */}
          <div className="flex flex-col items-end gap-2">
            <span
              className="w-3 h-3 rounded-full"
              style={{ backgroundColor: statusColor }}
            />

            {/* Signal badges */}
            <div className="flex gap-1.5">
              {item.service.has_logs && (
                <span
                  className="px-2 py-0.5 text-xs font-medium rounded"
                  style={{
                    backgroundColor: 'rgba(21, 101, 192, 0.1)',
                    color: '#1565c0',
                  }}
                >
                  Logs
                </span>
              )}
              {item.service.has_traces && (
                <span
                  className="px-2 py-0.5 text-xs font-medium rounded"
                  style={{
                    backgroundColor: 'rgba(123, 31, 162, 0.1)',
                    color: '#7b1fa2',
                  }}
                >
                  Traces
                </span>
              )}
              {item.service.has_metrics && (
                <span
                  className="px-2 py-0.5 text-xs font-medium rounded"
                  style={{
                    backgroundColor: 'rgba(5, 150, 105, 0.1)',
                    color: '#059669',
                  }}
                >
                  Metrics
                </span>
              )}
            </div>
          </div>
        </div>
      </motion.button>

      {/* Expanded content */}
      <AnimatePresence>
        {isSelected && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{
              height: { duration: 0.35, ease: [0.4, 0, 0.2, 1] },
              opacity: { duration: 0.25 },
            }}
            className="overflow-hidden"
          >
            <div
              className="px-5 pb-5 pt-2 space-y-5"
              style={{ borderTop: '1px solid var(--color-border-light)' }}
            >
              {/* Loading state */}
              {detailLoading && (
                <div className="flex items-center justify-center py-8">
                  <motion.div
                    className="h-6 w-6 rounded-full border-2"
                    style={{
                      borderColor: 'var(--color-border)',
                      borderTopColor: 'var(--color-accent)',
                    }}
                    animate={{ rotate: 360 }}
                    transition={{ duration: 1, repeat: Infinity, ease: 'linear' }}
                  />
                </div>
              )}

              {/* RED Metrics */}
              {!detailLoading && redMetrics && (
                <motion.div
                  className="space-y-3"
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.1, duration: 0.3 }}
                >
                  <MetricSparkline
                    label="Rate"
                    value={redMetrics.rate}
                    unit="/min"
                    color="var(--color-accent)"
                    data={redMetrics.rateData}
                  />
                  <MetricSparkline
                    label="Errors"
                    value={redMetrics.errors}
                    unit="/min"
                    color="var(--color-error)"
                    data={redMetrics.errorData}
                  />
                  {redMetrics.latencyData.length > 0 && (
                    <MetricSparkline
                      label="Duration"
                      value={redMetrics.latency}
                      unit="ms"
                      color="var(--color-warning)"
                      data={redMetrics.latencyData}
                    />
                  )}
                </motion.div>
              )}

              {/* Suggested Queries */}
              {!detailLoading && suggestedQueries.length > 0 && (
                <motion.div
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <div className="flex items-center gap-2 mb-3">
                    <div
                      className="flex-1 h-px"
                      style={{ backgroundColor: 'var(--color-border)' }}
                    />
                    <span
                      className="text-xs font-medium uppercase tracking-wider"
                      style={{ color: 'var(--color-text-muted)' }}
                    >
                      Suggested Queries
                    </span>
                    <div
                      className="flex-1 h-px"
                      style={{ backgroundColor: 'var(--color-border)' }}
                    />
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    {suggestedQueries.map((query, index) => (
                      <motion.div
                        key={query.label}
                        initial={{ opacity: 0, y: 8 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: 0.25 + index * 0.05, duration: 0.2 }}
                      >
                        <QueryChip
                          icon={query.icon}
                          label={query.label}
                          description={query.description}
                          onClick={() => handleQueryClick(query.type, query.filter)}
                        />
                      </motion.div>
                    ))}
                  </div>
                </motion.div>
              )}

              {/* Close hint */}
              <motion.p
                className="text-center text-xs pt-2"
                style={{ color: 'var(--color-text-muted)' }}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: 0.35 }}
              >
                Click card to collapse
              </motion.p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
});

/**
 * Editorial card grid for service health overview.
 * Each card shows service health at a glance with expand-to-reveal RED metrics.
 */
export function ServiceHealthCards({
  services,
  selectedService,
  onSelectService,
  detailStats,
  detailLoading,
}: ServiceHealthCardsProps) {
  // Sort services: critical first, then warning, then healthy
  const sortedServices = useMemo(() => {
    return [...services].sort((a, b) => {
      const statusA = getHealthStatus(a.errorRate);
      const statusB = getHealthStatus(b.errorRate);
      const order = { critical: 0, warning: 1, healthy: 2 };
      if (order[statusA] !== order[statusB]) {
        return order[statusA] - order[statusB];
      }
      // Secondary sort by error rate descending
      return b.errorRate - a.errorRate;
    });
  }, [services]);

  if (services.length === 0) {
    return (
      <div
        className="rounded-xl p-8 text-center"
        style={{
          backgroundColor: 'var(--color-paper-warm)',
          border: '1px solid var(--color-border)',
        }}
      >
        <p style={{ color: 'var(--color-text-secondary)' }}>No services found.</p>
        <p className="mt-2 text-sm" style={{ color: 'var(--color-text-muted)' }}>
          Services will appear here once they start sending telemetry data.
        </p>
      </div>
    );
  }

  return (
    <motion.div
      className="grid gap-4"
      style={{
        gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))',
      }}
      layout
    >
      <AnimatePresence mode="popLayout">
        {sortedServices.map((item) => (
          <ServiceCard
            key={item.service.name}
            item={item}
            isSelected={selectedService === item.service.name}
            isOtherSelected={selectedService !== null && selectedService !== item.service.name}
            onSelect={() =>
              onSelectService(selectedService === item.service.name ? null : item.service.name)
            }
            detailStats={selectedService === item.service.name ? detailStats : undefined}
            detailLoading={selectedService === item.service.name ? detailLoading : false}
          />
        ))}
      </AnimatePresence>
    </motion.div>
  );
}
