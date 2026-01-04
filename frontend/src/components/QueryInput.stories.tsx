import { useState } from 'react'
import type { Story } from '@ladle/react'
import { QueryInput, type QueryInputState } from './QueryInput'

export default {
  title: 'Components/QueryInput',
}

const DEFAULT_SQL = `SELECT *
FROM r2_catalog.default.logs
LIMIT 100`

const TAIL_COMMAND = `TAIL api-gateway logs`

export const Default: Story = () => {
  const [value, setValue] = useState(DEFAULT_SQL)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => console.log('Run:', value)}
      state="idle"
      canRun={true}
    />
  )
}

export const TailCommand: Story = () => {
  const [value, setValue] = useState(TAIL_COMMAND)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => console.log('Run:', value)}
      state="idle"
      isTailCommand={true}
      canRun={true}
    />
  )
}

export const Running: Story = () => {
  const [value, setValue] = useState(DEFAULT_SQL)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => {}}
      state="running"
      canRun={false}
    />
  )
}

export const Connecting: Story = () => {
  const [value, setValue] = useState(TAIL_COMMAND)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => {}}
      state="connecting"
      isTailCommand={true}
      canRun={false}
    />
  )
}

export const Tailing: Story = () => {
  const [value, setValue] = useState(TAIL_COMMAND)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => console.log('Stop tail')}
      state="tailing"
      isTailCommand={true}
      tailRecordCount={127}
    />
  )
}

export const TailingWithDropped: Story = () => {
  const [value, setValue] = useState(TAIL_COMMAND)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => console.log('Stop tail')}
      state="tailing"
      isTailCommand={true}
      tailRecordCount={500}
      droppedCount={23}
    />
  )
}

export const Reconnecting: Story = () => {
  const [value, setValue] = useState(TAIL_COMMAND)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => {}}
      state="reconnecting"
      isTailCommand={true}
      reconnectAttempt={2}
      canRun={false}
    />
  )
}

export const WithResults: Story = () => {
  const [value, setValue] = useState(DEFAULT_SQL)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => console.log('Run:', value)}
      state="idle"
      canRun={true}
      queryTimeMs={342}
      rowCount={100}
    />
  )
}

export const Disabled: Story = () => {
  const [value, setValue] = useState(DEFAULT_SQL)

  return (
    <QueryInput
      value={value}
      onChange={setValue}
      onRun={() => {}}
      state="idle"
      canRun={false}
    />
  )
}

export const Interactive: Story = () => {
  const [value, setValue] = useState(DEFAULT_SQL)
  const [state, setState] = useState<QueryInputState>('idle')
  const [queryTimeMs, setQueryTimeMs] = useState<number | null>(null)
  const [rowCount, setRowCount] = useState<number | null>(null)
  const [tailRecordCount, setTailRecordCount] = useState(0)

  const isTailCommand = value.trim().toUpperCase().startsWith('TAIL')

  const handleRun = () => {
    if (state === 'tailing') {
      setState('idle')
      setTailRecordCount(0)
      return
    }

    if (isTailCommand) {
      setState('connecting')
      setTimeout(() => {
        setState('tailing')
        // Simulate incoming records
        let count = 0
        const interval = setInterval(() => {
          count += Math.floor(Math.random() * 5) + 1
          setTailRecordCount(count)
        }, 500)
        // Store interval for cleanup (in real usage)
        ;(window as any).__tailInterval = interval
      }, 1000)
    } else {
      setState('running')
      setQueryTimeMs(null)
      setRowCount(null)
      setTimeout(() => {
        setState('idle')
        setQueryTimeMs(Math.floor(Math.random() * 500) + 100)
        setRowCount(Math.floor(Math.random() * 1000))
      }, 1500)
    }
  }

  return (
    <div className="space-y-4">
      <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Try running a query or changing to a TAIL command (e.g., "TAIL api-gateway logs")
      </p>
      <QueryInput
        value={value}
        onChange={setValue}
        onRun={handleRun}
        state={state}
        isTailCommand={isTailCommand}
        canRun={state === 'idle'}
        queryTimeMs={queryTimeMs}
        rowCount={rowCount}
        tailRecordCount={tailRecordCount}
      />
    </div>
  )
}
