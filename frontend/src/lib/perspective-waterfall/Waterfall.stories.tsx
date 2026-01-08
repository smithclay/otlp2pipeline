/**
 * Waterfall Plugin Stories
 *
 * Showcases the custom waterfall trace visualization plugin
 * with various trace structures and states.
 */

import type { Story } from '@ladle/react'
import {
  PerspectiveWrapper,
  PerspectiveStoryContainer,
} from '../../stories/PerspectiveWrapper'
import {
  simpleTraceSpans,
  multiServiceSpans,
  deepNestedSpans,
  errorTraceSpans,
  largeTraceSpans,
} from '../../stories/perspectiveMocks'

// Register the waterfall plugin
import './index'

export default {
  title: 'Perspective/Waterfall Plugin',
}

// Waterfall plugin configuration
const waterfallConfig = {
  plugin: 'Waterfall',
  settings: false,
}

export const SimpleTrace: Story = () => (
  <PerspectiveStoryContainer
    title="Simple Trace"
    description="3-5 spans from a single service showing basic parent-child hierarchy."
  >
    <PerspectiveWrapper
      data={simpleTraceSpans}
      config={waterfallConfig}
      height={250}
    />
  </PerspectiveStoryContainer>
)

export const MultiServiceTrace: Story = () => (
  <PerspectiveStoryContainer
    title="Multi-Service Trace"
    description="8-10 spans across multiple services. Each service gets a consistent color from the palette."
  >
    <PerspectiveWrapper
      data={multiServiceSpans}
      config={waterfallConfig}
      height={350}
    />
  </PerspectiveStoryContainer>
)

export const DeepNesting: Story = () => (
  <PerspectiveStoryContainer
    title="Deep Nesting"
    description="Trace with 5+ depth levels showing hierarchical indentation in the tree panel."
  >
    <PerspectiveWrapper
      data={deepNestedSpans}
      config={waterfallConfig}
      height={350}
    />
  </PerspectiveStoryContainer>
)

export const WithErrors: Story = () => (
  <PerspectiveStoryContainer
    title="Trace With Errors"
    description="Error spans are highlighted with red borders. The root span and some children have errors."
  >
    <PerspectiveWrapper
      data={errorTraceSpans}
      config={waterfallConfig}
      height={300}
    />
  </PerspectiveStoryContainer>
)

export const LargeTrace: Story = () => (
  <PerspectiveStoryContainer
    title="Large Trace (50+ spans)"
    description="Performance test with many spans. Demonstrates scrolling and canvas rendering efficiency."
  >
    <PerspectiveWrapper
      data={largeTraceSpans}
      config={waterfallConfig}
      height={500}
    />
  </PerspectiveStoryContainer>
)

export const EmptyTrace: Story = () => (
  <PerspectiveStoryContainer
    title="Empty Trace"
    description="When no spans are present, the waterfall shows an empty state."
  >
    <PerspectiveWrapper
      data={[]}
      config={waterfallConfig}
      height={200}
    />
  </PerspectiveStoryContainer>
)

export const SingleSpan: Story = () => {
  const singleSpan = [
    {
      trace_id: 'single-001',
      span_id: 'span-root',
      parent_span_id: null,
      service_name: 'standalone-service',
      span_name: 'singleOperation',
      timestamp: Date.now() - 100,
      end_timestamp: Date.now(),
      duration: 100,
      status_code: 'OK',
    },
  ]

  return (
    <PerspectiveStoryContainer
      title="Single Span"
      description="A trace with just one root span, no children."
    >
      <PerspectiveWrapper
        data={singleSpan}
        config={waterfallConfig}
        height={150}
      />
    </PerspectiveStoryContainer>
  )
}

export const MixedStatusCodes: Story = () => {
  const mixedSpans = [
    {
      trace_id: 'mixed-001',
      span_id: 'span-root',
      parent_span_id: null,
      service_name: 'api-gateway',
      span_name: 'handleRequest',
      timestamp: Date.now() - 200,
      end_timestamp: Date.now(),
      duration: 200,
      status_code: 'OK',
    },
    {
      trace_id: 'mixed-001',
      span_id: 'span-ok',
      parent_span_id: 'span-root',
      service_name: 'user-service',
      span_name: 'getUser',
      timestamp: Date.now() - 180,
      end_timestamp: Date.now() - 100,
      duration: 80,
      status_code: 'OK',
    },
    {
      trace_id: 'mixed-001',
      span_id: 'span-error',
      parent_span_id: 'span-root',
      service_name: 'payment-service',
      span_name: 'chargeCard',
      timestamp: Date.now() - 90,
      end_timestamp: Date.now() - 20,
      duration: 70,
      status_code: 'ERROR',
    },
    {
      trace_id: 'mixed-001',
      span_id: 'span-unset',
      parent_span_id: 'span-root',
      service_name: 'notification-service',
      span_name: 'sendEmail',
      timestamp: Date.now() - 15,
      end_timestamp: Date.now() - 5,
      duration: 10,
      status_code: 'UNSET',
    },
  ]

  return (
    <PerspectiveStoryContainer
      title="Mixed Status Codes"
      description="Shows OK (normal), ERROR (red border), and UNSET status codes."
    >
      <PerspectiveWrapper
        data={mixedSpans}
        config={waterfallConfig}
        height={250}
      />
    </PerspectiveStoryContainer>
  )
}
