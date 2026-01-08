import { useState } from 'react'
import type { Story } from '@ladle/react'
import { ServiceHealthCards } from './ServiceHealthCards'
import {
  mockServicesWithStats,
  mockServices,
  mockDetailStats,
} from '../stories/mocks'
import type { ServiceWithStats } from './ServiceHealthCards'

export default {
  title: 'Components/ServiceHealthCards',
}

export const AllHealthy: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  const healthyServices: ServiceWithStats[] = mockServices.slice(0, 4).map((service) => ({
    service,
    errorRate: 0,
    totalCount: 5000 + Math.floor(Math.random() * 10000),
    errorCount: 0,
  }))

  return (
    <ServiceHealthCards
      services={healthyServices}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const WithWarnings: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  const warningServices: ServiceWithStats[] = [
    {
      service: mockServices[0],
      errorRate: 0,
      totalCount: 12000,
      errorCount: 0,
    },
    {
      service: mockServices[1],
      errorRate: 1.5,
      totalCount: 8000,
      errorCount: 120,
    },
    {
      service: mockServices[2],
      errorRate: 3.2,
      totalCount: 5000,
      errorCount: 160,
    },
    {
      service: mockServices[3],
      errorRate: 0.5,
      totalCount: 2000,
      errorCount: 10,
    },
  ]

  return (
    <ServiceHealthCards
      services={warningServices}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const WithCritical: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  const criticalServices: ServiceWithStats[] = [
    {
      service: mockServices[0],
      errorRate: 0,
      totalCount: 12000,
      errorCount: 0,
    },
    {
      service: mockServices[1],
      errorRate: 8.5,
      totalCount: 8000,
      errorCount: 680,
    },
    {
      service: mockServices[2],
      errorRate: 15.0,
      totalCount: 5000,
      errorCount: 750,
    },
    {
      service: mockServices[3],
      errorRate: 2.0,
      totalCount: 2000,
      errorCount: 40,
    },
  ]

  return (
    <ServiceHealthCards
      services={criticalServices}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}

export const ExpandedWithStats: Story = () => {
  const [selected, setSelected] = useState<string | null>('api-gateway')

  return (
    <ServiceHealthCards
      services={mockServicesWithStats}
      selectedService={selected}
      onSelectService={setSelected}
      detailStats={mockDetailStats}
      detailLoading={false}
    />
  )
}

export const ExpandedLoading: Story = () => {
  const [selected, setSelected] = useState<string | null>('api-gateway')

  return (
    <ServiceHealthCards
      services={mockServicesWithStats}
      selectedService={selected}
      onSelectService={setSelected}
      detailLoading={true}
    />
  )
}

export const Empty: Story = () => (
  <ServiceHealthCards
    services={[]}
    selectedService={null}
    onSelectService={() => {}}
  />
)

export const SingleService: Story = () => {
  const [selected, setSelected] = useState<string | null>(null)

  return (
    <ServiceHealthCards
      services={[mockServicesWithStats[0]]}
      selectedService={selected}
      onSelectService={setSelected}
    />
  )
}
