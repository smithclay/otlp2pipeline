import { HexGrid, Layout, Hexagon, Text } from 'react-hexgrid';
import type { Service } from '../lib/api';

export interface ServiceWithStats {
  service: Service;
  errorRate: number;
}

export interface HoneycombGridProps {
  services: ServiceWithStats[];
  selectedService: string | null;
  onSelectService: (name: string) => void;
}

/**
 * Get health color based on error rate.
 * Data journalism palette - pastel fills with dark text for maximum legibility.
 * Text is always near-black for readability, only the fill/stroke indicate status.
 */
function getHealthColor(errorRate: number): { fill: string; stroke: string; text: string } {
  // Text is always dark for legibility - status shown by fill color only
  const darkText = '#2d2d2d';

  if (errorRate === 0) {
    return { fill: '#e8f5e9', stroke: '#81c784', text: darkText }; // soft green
  }
  if (errorRate < 5) {
    return { fill: '#fff3e0', stroke: '#ffb74d', text: darkText }; // soft amber
  }
  return { fill: '#ffebee', stroke: '#e57373', text: darkText }; // soft red
}

/**
 * Generate hex positions in a spiral/honeycomb pattern.
 * Uses axial coordinates (q, r) which convert to cubic (q, r, s=-q-r).
 */
function generateHexPositions(count: number): Array<{ q: number; r: number; s: number }> {
  if (count === 0) return [];

  const positions: Array<{ q: number; r: number; s: number }> = [];

  // Start with center hex
  positions.push({ q: 0, r: 0, s: 0 });
  if (count === 1) return positions;

  // Spiral outward in rings
  let ring = 1;
  while (positions.length < count) {
    // Direction vectors for the 6 sides of each ring
    const directions = [
      { q: 1, r: 0 },   // E
      { q: 0, r: 1 },   // SE
      { q: -1, r: 1 },  // SW
      { q: -1, r: 0 },  // W
      { q: 0, r: -1 },  // NW
      { q: 1, r: -1 },  // NE
    ];

    // Start position for this ring
    let q = 0;
    let r = -ring;

    // Walk around the ring
    for (let side = 0; side < 6 && positions.length < count; side++) {
      const dir = directions[side];
      for (let step = 0; step < ring && positions.length < count; step++) {
        positions.push({ q, r, s: -q - r });
        q += dir.q;
        r += dir.r;
      }
    }
    ring++;
  }

  return positions;
}

/**
 * Truncate service name for display.
 */
function truncateName(name: string, maxLength: number = 10): string {
  if (name.length <= maxLength) return name;
  return name.slice(0, maxLength - 1) + '…';
}

/**
 * Honeycomb grid layout for service cells using react-hexgrid.
 * Uses SVG hexagons with proper honeycomb positioning.
 */
export function HoneycombGrid({
  services,
  selectedService,
  onSelectService,
}: HoneycombGridProps) {
  if (services.length === 0) {
    return (
      <div
        className="rounded-lg p-8 text-center"
        style={{
          backgroundColor: 'var(--color-paper-warm)',
          border: '1px solid var(--color-border)',
        }}
      >
        <p style={{ color: 'var(--color-text-secondary)' }}>No services found.</p>
        <p className="mt-2 text-sm" style={{ color: 'var(--color-text-muted)' }}>
          Services will appear here once they start sending telemetry data.
        </p>
      </div>
    );
  }

  const positions = generateHexPositions(services.length);

  // Calculate viewBox dimensions based on number of services
  const rings = Math.ceil((1 + Math.sqrt(1 + 4 * (services.length - 1) / 3)) / 2);

  // Larger hexagons for better readability
  const hexSize = 16;
  const viewBoxPadding = Math.max(80, rings * 35);

  return (
    <div className="flex justify-center py-6">
      <HexGrid
        width={Math.min(1100, 300 + services.length * 50)}
        height={Math.min(800, 200 + rings * 120)}
        viewBox={`-${viewBoxPadding} -${viewBoxPadding} ${viewBoxPadding * 2} ${viewBoxPadding * 2}`}
      >
        <Layout size={{ x: hexSize, y: hexSize }} flat={true} spacing={1.06} origin={{ x: 0, y: 0 }}>
          {services.map((item, index) => {
            const pos = positions[index];
            const colors = getHealthColor(item.errorRate);
            const isSelected = selectedService === item.service.name;

            // Selected state uses light blue fill with dark text
            const fillColor = isSelected ? '#e3f2fd' : colors.fill;
            const strokeColor = isSelected ? '#1976d2' : colors.stroke;
            const textColor = '#2d2d2d'; // Always dark for legibility

            return (
              <Hexagon
                key={item.service.name}
                q={pos.q}
                r={pos.r}
                s={pos.s}
                onClick={() => onSelectService(item.service.name)}
                className="hexagon-cell"
                style={{
                  fill: fillColor,
                  stroke: strokeColor,
                  strokeWidth: isSelected ? 1.5 : 0.8,
                  cursor: 'pointer',
                }}
              >
                {/* Service name */}
                <Text
                  y={-2}
                  style={{
                    fill: textColor,
                    fontSize: '5px',
                    fontWeight: 300,
                    fontFamily: 'Inter, system-ui, sans-serif',
                    textAnchor: 'middle',
                    dominantBaseline: 'middle',
                    pointerEvents: 'none',
                  }}
                >
                  {truncateName(item.service.name, 12)}
                </Text>

                {/* Error rate */}
                <Text
                  y={4.5}
                  style={{
                    fill: textColor,
                    fontSize: '4.5px',
                    fontWeight: 300,
                    fontFamily: 'Inter, system-ui, sans-serif',
                    textAnchor: 'middle',
                    dominantBaseline: 'middle',
                    pointerEvents: 'none',
                  }}
                >
                  {item.errorRate.toFixed(1)}%
                </Text>

                {/* Signal indicators */}
                <Text
                  y={10}
                  style={{
                    fontSize: '3.5px',
                    fontWeight: 300,
                    fontFamily: 'Inter, system-ui, sans-serif',
                    textAnchor: 'middle',
                    dominantBaseline: 'middle',
                    pointerEvents: 'none',
                  }}
                >
                  <tspan style={{ fill: item.service.has_logs ? '#1565c0' : '#9e9e9e' }}>L</tspan>
                  <tspan>  </tspan>
                  <tspan style={{ fill: item.service.has_traces ? '#7b1fa2' : '#9e9e9e' }}>T</tspan>
                </Text>
              </Hexagon>
            );
          })}
        </Layout>
      </HexGrid>

      <style>{`
        .hexagon-cell:hover {
          filter: brightness(0.95);
        }
        .hexagon-cell {
          transition: filter 0.15s ease, stroke-width 0.15s ease;
        }
      `}</style>
    </div>
  );
}
