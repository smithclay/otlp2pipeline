/**
 * Perspective Datagrid Preset Stories
 *
 * Showcases the different configurations and styling options
 * for logs and traces data in query and tail modes.
 */

import type { Story } from '@ladle/react'
import {
  PerspectiveWrapper,
  PerspectiveStoryContainer,
} from '../stories/PerspectiveWrapper'
import { mockLogRecords, mockTraceRecords } from '../stories/perspectiveMocks'
import { createPreset } from './perspectivePresets'

export default {
  title: 'Perspective/Datagrid Presets',
}

// Log columns for preset generation
const logColumns = [
  'timestamp',
  'service_name',
  'severity_number',
  'severity_text',
  'body',
  'trace_id',
  'span_id',
]

// Trace columns for preset generation
const traceColumns = [
  'timestamp',
  'trace_id',
  'span_id',
  'parent_span_id',
  'service_name',
  'span_name',
  'duration_us',
  'duration_ms',
  'status_code',
  'span_kind',
  'events_count',
  'links_count',
]

export const LogsQueryMode: Story = () => {
  const preset = createPreset({ signal: 'logs', mode: 'query' }, logColumns)

  return (
    <PerspectiveStoryContainer
      title="Logs - Query Mode"
      description="Full settings panel enabled, default column order. Severity is color-coded."
    >
      <PerspectiveWrapper
        data={mockLogRecords}
        config={preset}
        height={350}
      />
    </PerspectiveStoryContainer>
  )
}

export const LogsTailMode: Story = () => {
  const preset = createPreset({ signal: 'logs', mode: 'tail' }, logColumns)

  return (
    <PerspectiveStoryContainer
      title="Logs - Tail Mode"
      description="Settings hidden, columns prioritized (timestamp -> severity -> service -> body), sorted descending."
    >
      <PerspectiveWrapper
        data={mockLogRecords}
        config={preset}
        height={350}
      />
    </PerspectiveStoryContainer>
  )
}

export const TracesQueryMode: Story = () => {
  const preset = createPreset({ signal: 'traces', mode: 'query' }, traceColumns)

  return (
    <PerspectiveStoryContainer
      title="Traces - Query Mode"
      description="Duration shown as blue bars, status_code color-coded (red for errors)."
    >
      <PerspectiveWrapper
        data={mockTraceRecords}
        config={preset}
        height={350}
      />
    </PerspectiveStoryContainer>
  )
}

export const TracesTailMode: Story = () => {
  const preset = createPreset({ signal: 'traces', mode: 'tail' }, traceColumns)

  return (
    <PerspectiveStoryContainer
      title="Traces - Tail Mode"
      description="Settings hidden, columns prioritized for live viewing, sorted descending."
    >
      <PerspectiveWrapper
        data={mockTraceRecords}
        config={preset}
        height={350}
      />
    </PerspectiveStoryContainer>
  )
}

export const SeverityColoring: Story = () => {
  // Create records with varying severities to showcase coloring
  const severityRecords = [
    { ...mockLogRecords[0], severity_number: 5, severity_text: 'DEBUG' },
    { ...mockLogRecords[1], severity_number: 9, severity_text: 'INFO' },
    { ...mockLogRecords[2], severity_number: 13, severity_text: 'WARN' },
    { ...mockLogRecords[3], severity_number: 17, severity_text: 'ERROR' },
    { ...mockLogRecords[4], severity_number: 21, severity_text: 'FATAL' },
  ]

  const preset = createPreset({ signal: 'logs', mode: 'query' }, logColumns)

  return (
    <PerspectiveStoryContainer
      title="Severity Coloring"
      description="Higher severity numbers appear in red, lower in green. Shows DEBUG(5) -> FATAL(21) spectrum."
    >
      <PerspectiveWrapper
        data={severityRecords}
        config={preset}
        height={300}
      />
    </PerspectiveStoryContainer>
  )
}

export const DurationBars: Story = () => {
  // Create records with varying durations to showcase bar visualization
  const durationRecords = [
    { ...mockTraceRecords[0], duration_us: 5000, duration_ms: 5 },
    { ...mockTraceRecords[1], duration_us: 25000, duration_ms: 25 },
    { ...mockTraceRecords[2], duration_us: 85000, duration_ms: 85 },
    { ...mockTraceRecords[3], duration_us: 250000, duration_ms: 250 },
  ]

  const preset = createPreset({ signal: 'traces', mode: 'query' }, traceColumns)

  return (
    <PerspectiveStoryContainer
      title="Duration Bars"
      description="Duration columns display as blue gradient bars. Longer durations show longer bars."
    >
      <PerspectiveWrapper
        data={durationRecords}
        config={preset}
        height={300}
      />
    </PerspectiveStoryContainer>
  )
}

export const StatusCodeColoring: Story = () => {
  // Create records with different status codes
  const statusRecords = [
    { ...mockTraceRecords[0], status_code: 0, span_name: 'UNSET status' },
    { ...mockTraceRecords[1], status_code: 1, span_name: 'OK status' },
    { ...mockTraceRecords[2], status_code: 2, span_name: 'ERROR status' },
    { ...mockTraceRecords[3], status_code: 1, span_name: 'Another OK' },
  ]

  const preset = createPreset({ signal: 'traces', mode: 'query' }, traceColumns)

  return (
    <PerspectiveStoryContainer
      title="Status Code Coloring"
      description="Status code 2 (ERROR) appears in red, codes 0-1 (UNSET/OK) in green."
    >
      <PerspectiveWrapper
        data={statusRecords}
        config={preset}
        height={300}
      />
    </PerspectiveStoryContainer>
  )
}

export const ServiceGrouping: Story = () => {
  // Create records from multiple services to show series coloring
  const serviceRecords = mockLogRecords.map((r, i) => ({
    ...r,
    service_name: ['api-gateway', 'user-service', 'payment-service', 'auth-service', 'cache-service'][i % 5],
  }))

  const preset = createPreset({ signal: 'logs', mode: 'query' }, logColumns)

  return (
    <PerspectiveStoryContainer
      title="Service Grouping"
      description="Service names use series coloring for visual grouping. Each service gets a consistent color."
    >
      <PerspectiveWrapper
        data={serviceRecords}
        config={preset}
        height={350}
      />
    </PerspectiveStoryContainer>
  )
}
