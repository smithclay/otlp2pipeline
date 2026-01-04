/**
 * Mock data for Perspective and Waterfall plugin stories
 */

import type { RawSpan } from '../lib/perspective-waterfall/types'

// ============================================================================
// Log Records for Datagrid Stories
// ============================================================================

const now = Date.now()

export interface MockLogRecord {
  timestamp: number
  service_name: string
  severity_number: number
  severity_text: string
  body: string
  trace_id: string
  span_id: string
}

export const mockLogRecords: MockLogRecord[] = [
  {
    timestamp: now - 1000,
    service_name: 'api-gateway',
    severity_number: 9,
    severity_text: 'INFO',
    body: 'Request received: POST /api/users',
    trace_id: 'trace001',
    span_id: 'span001',
  },
  {
    timestamp: now - 2000,
    service_name: 'user-service',
    severity_number: 9,
    severity_text: 'INFO',
    body: 'Creating new user account',
    trace_id: 'trace001',
    span_id: 'span002',
  },
  {
    timestamp: now - 3000,
    service_name: 'payment-service',
    severity_number: 17,
    severity_text: 'ERROR',
    body: 'Payment gateway timeout after 30s',
    trace_id: 'trace002',
    span_id: 'span003',
  },
  {
    timestamp: now - 4000,
    service_name: 'api-gateway',
    severity_number: 13,
    severity_text: 'WARN',
    body: 'Rate limit approaching for client 192.168.1.1',
    trace_id: 'trace003',
    span_id: 'span004',
  },
  {
    timestamp: now - 5000,
    service_name: 'notification-worker',
    severity_number: 5,
    severity_text: 'DEBUG',
    body: 'Processing email queue batch',
    trace_id: 'trace004',
    span_id: 'span005',
  },
  {
    timestamp: now - 6000,
    service_name: 'auth-service',
    severity_number: 21,
    severity_text: 'FATAL',
    body: 'Database connection pool exhausted',
    trace_id: 'trace005',
    span_id: 'span006',
  },
]

// ============================================================================
// Trace Records for Datagrid Stories
// ============================================================================

export interface MockTraceRecord {
  timestamp: number
  trace_id: string
  span_id: string
  parent_span_id: string | null
  service_name: string
  span_name: string
  duration_us: number
  duration_ms: number
  status_code: number
  span_kind: string
  events_count: number
  links_count: number
}

export const mockTraceRecords: MockTraceRecord[] = [
  {
    timestamp: now - 100,
    trace_id: 'trace001',
    span_id: 'span001',
    parent_span_id: null,
    service_name: 'api-gateway',
    span_name: 'POST /api/users',
    duration_us: 125000,
    duration_ms: 125,
    status_code: 1, // OK
    span_kind: 'server',
    events_count: 2,
    links_count: 0,
  },
  {
    timestamp: now - 200,
    trace_id: 'trace001',
    span_id: 'span002',
    parent_span_id: 'span001',
    service_name: 'user-service',
    span_name: 'createUser',
    duration_us: 85000,
    duration_ms: 85,
    status_code: 1, // OK
    span_kind: 'internal',
    events_count: 0,
    links_count: 0,
  },
  {
    timestamp: now - 300,
    trace_id: 'trace002',
    span_id: 'span003',
    parent_span_id: null,
    service_name: 'payment-service',
    span_name: 'processPayment',
    duration_us: 2500000,
    duration_ms: 2500,
    status_code: 2, // ERROR
    span_kind: 'server',
    events_count: 1,
    links_count: 0,
  },
  {
    timestamp: now - 400,
    trace_id: 'trace003',
    span_id: 'span004',
    parent_span_id: null,
    service_name: 'api-gateway',
    span_name: 'GET /api/products',
    duration_us: 45000,
    duration_ms: 45,
    status_code: 1, // OK
    span_kind: 'server',
    events_count: 0,
    links_count: 1,
  },
]

// ============================================================================
// Waterfall Span Data
// ============================================================================

/**
 * Simple 3-span trace from a single service
 */
export const simpleTraceSpans: RawSpan[] = [
  {
    trace_id: 'simple-trace-001',
    span_id: 'span-root',
    parent_span_id: null,
    service_name: 'api-gateway',
    span_name: 'GET /api/users',
    timestamp: now - 100,
    end_timestamp: now,
    duration: 100,
    status_code: 'OK',
  },
  {
    trace_id: 'simple-trace-001',
    span_id: 'span-child-1',
    parent_span_id: 'span-root',
    service_name: 'api-gateway',
    span_name: 'validateToken',
    timestamp: now - 95,
    end_timestamp: now - 80,
    duration: 15,
    status_code: 'OK',
  },
  {
    trace_id: 'simple-trace-001',
    span_id: 'span-child-2',
    parent_span_id: 'span-root',
    service_name: 'api-gateway',
    span_name: 'fetchUserData',
    timestamp: now - 75,
    end_timestamp: now - 10,
    duration: 65,
    status_code: 'OK',
  },
]

/**
 * Multi-service trace showing 3+ services with color hashing
 */
export const multiServiceSpans: RawSpan[] = [
  {
    trace_id: 'multi-trace-001',
    span_id: 'span-root',
    parent_span_id: null,
    service_name: 'api-gateway',
    span_name: 'POST /api/orders',
    timestamp: now - 250,
    end_timestamp: now,
    duration: 250,
    status_code: 'OK',
  },
  {
    trace_id: 'multi-trace-001',
    span_id: 'span-auth',
    parent_span_id: 'span-root',
    service_name: 'auth-service',
    span_name: 'validateSession',
    timestamp: now - 240,
    end_timestamp: now - 220,
    duration: 20,
    status_code: 'OK',
  },
  {
    trace_id: 'multi-trace-001',
    span_id: 'span-user',
    parent_span_id: 'span-root',
    service_name: 'user-service',
    span_name: 'getUser',
    timestamp: now - 215,
    end_timestamp: now - 180,
    duration: 35,
    status_code: 'OK',
  },
  {
    trace_id: 'multi-trace-001',
    span_id: 'span-inventory',
    parent_span_id: 'span-root',
    service_name: 'inventory-service',
    span_name: 'checkStock',
    timestamp: now - 175,
    end_timestamp: now - 120,
    duration: 55,
    status_code: 'OK',
  },
  {
    trace_id: 'multi-trace-001',
    span_id: 'span-payment',
    parent_span_id: 'span-root',
    service_name: 'payment-service',
    span_name: 'processPayment',
    timestamp: now - 115,
    end_timestamp: now - 30,
    duration: 85,
    status_code: 'OK',
  },
  {
    trace_id: 'multi-trace-001',
    span_id: 'span-notification',
    parent_span_id: 'span-root',
    service_name: 'notification-service',
    span_name: 'sendConfirmation',
    timestamp: now - 25,
    end_timestamp: now - 5,
    duration: 20,
    status_code: 'OK',
  },
]

/**
 * Deep nested trace (5+ depth levels)
 */
export const deepNestedSpans: RawSpan[] = [
  {
    trace_id: 'deep-trace-001',
    span_id: 'depth-0',
    parent_span_id: null,
    service_name: 'api-gateway',
    span_name: 'handleRequest',
    timestamp: now - 300,
    end_timestamp: now,
    duration: 300,
    status_code: 'OK',
  },
  {
    trace_id: 'deep-trace-001',
    span_id: 'depth-1',
    parent_span_id: 'depth-0',
    service_name: 'api-gateway',
    span_name: 'middleware.auth',
    timestamp: now - 290,
    end_timestamp: now - 10,
    duration: 280,
    status_code: 'OK',
  },
  {
    trace_id: 'deep-trace-001',
    span_id: 'depth-2',
    parent_span_id: 'depth-1',
    service_name: 'user-service',
    span_name: 'userController.get',
    timestamp: now - 280,
    end_timestamp: now - 20,
    duration: 260,
    status_code: 'OK',
  },
  {
    trace_id: 'deep-trace-001',
    span_id: 'depth-3',
    parent_span_id: 'depth-2',
    service_name: 'user-service',
    span_name: 'userRepository.find',
    timestamp: now - 270,
    end_timestamp: now - 30,
    duration: 240,
    status_code: 'OK',
  },
  {
    trace_id: 'deep-trace-001',
    span_id: 'depth-4',
    parent_span_id: 'depth-3',
    service_name: 'user-service',
    span_name: 'database.query',
    timestamp: now - 260,
    end_timestamp: now - 40,
    duration: 220,
    status_code: 'OK',
  },
  {
    trace_id: 'deep-trace-001',
    span_id: 'depth-5',
    parent_span_id: 'depth-4',
    service_name: 'user-service',
    span_name: 'connection.execute',
    timestamp: now - 250,
    end_timestamp: now - 50,
    duration: 200,
    status_code: 'OK',
  },
]

/**
 * Trace with error spans (red borders)
 */
export const errorTraceSpans: RawSpan[] = [
  {
    trace_id: 'error-trace-001',
    span_id: 'span-root',
    parent_span_id: null,
    service_name: 'api-gateway',
    span_name: 'POST /api/checkout',
    timestamp: now - 500,
    end_timestamp: now,
    duration: 500,
    status_code: 'ERROR',
  },
  {
    trace_id: 'error-trace-001',
    span_id: 'span-auth',
    parent_span_id: 'span-root',
    service_name: 'auth-service',
    span_name: 'validateToken',
    timestamp: now - 490,
    end_timestamp: now - 470,
    duration: 20,
    status_code: 'OK',
  },
  {
    trace_id: 'error-trace-001',
    span_id: 'span-cart',
    parent_span_id: 'span-root',
    service_name: 'cart-service',
    span_name: 'getCart',
    timestamp: now - 460,
    end_timestamp: now - 400,
    duration: 60,
    status_code: 'OK',
  },
  {
    trace_id: 'error-trace-001',
    span_id: 'span-payment',
    parent_span_id: 'span-root',
    service_name: 'payment-service',
    span_name: 'chargeCard',
    timestamp: now - 390,
    end_timestamp: now - 50,
    duration: 340,
    status_code: 'ERROR',
  },
  {
    trace_id: 'error-trace-001',
    span_id: 'span-payment-retry',
    parent_span_id: 'span-payment',
    service_name: 'payment-service',
    span_name: 'retryCharge',
    timestamp: now - 200,
    end_timestamp: now - 60,
    duration: 140,
    status_code: 'ERROR',
  },
]

/**
 * Large trace (50+ spans) for performance/scrolling test
 */
export function generateLargeTrace(spanCount: number = 50): RawSpan[] {
  const spans: RawSpan[] = []
  const services = [
    'api-gateway',
    'user-service',
    'auth-service',
    'payment-service',
    'notification-service',
    'inventory-service',
    'analytics-service',
  ]
  const operations = [
    'handleRequest',
    'validateInput',
    'processData',
    'queryDatabase',
    'callExternalApi',
    'cacheResult',
    'logMetrics',
    'sendResponse',
  ]

  const totalDuration = 1000

  // Root span
  spans.push({
    trace_id: 'large-trace-001',
    span_id: 'span-0',
    parent_span_id: null,
    service_name: services[0],
    span_name: 'rootOperation',
    timestamp: now - totalDuration,
    end_timestamp: now,
    duration: totalDuration,
    status_code: 'OK',
  })

  // Generate child spans with varying depths
  for (let i = 1; i < spanCount; i++) {
    const parentIndex = Math.floor(Math.random() * Math.min(i, 5))
    const service = services[i % services.length]
    const operation = operations[i % operations.length]
    const startOffset = Math.floor((i / spanCount) * totalDuration * 0.9)
    const duration = Math.floor(Math.random() * 50) + 10
    const isError = Math.random() < 0.05 // 5% error rate

    spans.push({
      trace_id: 'large-trace-001',
      span_id: `span-${i}`,
      parent_span_id: `span-${parentIndex}`,
      service_name: service,
      span_name: `${operation}_${i}`,
      timestamp: now - totalDuration + startOffset,
      end_timestamp: now - totalDuration + startOffset + duration,
      duration,
      status_code: isError ? 'ERROR' : 'OK',
    })
  }

  return spans
}

export const largeTraceSpans = generateLargeTrace(50)
