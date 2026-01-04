import type { Story } from '@ladle/react'
import { LoadingSpinner, ErrorMessage } from './LoadingState'

export default {
  title: 'Components/LoadingState',
}

export const Spinner: Story = () => <LoadingSpinner />

export const Error: Story = () => (
  <ErrorMessage
    message="Failed to fetch services. Please check your connection and try again."
    onRetry={() => alert('Retry clicked!')}
  />
)

export const ErrorShort: Story = () => (
  <ErrorMessage
    message="Network error"
    onRetry={() => alert('Retry clicked!')}
  />
)
