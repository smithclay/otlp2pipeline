import type { Story } from '@ladle/react'
import { CatalogOverview } from './CatalogOverview'
import { mockCatalogStats, mockEmptyCatalogStats } from '../stories/mocks'

export default {
  title: 'Components/CatalogOverview',
}

export const Loading: Story = () => (
  <CatalogOverview
    stats={null}
    isLoading={true}
    error={null}
    onRefresh={() => {}}
  />
)

export const Loaded: Story = () => (
  <CatalogOverview
    stats={mockCatalogStats}
    isLoading={false}
    error={null}
    onRefresh={() => alert('Refresh clicked!')}
  />
)

export const WithError: Story = () => (
  <CatalogOverview
    stats={null}
    isLoading={false}
    error="Failed to connect to the Iceberg catalog. Check your R2 credentials and try again."
    onRefresh={() => alert('Refresh clicked!')}
  />
)

export const PartialError: Story = () => (
  <CatalogOverview
    stats={mockCatalogStats}
    isLoading={false}
    error="Loaded 2 tables. Failed to load metadata for 1 table(s)."
    onRefresh={() => alert('Refresh clicked!')}
  />
)

export const Empty: Story = () => (
  <CatalogOverview
    stats={mockEmptyCatalogStats}
    isLoading={false}
    error={null}
    onRefresh={() => alert('Refresh clicked!')}
  />
)

export const NoCredentials: Story = () => (
  <CatalogOverview
    stats={null}
    isLoading={false}
    error={null}
    onRefresh={() => alert('Refresh clicked!')}
  />
)
