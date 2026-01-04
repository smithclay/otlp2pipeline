import type { Story } from '@ladle/react'
import { useToast } from './Toast'

export default {
  title: 'Components/Toast',
}

function ToastTrigger({
  type,
  message,
  action,
}: {
  type: 'info' | 'error' | 'success'
  message: string
  action?: { label: string; to: string }
}) {
  const { showToast } = useToast()

  return (
    <button
      onClick={() => showToast({ type, message, action })}
      className="rounded-md px-4 py-2 text-sm font-medium transition-colors"
      style={{
        backgroundColor: 'var(--color-accent)',
        color: 'white',
      }}
    >
      Show {type} toast
    </button>
  )
}

export const Info: Story = () => (
  <ToastTrigger
    type="info"
    message="Connection established successfully"
  />
)

export const Success: Story = () => (
  <ToastTrigger
    type="success"
    message="Data saved successfully"
  />
)

export const Error: Story = () => (
  <ToastTrigger
    type="error"
    message="Failed to connect to the server. Please check your network connection."
  />
)

export const WithAction: Story = () => (
  <ToastTrigger
    type="info"
    message="New data available for your query"
    action={{ label: 'View results', to: '/records' }}
  />
)

export const AllVariants: Story = () => (
  <div className="space-y-4">
    <ToastTrigger type="info" message="Info toast message" />
    <ToastTrigger type="success" message="Success toast message" />
    <ToastTrigger type="error" message="Error toast message" />
    <ToastTrigger
      type="info"
      message="Toast with action"
      action={{ label: 'Go to settings', to: '/settings' }}
    />
  </div>
)
