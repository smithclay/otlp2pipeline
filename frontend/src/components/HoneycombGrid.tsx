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
 * - Green: 0% error rate
 * - Yellow: >0% and <5% error rate
 * - Red: ≥5% error rate
 */
function getHealthColor(errorRate: number): { fill: string; stroke: string } {
  if (errorRate === 0) {
    return { fill: '#166534', stroke: '#22c55e' }; // green-800, green-500
  }
  if (errorRate < 5) {
    return { fill: '#854d0e', stroke: '#eab308' }; // yellow-800, yellow-500
  }
  return { fill: '#991b1b', stroke: '#ef4444' }; // red-800, red-500
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
      <div className="rounded-lg border border-slate-700 bg-slate-800 p-6 text-center">
        <p className="text-slate-400">No services found.</p>
        <p className="mt-1 text-sm text-slate-500">
          Services will appear here once they start sending telemetry data.
        </p>
      </div>
    );
  }

  const positions = generateHexPositions(services.length);

  // Calculate viewBox dimensions based on number of services
  const rings = Math.ceil((1 + Math.sqrt(1 + 4 * (services.length - 1) / 3)) / 2);
  const viewBoxSize = Math.max(60, rings * 22);

  return (
    <div className="flex justify-center py-4">
      <HexGrid
        width={Math.min(900, 200 + services.length * 40)}
        height={Math.min(700, 150 + rings * 80)}
        viewBox={`-${viewBoxSize} -${viewBoxSize} ${viewBoxSize * 2} ${viewBoxSize * 2}`}
      >
        <Layout size={{ x: 10, y: 10 }} flat={true} spacing={1.08} origin={{ x: 0, y: 0 }}>
          {services.map((item, index) => {
            const pos = positions[index];
            const colors = getHealthColor(item.errorRate);
            const isSelected = selectedService === item.service.name;

            return (
              <Hexagon
                key={item.service.name}
                q={pos.q}
                r={pos.r}
                s={pos.s}
                onClick={() => onSelectService(item.service.name)}
                className="hexagon-cell"
                style={{
                  fill: colors.fill,
                  stroke: isSelected ? '#06b6d4' : colors.stroke,
                  strokeWidth: isSelected ? 1 : 0.5,
                  cursor: 'pointer',
                }}
              >
                {/* Service name */}
                <Text
                  y={-1}
                  style={{
                    fill: '#f1f5f9',
                    fontSize: '3px',
                    fontWeight: 600,
                    textAnchor: 'middle',
                    dominantBaseline: 'middle',
                    pointerEvents: 'none',
                  }}
                >
                  {truncateName(item.service.name)}
                </Text>

                {/* Error rate */}
                <Text
                  y={3}
                  style={{
                    fill: '#cbd5e1',
                    fontSize: '2.5px',
                    textAnchor: 'middle',
                    dominantBaseline: 'middle',
                    pointerEvents: 'none',
                  }}
                >
                  {item.errorRate.toFixed(1)}%
                </Text>

                {/* Signal indicators */}
                <Text
                  y={6}
                  style={{
                    fontSize: '2px',
                    textAnchor: 'middle',
                    dominantBaseline: 'middle',
                    pointerEvents: 'none',
                  }}
                >
                  <tspan style={{ fill: item.service.has_logs ? '#22d3ee' : '#475569' }}>L</tspan>
                  <tspan> </tspan>
                  <tspan style={{ fill: item.service.has_traces ? '#a78bfa' : '#475569' }}>T</tspan>
                </Text>
              </Hexagon>
            );
          })}
        </Layout>
      </HexGrid>

      <style>{`
        .hexagon-cell:hover {
          filter: brightness(1.2);
        }
        .hexagon-cell {
          transition: filter 0.15s ease;
        }
      `}</style>
    </div>
  );
}
