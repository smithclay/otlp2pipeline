/**
 * Recursive JSON tree renderer for displaying structured data.
 */

interface JsonTreeProps {
  data: unknown;
  depth?: number;
}

/**
 * Renders JSON data as a collapsible tree structure.
 * Uses color coding for different value types:
 * - Strings: accent blue
 * - Numbers: teal
 * - Booleans: purple
 * - Null: muted gray
 */
export function JsonTree({ data, depth = 0 }: JsonTreeProps) {
  if (data === null) {
    return <span style={{ color: 'var(--color-text-muted)' }}>null</span>;
  }

  if (typeof data !== 'object') {
    return (
      <span
        style={{
          color:
            typeof data === 'string'
              ? 'var(--color-accent)'
              : typeof data === 'number'
              ? '#0d9488'
              : typeof data === 'boolean'
              ? '#7c3aed'
              : 'var(--color-text-primary)',
        }}
        className="font-mono text-xs"
      >
        {typeof data === 'string' ? `"${data}"` : String(data)}
      </span>
    );
  }

  const entries = Object.entries(data as Record<string, unknown>);

  if (entries.length === 0) {
    return (
      <span style={{ color: 'var(--color-text-muted)' }} className="font-mono text-xs">
        {Array.isArray(data) ? '[]' : '{}'}
      </span>
    );
  }

  return (
    <div className="space-y-1">
      {entries.map(([key, value]) => (
        <div key={key} className="flex items-start gap-2">
          <span
            style={{ color: 'var(--color-text-secondary)' }}
            className="font-mono text-xs flex-shrink-0"
          >
            {key}:
          </span>
          <JsonTree data={value} depth={depth + 1} />
        </div>
      ))}
    </div>
  );
}
