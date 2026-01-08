import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import type { CatalogStats, TableStats } from '../hooks/useCatalogStats';
import { formatCompact, formatRelativeTime, formatBytes } from '../lib/format';

export interface CatalogOverviewProps {
  stats: CatalogStats | null;
  isLoading: boolean;
  error: string | null;
  onRefresh: () => void;
}

/**
 * Format schema fields as a preview string.
 */
function formatSchemaPreview(
  fields: Array<{ name: string; type: string }>,
  maxShow = 5
): string {
  if (fields.length === 0) return 'No fields';

  const shown = fields.slice(0, maxShow);
  const remaining = fields.length - maxShow;

  const preview = shown.map((f) => f.name).join(', ');

  if (remaining > 0) {
    return `${preview}, ... +${remaining} more`;
  }

  return preview;
}

/**
 * Skeleton loading placeholder for a stat card.
 */
function StatCardSkeleton() {
  return (
    <div
      className="rounded-lg p-4"
      style={{
        backgroundColor: 'white',
        border: '1px solid var(--color-border)',
      }}
    >
      <div
        className="h-3 w-16 rounded animate-pulse mb-2"
        style={{ backgroundColor: 'var(--color-border)' }}
      />
      <div
        className="h-8 w-20 rounded animate-pulse"
        style={{ backgroundColor: 'var(--color-border-light)' }}
      />
    </div>
  );
}

/**
 * Stat card for displaying a single metric.
 */
function StatCard({ label, value }: { label: string; value: string | number }) {
  return (
    <motion.div
      className="rounded-lg p-4"
      style={{
        backgroundColor: 'white',
        border: '1px solid var(--color-border)',
        boxShadow: 'var(--shadow-sm)',
      }}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2 }}
    >
      <span
        className="text-xs font-medium uppercase tracking-wider block mb-1"
        style={{ color: 'var(--color-text-tertiary)' }}
      >
        {label}
      </span>
      <span
        className="mono text-2xl font-semibold"
        style={{ color: 'var(--color-text-primary)' }}
      >
        {typeof value === 'number' ? formatCompact(value) : value}
      </span>
    </motion.div>
  );
}

/**
 * Table row skeleton for loading state.
 */
function TableRowSkeleton() {
  return (
    <tr>
      <td className="py-3 px-4">
        <div
          className="h-4 w-32 rounded animate-pulse"
          style={{ backgroundColor: 'var(--color-border)' }}
        />
      </td>
      <td className="py-3 px-4 text-right">
        <div
          className="h-4 w-10 rounded animate-pulse ml-auto"
          style={{ backgroundColor: 'var(--color-border-light)' }}
        />
      </td>
      <td className="py-3 px-4 text-right">
        <div
          className="h-4 w-24 rounded animate-pulse ml-auto"
          style={{ backgroundColor: 'var(--color-border-light)' }}
        />
      </td>
    </tr>
  );
}

/**
 * Refresh button component.
 */
function RefreshButton({ onClick, disabled }: { onClick: () => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="p-2 rounded-lg transition-colors disabled:opacity-50"
      style={{
        backgroundColor: 'var(--color-paper-warm)',
        border: '1px solid var(--color-border)',
      }}
      title="Refresh catalog stats"
    >
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        style={{ color: 'var(--color-text-secondary)' }}
        className={disabled ? 'animate-spin' : ''}
      >
        <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
        <path d="M3 3v5h5" />
        <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
        <path d="M16 16h5v5" />
      </svg>
    </button>
  );
}

/**
 * Chevron icon that rotates when expanded.
 */
function ChevronIcon({ isExpanded }: { isExpanded: boolean }) {
  return (
    <motion.svg
      xmlns="http://www.w3.org/2000/svg"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ color: 'var(--color-text-muted)' }}
      animate={{ rotate: isExpanded ? 180 : 0 }}
      transition={{ duration: 0.2 }}
    >
      <path d="m6 9 6 6 6-6" />
    </motion.svg>
  );
}

interface TableRowProps {
  table: TableStats;
  isExpanded: boolean;
  onToggle: () => void;
}

/**
 * Table row for displaying stats of a single table.
 * Expandable to show partition, storage, and schema details.
 */
function TableRow({ table, isExpanded, onToggle }: TableRowProps) {
  const fullName = `${table.namespace}.${table.name}`;

  return (
    <>
      <motion.tr
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.2 }}
        className="border-t cursor-pointer focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
        style={{
          borderColor: 'var(--color-border-light)',
        }}
        onClick={onToggle}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onToggle();
          }
        }}
        tabIndex={0}
        role="button"
        aria-expanded={isExpanded}
      >
        <td
          className="py-3 px-4 mono text-sm"
          style={{ color: 'var(--color-text-primary)' }}
        >
          <div className="flex items-center gap-2">
            <ChevronIcon isExpanded={isExpanded} />
            <span>{fullName}</span>
          </div>
        </td>
        <td
          className="py-3 px-4 text-right mono text-sm"
          style={{ color: 'var(--color-text-secondary)' }}
        >
          {table.snapshotCount > 0 ? formatCompact(table.snapshotCount) : '\u2014'}
        </td>
        <td
          className="py-3 px-4 text-right text-sm"
          style={{ color: 'var(--color-text-muted)' }}
        >
          {formatRelativeTime(table.lastUpdatedMs)}
        </td>
      </motion.tr>

      {/* Expanded content row */}
      <AnimatePresence>
        {isExpanded && (
          <motion.tr
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{
              height: { duration: 0.35, ease: [0.4, 0, 0.2, 1] },
              opacity: { duration: 0.25 },
            }}
            style={{ backgroundColor: 'var(--color-paper-warm)' }}
          >
            <td colSpan={3} className="overflow-hidden">
              <motion.div
                className="px-4 py-4 pl-10 space-y-3"
                initial={{ opacity: 0, y: -8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1, duration: 0.2 }}
              >
                {/* Partition */}
                <div className="flex items-start gap-4">
                  <span
                    className="text-xs font-medium uppercase tracking-wider w-20 shrink-0"
                    style={{ color: 'var(--color-text-tertiary)' }}
                  >
                    Partition
                  </span>
                  <span
                    className="mono text-sm"
                    style={{ color: 'var(--color-text-primary)' }}
                  >
                    {table.partitionSpec}
                  </span>
                </div>

                {/* Storage */}
                <div className="flex items-start gap-4">
                  <span
                    className="text-xs font-medium uppercase tracking-wider w-20 shrink-0"
                    style={{ color: 'var(--color-text-tertiary)' }}
                  >
                    Storage
                  </span>
                  <span
                    className="mono text-sm"
                    style={{ color: 'var(--color-text-primary)' }}
                  >
                    {formatCompact(table.fileCount)} files{' '}
                    <span style={{ color: 'var(--color-text-muted)' }}>·</span>{' '}
                    {formatCompact(table.recordCount)} records{' '}
                    <span style={{ color: 'var(--color-text-muted)' }}>·</span>{' '}
                    {formatBytes(table.totalSizeBytes)}
                  </span>
                </div>

                {/* Schema */}
                <div className="flex items-start gap-4">
                  <span
                    className="text-xs font-medium uppercase tracking-wider w-20 shrink-0"
                    style={{ color: 'var(--color-text-tertiary)' }}
                  >
                    Schema
                  </span>
                  <span
                    className="mono text-sm"
                    style={{ color: 'var(--color-text-primary)' }}
                  >
                    {formatSchemaPreview(table.schemaFields)}
                  </span>
                </div>
              </motion.div>
            </td>
          </motion.tr>
        )}
      </AnimatePresence>
    </>
  );
}

/**
 * Component that displays Iceberg catalog statistics.
 *
 * Shows:
 * - Summary cards with totals (tables, files, records, snapshots)
 * - Detailed table with per-table stats
 * - Loading, error, and empty states
 */
export function CatalogOverview({
  stats,
  isLoading,
  error,
  onRefresh,
}: CatalogOverviewProps) {
  // Track which table row is expanded (only one at a time)
  const [expandedTable, setExpandedTable] = useState<string | null>(null);

  const handleToggleExpand = (tableKey: string) => {
    setExpandedTable((prev) => (prev === tableKey ? null : tableKey));
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2
          className="headline text-xl"
          style={{ color: 'var(--color-text-primary)' }}
        >
          Catalog Overview
        </h2>
        <RefreshButton onClick={onRefresh} disabled={isLoading} />
      </div>

      {/* Error Banner */}
      {error && (
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          className="rounded-lg p-4"
          style={{
            backgroundColor: 'var(--color-error-bg)',
            border: '1px solid var(--color-error)',
          }}
        >
          <p className="text-sm" style={{ color: 'var(--color-error)' }}>
            {error}
          </p>
        </motion.div>
      )}

      {/* Summary Cards */}
      <div className="grid grid-cols-2 gap-4">
        {isLoading ? (
          <>
            <StatCardSkeleton />
            <StatCardSkeleton />
          </>
        ) : stats ? (
          <>
            <StatCard label="Tables" value={stats.totals.tableCount} />
            <StatCard label="Snapshots" value={stats.totals.snapshotCount} />
          </>
        ) : !error ? (
          <>
            <StatCard label="Tables" value="—" />
            <StatCard label="Snapshots" value="—" />
          </>
        ) : null}
      </div>

      {/* Tables Detail Table */}
      <div
        className="rounded-lg overflow-hidden"
        style={{
          backgroundColor: 'white',
          border: '1px solid var(--color-border)',
          boxShadow: 'var(--shadow-sm)',
        }}
      >
        <table className="w-full">
          <thead>
            <tr
              style={{
                backgroundColor: 'var(--color-paper-warm)',
                borderBottom: '1px solid var(--color-border)',
              }}
            >
              <th
                className="py-3 px-4 text-left text-xs font-medium uppercase tracking-wider"
                style={{ color: 'var(--color-text-tertiary)' }}
              >
                Table
              </th>
              <th
                className="py-3 px-4 text-right text-xs font-medium uppercase tracking-wider"
                style={{ color: 'var(--color-text-tertiary)' }}
              >
                Snapshots
              </th>
              <th
                className="py-3 px-4 text-right text-xs font-medium uppercase tracking-wider"
                style={{ color: 'var(--color-text-tertiary)' }}
              >
                Last Updated
              </th>
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              <>
                <TableRowSkeleton />
                <TableRowSkeleton />
              </>
            ) : stats && stats.tables.length > 0 ? (
              stats.tables.map((table) => {
                const tableKey = `${table.namespace}.${table.name}`;
                return (
                  <TableRow
                    key={tableKey}
                    table={table}
                    isExpanded={expandedTable === tableKey}
                    onToggle={() => handleToggleExpand(tableKey)}
                  />
                );
              })
            ) : !error ? (
              <tr>
                <td colSpan={3} className="py-12 text-center">
                  <p style={{ color: 'var(--color-text-secondary)' }}>
                    No tables found
                  </p>
                  <p
                    className="mt-2 text-sm"
                    style={{ color: 'var(--color-text-muted)' }}
                  >
                    Tables will appear here once data is ingested into the catalog.
                  </p>
                </td>
              </tr>
            ) : (
              <tr>
                <td colSpan={3} className="py-8 text-center">
                  <p style={{ color: 'var(--color-text-muted)' }}>
                    Unable to load table data
                  </p>
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
