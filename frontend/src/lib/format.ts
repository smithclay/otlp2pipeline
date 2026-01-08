/**
 * Shared formatting utilities for numbers, durations, and bytes.
 */

/**
 * Format large numbers with K/M suffixes.
 * Examples: 1234 -> "1.2k", 1234567 -> "1.2M"
 */
export function formatCompact(count: number): string {
  if (count >= 1_000_000) {
    return `${(count / 1_000_000).toFixed(1)}M`;
  }
  if (count >= 1_000) {
    return `${(count / 1_000).toFixed(1)}k`;
  }
  return count.toString();
}

/**
 * Format numbers with locale-aware separators.
 * Examples: 1234 -> "1,234"
 */
export function formatNumber(n: number): string {
  return n.toLocaleString('en-US');
}

/**
 * Format milliseconds with appropriate precision.
 * Examples: 500 -> "500ms", 1500 -> "1.50s"
 */
export function formatMs(ms: number): string {
  if (ms >= 1000) {
    return `${(ms / 1000).toFixed(2)}s`;
  }
  return `${Math.round(ms)}ms`;
}

/**
 * Format bytes as human-readable string.
 * Returns "0 B" for zero, negative, or invalid values.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const unitIndex = Math.min(i, units.length - 1);

  const value = bytes / Math.pow(k, unitIndex);
  const formatted = unitIndex === 0 ? value.toString() : value.toFixed(1);

  return `${formatted} ${units[unitIndex]}`;
}

/**
 * Format timestamp as relative time (e.g., "5 minutes ago").
 */
export function formatRelativeTime(timestampMs: number | null): string {
  if (timestampMs === null) {
    return '\u2014'; // em-dash
  }

  const now = Date.now();
  const diffMs = now - timestampMs;
  const diffSeconds = Math.floor(diffMs / 1000);
  const diffMinutes = Math.floor(diffSeconds / 60);
  const diffHours = Math.floor(diffMinutes / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSeconds < 60) {
    return 'just now';
  }
  if (diffMinutes < 60) {
    return `${diffMinutes} minute${diffMinutes === 1 ? '' : 's'} ago`;
  }
  if (diffHours < 24) {
    return `${diffHours} hour${diffHours === 1 ? '' : 's'} ago`;
  }
  if (diffDays < 7) {
    return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
  }

  const date = new Date(timestampMs);
  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() !== new Date().getFullYear() ? 'numeric' : undefined,
  });
}
