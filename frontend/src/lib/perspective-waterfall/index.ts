/**
 * Waterfall trace viewer plugin
 *
 * Importing this module automatically registers the waterfall plugin
 * with Perspective viewer.
 */

import { registerWaterfallPlugin } from './plugin';

// Auto-register the plugin when this module is imported
// This is a side effect, matching how official Perspective plugins work
registerWaterfallPlugin();

// Re-export for manual registration if needed
export { registerWaterfallPlugin, WaterfallElement, WaterfallPluginElement } from './plugin';
export { computeLayout } from './layout';
export { parseColumnarData } from './arrow';
export type { RawSpan, LayoutSpan, TraceLayout } from './types';
export { LAYOUT } from './types';
export { COLORS, getServiceColor } from './colors';
export { formatDuration } from './renderer';
