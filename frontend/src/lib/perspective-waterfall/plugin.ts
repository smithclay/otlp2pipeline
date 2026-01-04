/**
 * Perspective custom plugin for waterfall trace visualization
 */

import type * as perspective from '@finos/perspective';
import type { LayoutSpan, TraceLayout } from './types';
import { LAYOUT } from './types';
import { parseColumnarData } from './arrow';
import { computeLayout } from './layout';
import { render, hitTest, getTooltipText, type RenderContext } from './renderer';

/**
 * Custom element for waterfall visualization
 */
export class WaterfallElement extends HTMLElement {
  private _view: perspective.View | null = null;
  private _canvas: HTMLCanvasElement | null = null;
  private _container: HTMLDivElement | null = null;
  private _tooltip: HTMLDivElement | null = null;
  private _layout: TraceLayout | null = null;
  private _selectedSpanId: string | null = null;
  private _hoveredSpanId: string | null = null;
  private _scrollTop = 0;
  private _resizeObserver: ResizeObserver | null = null;
  private _animationFrame: number | null = null;

  connectedCallback(): void {
    // Create container
    this._container = document.createElement('div');
    this._container.style.cssText = `
      position: relative;
      width: 100%;
      height: 100%;
      overflow-y: auto;
      overflow-x: hidden;
    `;

    // Create canvas
    this._canvas = document.createElement('canvas');
    this._canvas.style.cssText = `
      display: block;
      width: 100%;
    `;

    // Create tooltip
    this._tooltip = document.createElement('div');
    this._tooltip.style.cssText = `
      position: absolute;
      background: #2d3436;
      color: white;
      padding: 6px 10px;
      border-radius: 4px;
      font-size: 12px;
      font-family: system-ui, sans-serif;
      pointer-events: none;
      opacity: 0;
      transition: opacity 0.15s;
      white-space: pre;
      z-index: 100;
      box-shadow: 0 2px 8px rgba(0,0,0,0.15);
    `;

    this._container.appendChild(this._canvas);
    this._container.appendChild(this._tooltip);
    this.appendChild(this._container);

    // Event listeners
    this._canvas.addEventListener('click', this._handleClick);
    this._canvas.addEventListener('mousemove', this._handleMouseMove);
    this._canvas.addEventListener('mouseleave', this._handleMouseLeave);
    this._container.addEventListener('scroll', this._handleScroll);

    // Resize observer
    this._resizeObserver = new ResizeObserver(() => this._scheduleRender());
    this._resizeObserver.observe(this._container);
  }

  disconnectedCallback(): void {
    this._resizeObserver?.disconnect();
    if (this._animationFrame) {
      cancelAnimationFrame(this._animationFrame);
    }
    this._canvas?.removeEventListener('click', this._handleClick);
    this._canvas?.removeEventListener('mousemove', this._handleMouseMove);
    this._canvas?.removeEventListener('mouseleave', this._handleMouseLeave);
    this._container?.removeEventListener('scroll', this._handleScroll);
  }

  /**
   * Called by Perspective when data changes
   */
  async draw(view: perspective.View): Promise<void> {
    this._view = view;

    try {
      // Get columnar data from view
      const data = await view.to_columns();
      const rawSpans = parseColumnarData(data as Record<string, unknown[]>);
      this._layout = computeLayout(rawSpans);

      // Update canvas height for scrolling
      if (this._canvas && this._layout) {
        const totalHeight = LAYOUT.TIME_AXIS_HEIGHT +
          this._layout.spans.length * LAYOUT.ROW_HEIGHT;
        this._canvas.style.height = `${totalHeight}px`;
      }

      this._scheduleRender();
    } catch (err) {
      console.error('Waterfall draw error:', err);
    }
  }

  /**
   * Called when view is cleared
   */
  async clear(): Promise<void> {
    this._view = null;
    this._layout = null;
    this._selectedSpanId = null;
    this._hoveredSpanId = null;
    this._scheduleRender();
  }

  private _scheduleRender(): void {
    if (this._animationFrame) return;
    this._animationFrame = requestAnimationFrame(() => {
      this._animationFrame = null;
      this._render();
    });
  }

  private _render(): void {
    const canvas = this._canvas;
    const container = this._container;
    if (!canvas || !container) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    const width = rect.width;
    const height = rect.height;

    // Size canvas for retina
    canvas.width = width * dpr;
    canvas.height = Math.max(
      height,
      this._layout
        ? LAYOUT.TIME_AXIS_HEIGHT + this._layout.spans.length * LAYOUT.ROW_HEIGHT
        : height
    ) * dpr;

    if (!this._layout) {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.scale(dpr, dpr);
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 0, width, height);
      return;
    }

    const rc: RenderContext = {
      canvas,
      ctx,
      layout: this._layout,
      selectedSpanId: this._selectedSpanId,
      hoveredSpanId: this._hoveredSpanId,
      scrollTop: this._scrollTop,
      width,
      height,
      dpr,
    };

    render(rc);
  }

  private _handleClick = (e: MouseEvent): void => {
    const span = this._hitTest(e);
    if (span) {
      this._selectedSpanId = span.span_id;
      this._scheduleRender();

      // Dispatch selection event
      this.dispatchEvent(new CustomEvent('span-select', {
        detail: { span },
        bubbles: true,
      }));
    }
  };

  private _handleMouseMove = (e: MouseEvent): void => {
    const span = this._hitTest(e);
    const prevHovered = this._hoveredSpanId;
    this._hoveredSpanId = span?.span_id ?? null;

    if (prevHovered !== this._hoveredSpanId) {
      this._scheduleRender();
    }

    // Update tooltip
    if (span && this._tooltip) {
      this._tooltip.textContent = getTooltipText(span);
      this._tooltip.style.opacity = '1';
      this._tooltip.style.left = `${e.offsetX + 12}px`;
      this._tooltip.style.top = `${e.offsetY + 12}px`;
    } else if (this._tooltip) {
      this._tooltip.style.opacity = '0';
    }
  };

  private _handleMouseLeave = (): void => {
    if (this._hoveredSpanId) {
      this._hoveredSpanId = null;
      this._scheduleRender();
    }
    if (this._tooltip) {
      this._tooltip.style.opacity = '0';
    }
  };

  private _handleScroll = (): void => {
    if (this._container) {
      this._scrollTop = this._container.scrollTop;
      this._scheduleRender();
    }
  };

  private _hitTest(e: MouseEvent): LayoutSpan | null {
    if (!this._canvas || !this._container || !this._layout) return null;

    const rect = this._container.getBoundingClientRect();
    const rc: RenderContext = {
      canvas: this._canvas,
      ctx: this._canvas.getContext('2d')!,
      layout: this._layout,
      selectedSpanId: this._selectedSpanId,
      hoveredSpanId: this._hoveredSpanId,
      scrollTop: this._scrollTop,
      width: rect.width,
      height: rect.height,
      dpr: window.devicePixelRatio || 1,
    };

    return hitTest(rc, e.offsetX, e.offsetY);
  }

  /**
   * Get currently selected span
   */
  getSelectedSpan(): LayoutSpan | null {
    if (!this._layout || !this._selectedSpanId) return null;
    return this._layout.spans.find(s => s.span_id === this._selectedSpanId) ?? null;
  }

  /**
   * Clear selection
   */
  clearSelection(): void {
    this._selectedSpanId = null;
    this._scheduleRender();
  }

  /**
   * Get the current Perspective view (if any)
   */
  getView(): perspective.View | null {
    return this._view;
  }
}

/**
 * Register the waterfall plugin with Perspective
 */
export function registerWaterfallPlugin(): void {
  // Register custom element
  if (!customElements.get('perspective-waterfall')) {
    customElements.define('perspective-waterfall', WaterfallElement);
  }

  // Note: Perspective plugin registration happens in perspective.ts
  // This function is called to ensure the element is defined
}
