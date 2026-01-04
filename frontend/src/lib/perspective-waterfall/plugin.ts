/**
 * Perspective custom plugin for waterfall trace visualization
 *
 * This plugin extends the perspective-viewer-plugin base class to integrate
 * with Perspective's plugin system.
 */

import type * as perspective from '@finos/perspective';
import type { LayoutSpan, TraceLayout } from './types';
import { LAYOUT } from './types';
import { parseColumnarData } from './arrow';
import { computeLayout } from './layout';
import { render, hitTest, getTooltipText, type RenderContext } from './renderer';

// Plugin element name - follows perspective-viewer-* convention
const PLUGIN_NAME = 'perspective-viewer-waterfall';

/**
 * Get the base plugin class from Perspective.
 * This is dynamically retrieved because it's defined by perspective-viewer.
 */
function getBasePluginClass(): typeof HTMLElement | null {
  return customElements.get('perspective-viewer-plugin') as typeof HTMLElement | undefined ?? null;
}

/**
 * Custom element for waterfall visualization.
 * Extends the Perspective plugin base class for proper integration.
 */
export class WaterfallPluginElement extends HTMLElement {
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

  // ============================================
  // Perspective Plugin Interface - Required Getters
  // ============================================

  /**
   * Plugin name shown in the UI
   */
  get name(): string {
    return 'Waterfall';
  }

  /**
   * Selection mode - "select" for row selection, "toggle" for multi-select
   */
  get select_mode(): string {
    return 'select';
  }

  /**
   * Minimum number of columns required for the plugin to operate
   */
  get min_config_columns(): number {
    return 0;
  }

  /**
   * Named column labels for drag/drop behavior
   */
  get config_column_names(): string[] | undefined {
    return undefined;
  }

  /**
   * Plugin priority (higher = preferred)
   */
  get priority(): number {
    return 0;
  }

  // ============================================
  // Lifecycle Methods
  // ============================================

  connectedCallback(): void {
    this._setupDOM();
    this._setupEventListeners();
  }

  disconnectedCallback(): void {
    this._cleanup();
  }

  private _setupDOM(): void {
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
  }

  private _setupEventListeners(): void {
    if (!this._canvas || !this._container) return;

    this._canvas.addEventListener('click', this._handleClick);
    this._canvas.addEventListener('mousemove', this._handleMouseMove);
    this._canvas.addEventListener('mouseleave', this._handleMouseLeave);
    this._container.addEventListener('scroll', this._handleScroll);

    // Resize observer
    this._resizeObserver = new ResizeObserver(() => this._scheduleRender());
    this._resizeObserver.observe(this._container);
  }

  private _cleanup(): void {
    this._resizeObserver?.disconnect();
    if (this._animationFrame) {
      cancelAnimationFrame(this._animationFrame);
    }
    this._canvas?.removeEventListener('click', this._handleClick);
    this._canvas?.removeEventListener('mousemove', this._handleMouseMove);
    this._canvas?.removeEventListener('mouseleave', this._handleMouseLeave);
    this._container?.removeEventListener('scroll', this._handleScroll);
  }

  // ============================================
  // Perspective Plugin Interface - Required Methods
  // ============================================

  /**
   * Called by Perspective when data changes (full redraw)
   */
  async draw(view: perspective.View): Promise<void> {
    this._view = view;

    try {
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
   * Called by Perspective for incremental updates
   */
  async update(view: perspective.View): Promise<void> {
    // For waterfall, we just redraw on update
    await this.draw(view);
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

  /**
   * Called when container is resized
   */
  async resize(): Promise<void> {
    this._scheduleRender();
  }

  /**
   * Called when styles change
   */
  async restyle(_view: perspective.View): Promise<void> {
    this._scheduleRender();
  }

  /**
   * Save plugin state
   */
  async save(): Promise<Record<string, unknown>> {
    return {
      selectedSpanId: this._selectedSpanId,
    };
  }

  /**
   * Restore plugin state
   */
  async restore(config: Record<string, unknown>): Promise<void> {
    if (config.selectedSpanId && typeof config.selectedSpanId === 'string') {
      this._selectedSpanId = config.selectedSpanId;
    }
  }

  /**
   * Cleanup when plugin is destroyed
   */
  async delete(): Promise<void> {
    this._cleanup();
  }

  // ============================================
  // Rendering
  // ============================================

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

    const rc = this._createRenderContext();
    if (!rc) return;

    const { width, height, dpr } = rc;

    // Size canvas for retina
    canvas.width = width * dpr;
    canvas.height = Math.max(
      height,
      this._layout
        ? LAYOUT.TIME_AXIS_HEIGHT + this._layout.spans.length * LAYOUT.ROW_HEIGHT
        : height
    ) * dpr;

    render(rc);
  }

  // ============================================
  // Event Handlers
  // ============================================

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

  // ============================================
  // Utilities
  // ============================================

  private _createRenderContext(): RenderContext | null {
    if (!this._canvas || !this._container || !this._layout) return null;

    const rect = this._container.getBoundingClientRect();
    return {
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
  }

  private _hitTest(e: MouseEvent): LayoutSpan | null {
    const rc = this._createRenderContext();
    if (!rc) return null;
    return hitTest(rc, e.offsetY);
  }

  // ============================================
  // Public API
  // ============================================

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

// Keep old name as alias for backwards compatibility
export const WaterfallElement = WaterfallPluginElement;

/**
 * Create the plugin class that extends the Perspective base plugin.
 * This is done as a factory function because the base class isn't available at compile time.
 */
function createWaterfallPluginClass(BasePlugin: typeof HTMLElement): typeof HTMLElement {
  return class WaterfallPlugin extends BasePlugin {
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

    // Required getters for Perspective plugin interface
    get name() { return 'Waterfall'; }
    get select_mode() { return 'select'; }
    get min_config_columns() { return 0; }
    get priority() { return 0; }

    connectedCallback() {
      this._setupDOM();
      this._setupEventListeners();
    }

    disconnectedCallback() {
      this._cleanup();
    }

    private _setupDOM(): void {
      this._container = document.createElement('div');
      this._container.style.cssText = `
        position: relative;
        width: 100%;
        height: 100%;
        overflow-y: auto;
        overflow-x: hidden;
      `;

      this._canvas = document.createElement('canvas');
      this._canvas.style.cssText = `
        display: block;
        width: 100%;
      `;

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
    }

    private _setupEventListeners(): void {
      if (!this._canvas || !this._container) return;

      this._canvas.addEventListener('click', this._handleClick);
      this._canvas.addEventListener('mousemove', this._handleMouseMove);
      this._canvas.addEventListener('mouseleave', this._handleMouseLeave);
      this._container.addEventListener('scroll', this._handleScroll);

      this._resizeObserver = new ResizeObserver(() => this._scheduleRender());
      this._resizeObserver.observe(this._container);
    }

    private _cleanup(): void {
      this._resizeObserver?.disconnect();
      if (this._animationFrame) {
        cancelAnimationFrame(this._animationFrame);
      }
      this._canvas?.removeEventListener('click', this._handleClick);
      this._canvas?.removeEventListener('mousemove', this._handleMouseMove);
      this._canvas?.removeEventListener('mouseleave', this._handleMouseLeave);
      this._container?.removeEventListener('scroll', this._handleScroll);
    }

    // Perspective plugin interface methods
    async draw(view: perspective.View): Promise<void> {
      this._view = view;
      try {
        const data = await view.to_columns();
        const rawSpans = parseColumnarData(data as Record<string, unknown[]>);
        this._layout = computeLayout(rawSpans);

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

    async update(view: perspective.View): Promise<void> {
      await this.draw(view);
    }

    async clear(): Promise<void> {
      this._view = null;
      this._layout = null;
      this._selectedSpanId = null;
      this._hoveredSpanId = null;
      this._scheduleRender();
    }

    async resize(): Promise<void> {
      this._scheduleRender();
    }

    async restyle(_view: perspective.View): Promise<void> {
      this._scheduleRender();
    }

    async save(): Promise<Record<string, unknown>> {
      return { selectedSpanId: this._selectedSpanId };
    }

    async restore(config: Record<string, unknown>): Promise<void> {
      if (config.selectedSpanId && typeof config.selectedSpanId === 'string') {
        this._selectedSpanId = config.selectedSpanId;
      }
    }

    async delete(): Promise<void> {
      this._cleanup();
    }

    // Rendering
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

      const rc = this._createRenderContext();
      if (!rc) return;

      const { width, height, dpr } = rc;

      canvas.width = width * dpr;
      canvas.height = Math.max(
        height,
        this._layout
          ? LAYOUT.TIME_AXIS_HEIGHT + this._layout.spans.length * LAYOUT.ROW_HEIGHT
          : height
      ) * dpr;

      render(rc);
    }

    // Event handlers
    private _handleClick = (e: MouseEvent): void => {
      const span = this._hitTest(e);
      if (span) {
        this._selectedSpanId = span.span_id;
        this._scheduleRender();
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

    // Utilities
    private _createRenderContext(): RenderContext | null {
      if (!this._canvas || !this._container || !this._layout) return null;

      const rect = this._container.getBoundingClientRect();
      return {
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
    }

    private _hitTest(e: MouseEvent): LayoutSpan | null {
      const rc = this._createRenderContext();
      if (!rc) return null;
      return hitTest(rc, e.offsetY);
    }

    // Public API
    getSelectedSpan(): LayoutSpan | null {
      if (!this._layout || !this._selectedSpanId) return null;
      return this._layout.spans.find(s => s.span_id === this._selectedSpanId) ?? null;
    }

    clearSelection(): void {
      this._selectedSpanId = null;
      this._scheduleRender();
    }

    getView(): perspective.View | null {
      return this._view;
    }
  };
}

/**
 * Register the waterfall plugin with Perspective.
 * This must be called after perspective-viewer is loaded.
 */
export function registerWaterfallPlugin(): void {
  // Don't register if already registered
  if (customElements.get(PLUGIN_NAME)) {
    return;
  }

  // Get the base plugin class
  const BasePlugin = getBasePluginClass();

  if (BasePlugin) {
    const PluginClass = createWaterfallPluginClass(BasePlugin);
    customElements.define(PLUGIN_NAME, PluginClass);
  } else {
    // Fallback: register WaterfallPluginElement directly
    console.warn('perspective-viewer-plugin base class not found, registering WaterfallPluginElement directly');
    customElements.define(PLUGIN_NAME, WaterfallPluginElement);
  }

  // Register the plugin with perspective-viewer
  const Viewer = customElements.get('perspective-viewer') as (typeof HTMLElement & {
    registerPlugin?: (name: string) => void;
  }) | undefined;

  if (Viewer?.registerPlugin) {
    Viewer.registerPlugin(PLUGIN_NAME);
  } else {
    console.warn('perspective-viewer.registerPlugin not available');
  }
}
