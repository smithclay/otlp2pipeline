/**
 * DFS tree layout algorithm for trace spans
 */

import type { RawSpan, LayoutSpan, TraceLayout } from './types';

/**
 * Parse raw span data and compute tree layout.
 *
 * Algorithm:
 * 1. Build parent->children adjacency map
 * 2. Identify root spans (null parent or orphans)
 * 3. Sort children by start_time, then duration (longest first)
 * 4. DFS traversal to assign depth and row_index
 */
export function computeLayout(rawSpans: RawSpan[]): TraceLayout {
  if (rawSpans.length === 0) {
    return {
      spans: [],
      roots: [],
      trace_start: 0,
      trace_end: 0,
      total_duration: 0,
    };
  }

  // Build span lookup and children map
  const spanMap = new Map<string, LayoutSpan>();
  const childrenMap = new Map<string, LayoutSpan[]>();

  // First pass: create LayoutSpan objects
  for (const raw of rawSpans) {
    const span: LayoutSpan = {
      ...raw,
      depth: 0,
      row_index: 0,
      is_error: String(raw.status_code) === '2' || raw.status_code === 'ERROR',
      children: [],
    };
    spanMap.set(raw.span_id, span);
  }

  // Second pass: build parent->children relationships
  for (const span of spanMap.values()) {
    const parentId = span.parent_span_id;
    if (parentId && spanMap.has(parentId)) {
      // Has valid parent
      let siblings = childrenMap.get(parentId);
      if (!siblings) {
        siblings = [];
        childrenMap.set(parentId, siblings);
      }
      siblings.push(span);
    }
  }

  // Sort children: start_time ASC, then duration DESC, then span_id ASC
  for (const children of childrenMap.values()) {
    children.sort((a, b) => {
      if (a.timestamp !== b.timestamp) {
        return a.timestamp - b.timestamp;
      }
      if (a.duration !== b.duration) {
        return b.duration - a.duration; // Longer first
      }
      return a.span_id.localeCompare(b.span_id);
    });
  }

  // Link children to parents
  for (const [parentId, children] of childrenMap) {
    const parent = spanMap.get(parentId);
    if (parent) {
      parent.children = children;
    }
  }

  // Find roots: spans with null parent or parent not in trace
  const roots: LayoutSpan[] = [];
  for (const span of spanMap.values()) {
    const parentId = span.parent_span_id;
    if (!parentId || !spanMap.has(parentId)) {
      roots.push(span);
    }
  }

  // Sort roots same as children
  roots.sort((a, b) => {
    if (a.timestamp !== b.timestamp) {
      return a.timestamp - b.timestamp;
    }
    if (a.duration !== b.duration) {
      return b.duration - a.duration;
    }
    return a.span_id.localeCompare(b.span_id);
  });

  // DFS to assign depth and row_index
  const orderedSpans: LayoutSpan[] = [];
  let rowIndex = 0;

  function dfs(span: LayoutSpan, depth: number): void {
    span.depth = depth;
    span.row_index = rowIndex++;
    orderedSpans.push(span);

    for (const child of span.children) {
      dfs(child, depth + 1);
    }
  }

  for (const root of roots) {
    dfs(root, 0);
  }

  // Compute time bounds
  let traceStart = Infinity;
  let traceEnd = -Infinity;

  for (const span of orderedSpans) {
    if (span.timestamp < traceStart) {
      traceStart = span.timestamp;
    }
    if (span.end_timestamp > traceEnd) {
      traceEnd = span.end_timestamp;
    }
  }

  // Handle edge case of single instant span
  if (traceStart === traceEnd) {
    traceEnd = traceStart + 1;
  }

  return {
    spans: orderedSpans,
    roots,
    trace_start: traceStart,
    trace_end: traceEnd,
    total_duration: traceEnd - traceStart,
  };
}
