/**
 * Muted pastel color palette (FT/Economist aesthetic)
 */

export const SERVICE_PALETTE = [
  '#8dd3c7',  // Soft teal
  '#bebada',  // Lavender
  '#80b1d3',  // Sky blue
  '#fdb462',  // Peach
  '#b3de69',  // Lime
  '#fccde5',  // Pink
  '#d9d9d9',  // Gray
  '#bc80bd',  // Mauve
  '#ccebc5',  // Mint
  '#ffffb3',  // Cream
] as const;

export const COLORS = {
  error: '#e07a5f',         // Coral/terracotta
  errorBorder: '#c1513b',   // Darker coral for border
  selectedBg: '#f8f4f0',    // Warm off-white
  selectedBorder: '#d4c5b5', // Warm gray
  textPrimary: '#2d3436',   // Soft black
  textSecondary: '#636e72', // Muted gray
  textMuted: '#b2bec3',     // Light gray
  gridLine: '#e9ecef',      // Very light gray
  background: '#ffffff',    // White
  treePanelBg: '#fafafa',   // Slight off-white
} as const;

/**
 * Hash a service name to a consistent palette color.
 * Same service always gets same color across sessions.
 */
export function hashServiceColor(serviceName: string): string {
  let hash = 0;
  for (let i = 0; i < serviceName.length; i++) {
    const char = serviceName.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // Convert to 32-bit int
  }
  return SERVICE_PALETTE[Math.abs(hash) % SERVICE_PALETTE.length];
}

export function getServiceColor(serviceName: string): string {
  return hashServiceColor(serviceName);
}
