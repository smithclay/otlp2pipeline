/**
 * Canvas renderer for waterfall visualization
 */

import type { LayoutSpan, TraceLayout } from './types';
import { LAYOUT } from './types';
import { COLORS, getServiceColor } from './colors';

export interface RenderContext {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  layout: TraceLayout;
  selectedSpanId: string | null;
  hoveredSpanId: string | null;
  scrollTop: number;
  width: number;
  height: number;
  dpr: number; // Device pixel ratio
}

/**
 * Format duration for display
 */
function formatDuration(ms: number): string {
  if (ms < 1) {
    return `${(ms * 1000).toFixed(0)}μs`;
  }
  if (ms < 1000) {
    return `${ms.toFixed(1)}ms`;
  }
  return `${(ms / 1000).toFixed(2)}s`;
}

/**
 * Truncate text to fit width
 */
function truncateText(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number
): string {
  const ellipsis = '…';
  let width = ctx.measureText(text).width;

  if (width <= maxWidth) {
    return text;
  }

  while (width > maxWidth && text.length > 0) {
    text = text.slice(0, -1);
    width = ctx.measureText(text + ellipsis).width;
  }

  return text + ellipsis;
}

/**
 * Main render function
 */
export function render(rc: RenderContext): void {
  const { ctx, layout, width, height, dpr, scrollTop } = rc;

  // Clear canvas
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.scale(dpr, dpr);
  ctx.fillStyle = COLORS.background;
  ctx.fillRect(0, 0, width, height);

  if (layout.spans.length === 0) {
    // Empty state
    ctx.fillStyle = COLORS.textMuted;
    ctx.font = '14px system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('No spans to display', width / 2, height / 2);
    return;
  }

  // Draw time axis
  drawTimeAxis(rc);

  // Draw tree panel background
  ctx.fillStyle = COLORS.treePanelBg;
  ctx.fillRect(0, LAYOUT.TIME_AXIS_HEIGHT, LAYOUT.TREE_PANEL_WIDTH, height - LAYOUT.TIME_AXIS_HEIGHT);

  // Draw vertical divider
  ctx.strokeStyle = COLORS.gridLine;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(LAYOUT.TREE_PANEL_WIDTH, LAYOUT.TIME_AXIS_HEIGHT);
  ctx.lineTo(LAYOUT.TREE_PANEL_WIDTH, height);
  ctx.stroke();

  // Calculate visible row range
  const firstVisibleRow = Math.floor(scrollTop / LAYOUT.ROW_HEIGHT);
  const visibleRows = Math.ceil(height / LAYOUT.ROW_HEIGHT) + 1;
  const lastVisibleRow = Math.min(
    firstVisibleRow + visibleRows,
    layout.spans.length
  );

  // Draw visible spans
  for (let i = firstVisibleRow; i < lastVisibleRow; i++) {
    const span = layout.spans[i];
    if (span) {
      drawSpan(rc, span);
    }
  }
}

/**
 * Draw time axis at top
 */
function drawTimeAxis(rc: RenderContext): void {
  const { ctx, layout, width } = rc;
  const timelineWidth = width - LAYOUT.TREE_PANEL_WIDTH;

  // Background
  ctx.fillStyle = COLORS.background;
  ctx.fillRect(0, 0, width, LAYOUT.TIME_AXIS_HEIGHT);

  // Bottom border
  ctx.strokeStyle = COLORS.gridLine;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, LAYOUT.TIME_AXIS_HEIGHT);
  ctx.lineTo(width, LAYOUT.TIME_AXIS_HEIGHT);
  ctx.stroke();

  // Calculate tick interval
  const totalDuration = layout.total_duration;
  const tickCount = Math.max(2, Math.min(10, Math.floor(timelineWidth / 80)));
  const tickInterval = totalDuration / tickCount;

  ctx.fillStyle = COLORS.textSecondary;
  ctx.font = '11px system-ui, sans-serif';
  ctx.textAlign = 'center';

  for (let i = 0; i <= tickCount; i++) {
    const time = i * tickInterval;
    const x = LAYOUT.TREE_PANEL_WIDTH + (i / tickCount) * timelineWidth;

    // Tick line
    ctx.strokeStyle = COLORS.gridLine;
    ctx.beginPath();
    ctx.moveTo(x, LAYOUT.TIME_AXIS_HEIGHT - 8);
    ctx.lineTo(x, LAYOUT.TIME_AXIS_HEIGHT);
    ctx.stroke();

    // Label
    ctx.fillText(formatDuration(time), x, LAYOUT.TIME_AXIS_HEIGHT - 12);
  }
}

/**
 * Draw a single span row
 */
function drawSpan(rc: RenderContext, span: LayoutSpan): void {
  const { ctx, layout, width, scrollTop, selectedSpanId, hoveredSpanId } = rc;
  const timelineWidth = width - LAYOUT.TREE_PANEL_WIDTH;

  const y = LAYOUT.TIME_AXIS_HEIGHT + span.row_index * LAYOUT.ROW_HEIGHT - scrollTop;

  // Skip if off screen
  if (y + LAYOUT.ROW_HEIGHT < LAYOUT.TIME_AXIS_HEIGHT || y > rc.height) {
    return;
  }

  const isSelected = span.span_id === selectedSpanId;
  const isHovered = span.span_id === hoveredSpanId;

  // Row background for selected/hovered
  if (isSelected) {
    ctx.fillStyle = COLORS.selectedBg;
    ctx.fillRect(0, y, width, LAYOUT.ROW_HEIGHT);
  } else if (isHovered) {
    ctx.fillStyle = 'rgba(0,0,0,0.02)';
    ctx.fillRect(0, y, width, LAYOUT.ROW_HEIGHT);
  }

  // Tree panel: service dot + span name
  const indent = 12 + span.depth * LAYOUT.INDENT_PER_DEPTH;
  const serviceColor = getServiceColor(span.service_name);

  // Service dot
  ctx.beginPath();
  ctx.arc(indent, y + LAYOUT.ROW_HEIGHT / 2, LAYOUT.SERVICE_DOT_RADIUS, 0, Math.PI * 2);
  ctx.fillStyle = serviceColor;
  ctx.fill();

  // Span name
  ctx.fillStyle = COLORS.textPrimary;
  ctx.font = '12px system-ui, sans-serif';
  ctx.textAlign = 'left';
  const nameX = indent + LAYOUT.SERVICE_DOT_RADIUS + 8;
  const maxNameWidth = LAYOUT.TREE_PANEL_WIDTH - nameX - 8;
  const truncatedName = truncateText(ctx, span.span_name, maxNameWidth);
  ctx.fillText(truncatedName, nameX, y + LAYOUT.ROW_HEIGHT / 2 + 4);

  // Timeline bar
  const barX = LAYOUT.TREE_PANEL_WIDTH +
    ((span.timestamp - layout.trace_start) / layout.total_duration) * timelineWidth;
  const barWidth = Math.max(2, (span.duration / layout.total_duration) * timelineWidth);
  const barY = y + LAYOUT.BAR_PADDING;

  // Bar fill
  ctx.fillStyle = serviceColor;
  ctx.fillRect(barX, barY, barWidth, LAYOUT.BAR_HEIGHT);

  // Error border
  if (span.is_error) {
    ctx.strokeStyle = COLORS.errorBorder;
    ctx.lineWidth = 2;
    ctx.strokeRect(barX, barY, barWidth, LAYOUT.BAR_HEIGHT);
  }

  // Selection border
  if (isSelected) {
    ctx.strokeStyle = COLORS.selectedBorder;
    ctx.lineWidth = 2;
    ctx.strokeRect(barX - 1, barY - 1, barWidth + 2, LAYOUT.BAR_HEIGHT + 2);
  }
}

/**
 * Hit test: find span at coordinates
 */
export function hitTest(
  rc: RenderContext,
  _x: number,
  y: number
): LayoutSpan | null {
  const { layout, scrollTop } = rc;

  // Must be below time axis
  if (y < LAYOUT.TIME_AXIS_HEIGHT) {
    return null;
  }

  // Calculate row
  const adjustedY = y - LAYOUT.TIME_AXIS_HEIGHT + scrollTop;
  const rowIndex = Math.floor(adjustedY / LAYOUT.ROW_HEIGHT);

  if (rowIndex >= 0 && rowIndex < layout.spans.length) {
    return layout.spans[rowIndex];
  }

  return null;
}

/**
 * Get tooltip text for a span
 */
export function getTooltipText(span: LayoutSpan): string {
  return `${span.service_name}: ${span.span_name}\n${formatDuration(span.duration)}`;
}
