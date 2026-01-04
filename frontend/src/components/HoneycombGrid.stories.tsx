import { useState } from 'react'
import type { Story } from '@ladle/react'
import { HoneycombGrid } from './HoneycombGrid'
import { mockServicesWithStats, mockServices } from '../stories/mocks'
import type { ServiceWithStats } from './ServiceHealthCards'

export default {
  title: 'Components/HoneycombGrid',
}

// Convert ServiceHealthCards.ServiceWithStats to HoneycombGrid.ServiceWithStats
const toHoneycombFormat = (items: typeof mockServicesWithStats) =>
  items.map((s) => ({
    service: s.service,
    errorRate: s.errorRate,
  }))

export const FewServices: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  return (
    <HoneycombGrid
      services={toHoneycombFormat(mockServicesWithStats.slice(0, 3))}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const ManyServices: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  // Create more services for the grid
  const manyServices: ServiceWithStats[] = [
    ...mockServicesWithStats,
    {
      service: { name: 'order-service', has_logs: true, has_traces: true, has_metrics: false },
      errorRate: 0,
      totalCount: 5000,
      errorCount: 0,
    },
    {
      service: { name: 'shipping-service', has_logs: true, has_traces: false, has_metrics: true },
      errorRate: 2.5,
      totalCount: 3000,
      errorCount: 75,
    },
    {
      service: { name: 'inventory-service', has_logs: true, has_traces: true, has_metrics: true },
      errorRate: 0.5,
      totalCount: 8000,
      errorCount: 40,
    },
    {
      service: { name: 'cache-service', has_logs: false, has_traces: false, has_metrics: true },
      errorRate: 0,
      totalCount: 15000,
      errorCount: 0,
    },
    {
      service: { name: 'queue-worker', has_logs: true, has_traces: false, has_metrics: false },
      errorRate: 1.0,
      totalCount: 2000,
      errorCount: 20,
    },
    {
      service: { name: 'scheduler', has_logs: true, has_traces: true, has_metrics: false },
      errorRate: 0,
      totalCount: 500,
      errorCount: 0,
    },
  ]

  return (
    <HoneycombGrid
      services={toHoneycombFormat(manyServices)}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const MixedHealth: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  const mixedServices: ServiceWithStats[] = [
    {
      service: mockServices[0],
      errorRate: 0,
      totalCount: 10000,
      errorCount: 0,
    },
    {
      service: mockServices[1],
      errorRate: 2.5,
      totalCount: 8000,
      errorCount: 200,
    },
    {
      service: mockServices[2],
      errorRate: 8.0,
      totalCount: 5000,
      errorCount: 400,
    },
    {
      service: mockServices[3],
      errorRate: 0,
      totalCount: 3000,
      errorCount: 0,
    },
    {
      service: mockServices[4],
      errorRate: 15.0,
      totalCount: 1000,
      errorCount: 150,
    },
  ]

  return (
    <HoneycombGrid
      services={toHoneycombFormat(mixedServices)}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const WithSelection: Story = () => {
  const [selected, setSelected] = useState<string | null>('api-gateway')

  return (
    <HoneycombGrid
      services={toHoneycombFormat(mockServicesWithStats)}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const Empty: Story = () => (
  <HoneycombGrid
    services={[]}
    selectedService={null}
    onSelectService={() => {}}
  />
)
