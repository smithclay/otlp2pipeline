import type { Story } from '@ladle/react'
import { Layout } from './Layout'

export default {
  title: 'Components/Layout',
}

export const WithContent: Story = () => (
  <Layout>
    <div className="space-y-6">
      <h1 className="headline text-3xl">Page Title</h1>
      <p style={{ color: 'var(--color-text-secondary)' }}>
        This is an example of the Layout component with some content.
        The layout provides a consistent header with navigation and a main content area.
      </p>
      <div
        className="rounded-lg p-6"
        style={{
          backgroundColor: 'white',
          border: '1px solid var(--color-border)',
        }}
      >
        <p>Content card example</p>
      </div>
    </div>
  </Layout>
)

export const Empty: Story = () => (
  <Layout>
    <div className="flex items-center justify-center py-24">
      <p style={{ color: 'var(--color-text-muted)' }}>
        No content to display
      </p>
    </div>
  </Layout>
)

export const LongContent: Story = () => (
  <Layout>
    <div className="space-y-6">
      <h1 className="headline text-3xl">Long Page</h1>
      {Array.from({ length: 10 }, (_, i) => (
        <div
          key={i}
          className="rounded-lg p-6"
          style={{
            backgroundColor: 'white',
            border: '1px solid var(--color-border)',
          }}
        >
          <h2 className="text-lg font-medium">Section {i + 1}</h2>
          <p className="mt-2" style={{ color: 'var(--color-text-secondary)' }}>
            Lorem ipsum dolor sit amet, consectetur adipiscing elit.
            Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
          </p>
        </div>
      ))}
    </div>
  </Layout>
)
