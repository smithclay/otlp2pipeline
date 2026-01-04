import { useState } from 'react'
import type { Story } from '@ladle/react'
import { TimeRangePicker } from './TimeRangePicker'
import { TIME_RANGES, TimeRange } from '../hooks/useStats'

export default {
  title: 'Components/TimeRangePicker',
}

export const Default: Story = () => {
  const [value, setValue] = useState<TimeRange>(TIME_RANGES[0])

  return (
    <div className="space-y-4">
      <TimeRangePicker value={value} onChange={setValue} />
      <p className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
        Selected: {value.label}
      </p>
    </div>
  )
}

export const Last24Hours: Story = () => {
  const [value, setValue] = useState<TimeRange>(TIME_RANGES[3]) // 24h

  return <TimeRangePicker value={value} onChange={setValue} />
}

export const Last7Days: Story = () => {
  const [value, setValue] = useState<TimeRange>(TIME_RANGES[4]) // 7d

  return <TimeRangePicker value={value} onChange={setValue} />
}
