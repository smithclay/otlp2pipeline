import { useState } from 'react'
import type { Story } from '@ladle/react'
import { SpanDetailsPanel } from './SpanDetailsPanel'
import { mockLayoutSpan, mockErrorSpan } from '../stories/mocks'

export default {
  title: 'Components/SpanDetailsPanel',
}

// Wrapper to position the panel correctly
function PanelWrapper({ children }: { children: React.ReactNode }) {
  return (
    <div
      className="relative w-full"
      style={{ height: '600px', backgroundColor: 'var(--color-paper-warm)' }}
    >
      <div className="absolute inset-0 flex items-center justify-center">
        <p style={{ color: 'var(--color-text-muted)' }}>
          Panel slides in from the right
        </p>
      </div>
      {children}
    </div>
  )
}

export const WithSpan: Story = () => {
  const [span, setSpan] = useState(mockLayoutSpan)

  return (
    <PanelWrapper>
      <SpanDetailsPanel span={span} onClose={() => setSpan(null as any)} />
    </PanelWrapper>
  )
}

export const ErrorSpan: Story = () => {
  const [span, setSpan] = useState(mockErrorSpan)

  return (
    <PanelWrapper>
      <SpanDetailsPanel span={span} onClose={() => setSpan(null as any)} />
    </PanelWrapper>
  )
}

export const Hidden: Story = () => (
  <PanelWrapper>
    <SpanDetailsPanel span={null} onClose={() => {}} />
  </PanelWrapper>
)

export const TogglePanel: Story = () => {
  const [span] = useState(mockLayoutSpan)
  const [visible, setVisible] = useState(true)

  return (
    <div className="space-y-4">
      <button
        onClick={() => setVisible(!visible)}
        className="px-4 py-2 rounded-md text-sm font-medium"
        style={{
          backgroundColor: 'var(--color-accent)',
          color: 'white',
        }}
      >
        {visible ? 'Hide Panel' : 'Show Panel'}
      </button>

      <PanelWrapper>
        <SpanDetailsPanel
          span={visible ? span : null}
          onClose={() => setVisible(false)}
        />
      </PanelWrapper>
    </div>
  )
}

export const MinimalAttributes: Story = () => {
  const minimalSpan = {
    ...mockLayoutSpan,
    span_attributes: undefined,
    resource_attributes: undefined,
    scope_attributes: undefined,
  }

  return (
    <PanelWrapper>
      <SpanDetailsPanel span={minimalSpan} onClose={() => {}} />
    </PanelWrapper>
  )
}
