import type { Service } from '../lib/api';

export interface ServiceCellProps {
  service: Service;
  errorRate: number;
  isSelected: boolean;
  onClick: () => void;
}

/**
 * Get traffic light color based on error rate.
 * - 🟢 Green: 0% error rate
 * - 🟡 Yellow: >0% and <5% error rate
 * - 🔴 Red: ≥5% error rate
 */
function getHealthColor(errorRate: number): {
  bg: string;
  border: string;
  indicator: string;
} {
  if (errorRate === 0) {
    return {
      bg: 'bg-green-900/30',
      border: 'border-green-700',
      indicator: 'bg-green-500',
    };
  }
  if (errorRate < 5) {
    return {
      bg: 'bg-yellow-900/30',
      border: 'border-yellow-700',
      indicator: 'bg-yellow-500',
    };
  }
  return {
    bg: 'bg-red-900/30',
    border: 'border-red-700',
    indicator: 'bg-red-500',
  };
}

/**
 * Hexagonal service cell for the honeycomb grid.
 */
export function ServiceCell({
  service,
  errorRate,
  isSelected,
  onClick,
}: ServiceCellProps) {
  const colors = getHealthColor(errorRate);

  return (
    <button
      type="button"
      onClick={onClick}
      className={`
        relative flex flex-col items-center justify-center
        w-24 h-28 p-2
        border-2 transition-all duration-200
        ${colors.bg} ${colors.border}
        ${isSelected ? 'ring-2 ring-cyan-500 ring-offset-2 ring-offset-slate-900' : ''}
        hover:scale-105 hover:z-10
      `}
      style={{
        clipPath: 'polygon(50% 0%, 100% 25%, 100% 75%, 50% 100%, 0% 75%, 0% 25%)',
      }}
    >
      {/* Health indicator dot */}
      <span
        className={`absolute top-3 w-2.5 h-2.5 rounded-full ${colors.indicator}`}
      />

      {/* Service name */}
      <span
        className="text-xs font-medium text-slate-200 text-center truncate w-full mt-4"
        title={service.name}
      >
        {service.name.length > 10
          ? `${service.name.slice(0, 10)}…`
          : service.name}
      </span>

      {/* Signal indicators */}
      <div className="flex gap-1 mt-1">
        {service.has_logs && (
          <span className="text-[10px] text-cyan-400" title="Logs available">
            L
          </span>
        )}
        {service.has_traces && (
          <span className="text-[10px] text-violet-400" title="Traces available">
            T
          </span>
        )}
      </div>

      {/* Error rate */}
      <span className="text-[10px] text-slate-400 mt-0.5">
        {errorRate.toFixed(1)}%
      </span>
    </button>
  );
}
