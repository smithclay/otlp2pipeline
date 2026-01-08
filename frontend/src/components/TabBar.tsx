import { motion } from 'framer-motion';

export type TabId = 'query' | 'tail';

export interface Tab {
  id: TabId;
  label: string;
}

export interface TabBarProps {
  /** Currently active tab */
  activeTab: TabId;
  /** Called when a tab is clicked */
  onTabChange: (tab: TabId) => void;
  /** Optional custom tabs (defaults to Query/Tail) */
  tabs?: Tab[];
}

const DEFAULT_TABS: Tab[] = [
  { id: 'query', label: 'Query' },
  { id: 'tail', label: 'Tail' },
];

/**
 * TabBar component with underline-style tabs.
 * Used for switching between Query and Tail modes on the Records page.
 */
export function TabBar({
  activeTab,
  onTabChange,
  tabs = DEFAULT_TABS,
}: TabBarProps) {
  return (
    <div
      className="flex gap-1"
      style={{
        borderBottom: '1px solid var(--color-border)',
      }}
      role="tablist"
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab;

        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={isActive}
            onClick={() => onTabChange(tab.id)}
            className="relative px-4 py-2.5 text-sm font-medium transition-colors"
            style={{
              color: isActive
                ? 'var(--color-accent)'
                : 'var(--color-text-secondary)',
              backgroundColor: 'transparent',
            }}
            onMouseEnter={(e) => {
              if (!isActive) {
                e.currentTarget.style.color = 'var(--color-text-primary)';
              }
            }}
            onMouseLeave={(e) => {
              if (!isActive) {
                e.currentTarget.style.color = 'var(--color-text-secondary)';
              }
            }}
          >
            {tab.label}
            {isActive && (
              <motion.div
                layoutId="tab-underline"
                className="absolute bottom-0 left-0 right-0 h-0.5"
                style={{
                  backgroundColor: 'var(--color-accent)',
                }}
                transition={{
                  type: 'spring',
                  stiffness: 500,
                  damping: 30,
                }}
              />
            )}
          </button>
        );
      })}
    </div>
  );
}
