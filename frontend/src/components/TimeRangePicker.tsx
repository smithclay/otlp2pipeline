import { TimeRange, TIME_RANGES } from '../hooks/useStats';

interface TimeRangePickerProps {
  value: TimeRange;
  onChange: (range: TimeRange) => void;
}

/**
 * Dropdown component for selecting a time range.
 */
export function TimeRangePicker({ value, onChange }: TimeRangePickerProps) {
  return (
    <div className="relative">
      <select
        value={value.value}
        onChange={(e) => {
          const selected = TIME_RANGES.find((r) => r.value === e.target.value);
          if (selected) {
            onChange(selected);
          }
        }}
        aria-label="Select time range"
        className="appearance-none rounded-md px-4 py-2 pr-8 text-sm font-medium transition-colors cursor-pointer"
        style={{
          backgroundColor: 'white',
          border: '1px solid var(--color-border)',
          color: 'var(--color-text-secondary)',
        }}
      >
        {TIME_RANGES.map((range) => (
          <option key={range.value} value={range.value}>
            {range.label}
          </option>
        ))}
      </select>
      {/* Dropdown arrow */}
      <div
        className="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2"
        style={{ color: 'var(--color-text-muted)' }}
      >
        <svg
          className="h-4 w-4"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          strokeWidth={1.5}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M19.5 8.25l-7.5 7.5-7.5-7.5"
          />
        </svg>
      </div>
    </div>
  );
}
