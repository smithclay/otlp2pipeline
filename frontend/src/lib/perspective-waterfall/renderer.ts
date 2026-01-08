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
export function formatDuration(ms: number): string {
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

  // Clear canvas and set up scaling
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.scale(dpr, dpr);

  // Optimize text rendering for legibility
  ctx.textRendering = 'optimizeLegibility';

  ctx.fillStyle = COLORS.background;
  ctx.fillRect(0, 0, width, height);

  if (layout.spans.length === 0) {
    // Empty state
    ctx.fillStyle = COLORS.textMuted;
    ctx.font = '500 14px system-ui, -apple-system, sans-serif';
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
 * Calculate nice round tick interval
 */
function niceTickInterval(duration: number, maxTicks: number): number {
  const rough = duration / maxTicks;
  const magnitude = Math.pow(10, Math.floor(Math.log10(rough)));
  const normalized = rough / magnitude;

  let nice: number;
  if (normalized <= 1) nice = 1;
  else if (normalized <= 2) nice = 2;
  else if (normalized <= 5) nice = 5;
  else nice = 10;

  return nice * magnitude;
}

/**
 * Draw time axis at top with vertical grid lines
 */
function drawTimeAxis(rc: RenderContext): void {
  const { ctx, layout, width, height } = rc;
  const timelineWidth = width - LAYOUT.TREE_PANEL_WIDTH - LAYOUT.TIMELINE_PADDING_RIGHT;

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

  // Calculate nice round tick interval
  const totalDuration = layout.total_duration;
  const maxTicks = Math.floor(timelineWidth / 80);
  const tickInterval = niceTickInterval(totalDuration, maxTicks);

  ctx.fillStyle = COLORS.textSecondary;
  ctx.font = '500 11px system-ui, -apple-system, sans-serif';
  ctx.textAlign = 'center';

  // Draw ticks at round intervals
  for (let time = 0; time <= totalDuration; time += tickInterval) {
    const x = LAYOUT.TREE_PANEL_WIDTH + (time / totalDuration) * timelineWidth;

    // Vertical grid line (full height, very light)
    ctx.strokeStyle = 'rgba(0,0,0,0.04)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(x, LAYOUT.TIME_AXIS_HEIGHT);
    ctx.lineTo(x, height);
    ctx.stroke();

    // Tick mark at axis
    ctx.strokeStyle = COLORS.gridLine;
    ctx.beginPath();
    ctx.moveTo(x, LAYOUT.TIME_AXIS_HEIGHT - 8);
    ctx.lineTo(x, LAYOUT.TIME_AXIS_HEIGHT);
    ctx.stroke();

    // Label (round numbers)
    ctx.fillText(formatDuration(time), x, LAYOUT.TIME_AXIS_HEIGHT - 12);
  }
}

/**
 * Draw a single span row (two-line layout)
 */
function drawSpan(rc: RenderContext, span: LayoutSpan): void {
  const { ctx, layout, width, scrollTop, selectedSpanId, hoveredSpanId } = rc;
  const timelineWidth = width - LAYOUT.TREE_PANEL_WIDTH - LAYOUT.TIMELINE_PADDING_RIGHT;

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

  // Row separator line
  ctx.strokeStyle = COLORS.gridLine;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, y + LAYOUT.ROW_HEIGHT);
  ctx.lineTo(width, y + LAYOUT.ROW_HEIGHT);
  ctx.stroke();

  // === Tree panel: two-line layout ===
  const indent = 12 + span.depth * LAYOUT.INDENT_PER_DEPTH;
  const line1Y = y + 16;  // Span name line
  const line2Y = y + 30;  // Service name line

  // Status indicator (positioned for first line)
  if (span.is_error) {
    // Error: red X
    ctx.strokeStyle = COLORS.errorBorder;
    ctx.lineWidth = 2;
    ctx.lineCap = 'round';
    const size = 4;
    ctx.beginPath();
    ctx.moveTo(indent - size, line1Y - size);
    ctx.lineTo(indent + size, line1Y + size);
    ctx.moveTo(indent + size, line1Y - size);
    ctx.lineTo(indent - size, line1Y + size);
    ctx.stroke();
  } else {
    // OK: green checkmark
    ctx.strokeStyle = '#4caf50';
    ctx.lineWidth = 2;
    ctx.lineCap = 'round';
    ctx.beginPath();
    ctx.moveTo(indent - 4, line1Y);
    ctx.lineTo(indent - 1, line1Y + 3);
    ctx.lineTo(indent + 5, line1Y - 3);
    ctx.stroke();
  }

  // Line 1: Span name
  ctx.fillStyle = COLORS.textPrimary;
  ctx.font = '500 12px system-ui, -apple-system, sans-serif';
  ctx.textAlign = 'left';
  const nameX = indent + 12;
  const maxNameWidth = LAYOUT.TREE_PANEL_WIDTH - nameX - 8;
  const truncatedName = truncateText(ctx, span.span_name, maxNameWidth);
  ctx.fillText(truncatedName, nameX, line1Y + 4);

  // Line 2: Service name (muted)
  ctx.fillStyle = COLORS.textMuted;
  ctx.font = '400 11px system-ui, -apple-system, sans-serif';
  const truncatedService = truncateText(ctx, span.service_name, maxNameWidth);
  ctx.fillText(truncatedService, nameX, line2Y + 2);

  // === Timeline bar ===
  const barX = LAYOUT.TREE_PANEL_WIDTH +
    ((span.timestamp - layout.trace_start) / layout.total_duration) * timelineWidth;
  const barWidth = Math.max(2, (span.duration / layout.total_duration) * timelineWidth);
  const barY = y + LAYOUT.BAR_PADDING;

  // Bar fill - error bars get red fill, OK bars get service color
  if (span.is_error) {
    ctx.fillStyle = COLORS.error;  // Red fill for errors
  } else {
    ctx.fillStyle = getServiceColor(span.service_name);
  }
  ctx.fillRect(barX, barY, barWidth, LAYOUT.BAR_HEIGHT);

  // Selection border
  if (isSelected) {
    ctx.strokeStyle = COLORS.selectedBorder;
    ctx.lineWidth = 2;
    ctx.strokeRect(barX - 1, barY - 1, barWidth + 2, LAYOUT.BAR_HEIGHT + 2);
  }

  // === Duration label at end of bar ===
  ctx.fillStyle = COLORS.textSecondary;
  ctx.font = '500 10px system-ui, -apple-system, sans-serif';
  ctx.textAlign = 'left';
  const durationText = formatDuration(span.duration);
  const labelX = barX + barWidth + 6;
  // Only show if it fits
  if (labelX + ctx.measureText(durationText).width < width - 8) {
    ctx.fillText(durationText, labelX, barY + LAYOUT.BAR_HEIGHT / 2 + 3);
  }
}

/**
 * Hit test: find span at y coordinate (row-based selection)
 */
export function hitTest(
  rc: RenderContext,
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
