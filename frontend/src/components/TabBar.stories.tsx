import { useState } from 'react';
import type { Story } from '@ladle/react';
import { TabBar, type TabId, type Tab } from './TabBar';

export default {
  title: 'Components/TabBar',
};

export const Default: Story = () => {
  const [activeTab, setActiveTab] = useState<TabId>('query');

  return (
    <TabBar
      activeTab={activeTab}
      onTabChange={setActiveTab}
    />
  );
};

export const QueryActive: Story = () => {
  return (
    <TabBar
      activeTab="query"
      onTabChange={(tab) => console.log('Tab changed:', tab)}
    />
  );
};

export const TailActive: Story = () => {
  return (
    <TabBar
      activeTab="tail"
      onTabChange={(tab) => console.log('Tab changed:', tab)}
    />
  );
};

export const Interactive: Story = () => {
  const [activeTab, setActiveTab] = useState<TabId>('query');

  return (
    <div className="space-y-4">
      <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Click the tabs to see the animated underline transition
      </p>
      <TabBar
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />
      <p className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
        Active tab: <strong>{activeTab}</strong>
      </p>
    </div>
  );
};

export const CustomTabs: Story = () => {
  const customTabs: Tab[] = [
    { id: 'query', label: 'SQL Query' },
    { id: 'tail', label: 'Live Tail' },
  ];
  const [activeTab, setActiveTab] = useState<TabId>('query');

  return (
    <div className="space-y-4">
      <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Custom tab labels
      </p>
      <TabBar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        tabs={customTabs}
      />
    </div>
  );
};

export const InContext: Story = () => {
  const [activeTab, setActiveTab] = useState<TabId>('query');

  return (
    <div
      className="rounded-lg p-6"
      style={{
        backgroundColor: 'white',
        border: '1px solid var(--color-border)',
        boxShadow: 'var(--shadow-sm)',
      }}
    >
      <h2
        className="text-lg font-semibold mb-4"
        style={{ color: 'var(--color-text-primary)' }}
      >
        Records
      </h2>
      <TabBar
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />
      <div className="mt-4 p-4 rounded" style={{ backgroundColor: 'var(--color-paper-warm)' }}>
        {activeTab === 'query' ? (
          <p style={{ color: 'var(--color-text-secondary)' }}>
            Query mode content would go here...
          </p>
        ) : (
          <p style={{ color: 'var(--color-text-secondary)' }}>
            Tail mode content would go here...
          </p>
        )}
      </div>
    </div>
  );
};
