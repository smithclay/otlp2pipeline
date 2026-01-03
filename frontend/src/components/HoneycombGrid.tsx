import type { Service } from '../lib/api';
import { ServiceCell } from './ServiceCell';

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
 * Honeycomb grid layout for service cells.
 * Uses CSS grid with hexagonal offset pattern.
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

  return (
    <div
      className="flex flex-wrap justify-center gap-x-[-12px] gap-y-[-10px] py-4"
      style={{
        // Negative gap with transform offset creates honeycomb pattern
      }}
    >
      {services.map((item, index) => (
        <div
          key={item.service.name}
          style={{
            // Offset odd rows to create honeycomb effect
            marginTop: index % 2 === 1 ? '14px' : '0',
            marginLeft: index > 0 ? '-12px' : '0',
          }}
        >
          <ServiceCell
            service={item.service}
            errorRate={item.errorRate}
            isSelected={selectedService === item.service.name}
            onClick={() => onSelectService(item.service.name)}
          />
        </div>
      ))}
    </div>
  );
}
