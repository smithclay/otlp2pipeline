import { useState } from 'react';
import type { Story } from '@ladle/react';
import { ViewToggle, type ViewType } from './ViewToggle';

export default {
  title: 'Components/ViewToggle',
};

/**
 * Table view selected (default state).
 */
export const TableSelected: Story = () => (
  <ViewToggle view="table" onViewChange={() => {}} />
);

/**
 * Waterfall view selected.
 */
export const WaterfallSelected: Story = () => (
  <ViewToggle view="waterfall" onViewChange={() => {}} />
);

/**
 * Interactive toggle - click to switch between views.
 */
export const Interactive: Story = () => {
  const [view, setView] = useState<ViewType>('table');

  return (
    <div className="space-y-4">
      <ViewToggle view={view} onViewChange={setView} />
      <p className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
        Current view: <span className="font-medium">{view}</span>
      </p>
    </div>
  );
};

/**
 * Positioned in corner - demonstrates typical usage context.
 */
export const InContext: Story = () => {
  const [view, setView] = useState<ViewType>('table');

  return (
    <div
      className="relative rounded-lg p-4"
      style={{
        backgroundColor: 'white',
        border: '1px solid var(--color-border)',
        minHeight: '200px',
      }}
    >
      <div className="absolute top-3 right-3">
        <ViewToggle view={view} onViewChange={setView} />
      </div>
      <div
        className="flex items-center justify-center h-full"
        style={{ color: 'var(--color-text-muted)', minHeight: '160px' }}
      >
        {view === 'table' ? 'Table View Content' : 'Waterfall View Content'}
      </div>
    </div>
  );
};
