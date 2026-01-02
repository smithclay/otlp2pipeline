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
        className="appearance-none rounded-md border border-slate-700 bg-slate-800 px-4 py-2 pr-8 text-sm text-slate-100 hover:border-slate-600 focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500 transition-colors cursor-pointer"
      >
        {TIME_RANGES.map((range) => (
          <option key={range.value} value={range.value}>
            {range.label}
          </option>
        ))}
      </select>
      {/* Dropdown arrow */}
      <div className="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2">
        <svg
          className="h-4 w-4 text-slate-400"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M19 9l-7 7-7-7"
          />
        </svg>
      </div>
    </div>
  );
}
