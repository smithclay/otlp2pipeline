/**
 * ViewToggle - A compact pill-shaped toggle for switching between Table and Waterfall views.
 * Used in the evidence area when viewing trace data.
 */

export type ViewType = 'table' | 'waterfall';

export interface ViewToggleProps {
  /** Currently selected view */
  view: ViewType;
  /** Called when view selection changes */
  onViewChange: (view: ViewType) => void;
}

/**
 * Compact segmented control for switching between Table and Waterfall views.
 * Designed to be unobtrusive - fits in the corner of a larger container.
 */
export function ViewToggle({ view, onViewChange }: ViewToggleProps) {
  return (
    <div
      className="inline-flex rounded-full p-0.5"
      style={{
        backgroundColor: 'var(--color-paper-cool)',
        border: '1px solid var(--color-border)',
      }}
      role="tablist"
      aria-label="View mode"
    >
      <ToggleButton
        label="Table"
        isSelected={view === 'table'}
        onClick={() => onViewChange('table')}
      />
      <ToggleButton
        label="Waterfall"
        isSelected={view === 'waterfall'}
        onClick={() => onViewChange('waterfall')}
      />
    </div>
  );
}

interface ToggleButtonProps {
  label: string;
  isSelected: boolean;
  onClick: () => void;
}

function ToggleButton({ label, isSelected, onClick }: ToggleButtonProps) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={isSelected}
      onClick={onClick}
      className="px-3 py-1 text-xs font-medium rounded-full transition-all duration-150"
      style={{
        backgroundColor: isSelected ? 'var(--color-accent)' : 'transparent',
        color: isSelected ? 'white' : 'var(--color-text-muted)',
      }}
    >
      {label}
    </button>
  );
}
