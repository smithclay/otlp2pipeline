/**
 * Wrapper component for Perspective stories.
 * Handles async worker initialization and table loading.
 */

import { useEffect, useRef, useState, type ReactNode } from 'react'
import perspective, { type Table, type TableData } from '@finos/perspective'
import '@finos/perspective-viewer'
import '@finos/perspective-viewer-datagrid'
import '@finos/perspective-viewer-d3fc'
import '@finos/perspective-viewer/dist/css/themes.css'
import type { HTMLPerspectiveViewerElement } from '@finos/perspective-viewer'

interface PerspectiveWrapperProps {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: readonly Record<string, any>[]
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  config?: Record<string, any>
  height?: number
  onViewerReady?: (viewer: HTMLPerspectiveViewerElement) => void
}

export function PerspectiveWrapper({
  data,
  config,
  height = 400,
  onViewerReady,
}: PerspectiveWrapperProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const viewerRef = useRef<HTMLPerspectiveViewerElement | null>(null)
  const tableRef = useRef<Table | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let mounted = true

    async function init() {
      if (!containerRef.current) return

      try {
        // Create viewer element
        const viewer = document.createElement('perspective-viewer') as HTMLPerspectiveViewerElement
        viewer.style.width = '100%'
        viewer.style.height = '100%'
        containerRef.current.appendChild(viewer)
        viewerRef.current = viewer

        // Create worker and table
        const worker = await perspective.worker()
        const table = await worker.table(data as unknown as TableData)
        tableRef.current = table

        // Load data into viewer
        await viewer.load(table)

        // Apply config if provided
        if (config) {
          await viewer.restore(config)
        }

        if (mounted) {
          setLoading(false)
          onViewerReady?.(viewer)
        }
      } catch (err) {
        if (mounted) {
          setError(err instanceof Error ? err.message : 'Failed to initialize Perspective')
          setLoading(false)
        }
      }
    }

    init()

    return () => {
      mounted = false
      // Cleanup
      if (viewerRef.current && containerRef.current?.contains(viewerRef.current)) {
        containerRef.current.removeChild(viewerRef.current)
      }
      viewerRef.current = null
      tableRef.current?.delete()
      tableRef.current = null
    }
  }, []) // Only run once on mount

  // Update data when it changes
  useEffect(() => {
    if (tableRef.current && data.length > 0) {
      tableRef.current.replace(data as unknown as TableData)
    }
  }, [data])

  // Update config when it changes
  useEffect(() => {
    if (viewerRef.current && config) {
      viewerRef.current.restore(config)
    }
  }, [config])

  if (error) {
    return (
      <div
        className="flex items-center justify-center p-8 rounded-lg"
        style={{
          height,
          backgroundColor: 'var(--color-error-bg)',
          border: '1px solid var(--color-error)',
        }}
      >
        <p style={{ color: 'var(--color-error)' }}>{error}</p>
      </div>
    )
  }

  // Handle empty data case - Perspective can't infer schema from empty array
  if (data.length === 0) {
    return (
      <div
        className="flex items-center justify-center p-8 rounded-lg"
        style={{
          height,
          backgroundColor: 'var(--color-bg-secondary, #f5f5f5)',
          border: '1px solid var(--color-border, #e0e0e0)',
        }}
      >
        <p style={{ color: 'var(--color-text-secondary, #666)' }}>No data to display</p>
      </div>
    )
  }

  return (
    <div
      ref={containerRef}
      className="relative rounded-lg overflow-hidden"
      style={{
        height,
        backgroundColor: 'white',
        border: '1px solid var(--color-border)',
      }}
    >
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-white/80">
          <div
            className="h-8 w-8 animate-spin rounded-full border-2"
            style={{
              borderColor: 'var(--color-border)',
              borderTopColor: 'var(--color-accent)',
            }}
          />
        </div>
      )}
    </div>
  )
}

interface PerspectiveStoryContainerProps {
  title: string
  description?: string
  children: ReactNode
}

export function PerspectiveStoryContainer({
  title,
  description,
  children,
}: PerspectiveStoryContainerProps) {
  return (
    <div className="space-y-4">
      <div>
        <h3
          className="text-lg font-semibold"
          style={{ color: 'var(--color-text-primary)' }}
        >
          {title}
        </h3>
        {description && (
          <p
            className="text-sm mt-1"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            {description}
          </p>
        )}
      </div>
      {children}
    </div>
  )
}
