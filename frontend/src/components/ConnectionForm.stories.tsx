import type { Story } from '@ladle/react'
import { ConnectionForm } from './ConnectionForm'

export default {
  title: 'Components/ConnectionForm',
}

export const Empty: Story = () => (
  <div className="max-w-md">
    <ConnectionForm
      onSave={(credentials) => {
        alert(`Saved: ${JSON.stringify(credentials, null, 2)}`)
      }}
    />
  </div>
)

export const Prefilled: Story = () => (
  <div className="max-w-md">
    <ConnectionForm
      onSave={(credentials) => {
        alert(`Saved: ${JSON.stringify(credentials, null, 2)}`)
      }}
      initialValues={{
        workerUrl: 'https://frostbit.example.workers.dev',
        r2Token: 'abc123def456',
      }}
    />
  </div>
)

export const UpdateMode: Story = () => (
  <div className="max-w-md">
    <ConnectionForm
      onSave={(credentials) => {
        alert(`Updated: ${JSON.stringify(credentials, null, 2)}`)
      }}
      initialValues={{
        workerUrl: 'https://frostbit.example.workers.dev',
        r2Token: 'existing-token',
      }}
      submitLabel="Update Connection"
    />
  </div>
)

export const SaveMode: Story = () => (
  <div className="max-w-md">
    <ConnectionForm
      onSave={(credentials) => {
        alert(`Saved: ${JSON.stringify(credentials, null, 2)}`)
      }}
      submitLabel="Save Settings"
    />
  </div>
)
