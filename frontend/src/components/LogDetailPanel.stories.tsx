import { useState } from 'react';
import type { Story } from '@ladle/react';
import { LogDetailPanel, type LogRecord } from './LogDetailPanel';

export default {
  title: 'Components/LogDetailPanel',
};

// ============================================================================
// Mock Data
// ============================================================================

const mockInfoLog: LogRecord = {
  timestamp: new Date().toISOString(),
  severity: 'INFO',
  message: 'User authentication successful',
  service: 'auth-service',
  trace_id: 'abc123def456789012345678',
  span_id: 'span001234567890',
  host: 'auth-service-pod-7d8f9c',
  attributes: {
    'user.id': 'usr_12345',
    'auth.method': 'oauth2',
    'session.duration_ms': 1500,
  },
  resource_attributes: {
    'service.name': 'auth-service',
    'service.version': '2.1.0',
    'deployment.environment': 'production',
    'k8s.pod.name': 'auth-service-pod-7d8f9c',
  },
};

const mockErrorLog: LogRecord = {
  timestamp: new Date().toISOString(),
  severity: 'ERROR',
  message: `Failed to process payment: Gateway timeout after 30000ms.
Stack trace:
  at PaymentGateway.charge (payment.ts:142)
  at PaymentService.processPayment (service.ts:89)
  at OrderController.checkout (controller.ts:56)
  at Router.handle (router.ts:23)`,
  service: 'payment-service',
  trace_id: 'xyz789abc123456789012345',
  span_id: 'span987654321098',
  host: 'payment-service-pod-3a4b5c',
  attributes: {
    'error.type': 'GatewayTimeoutError',
    'error.code': 'GATEWAY_TIMEOUT',
    'payment.amount': 99.99,
    'payment.currency': 'USD',
    'payment.method': 'credit_card',
    'retry.count': 3,
    'gateway.response_time_ms': 30000,
  },
  resource_attributes: {
    'service.name': 'payment-service',
    'service.version': '1.5.2',
    'deployment.environment': 'production',
    'k8s.namespace': 'payments',
    'k8s.pod.name': 'payment-service-pod-3a4b5c',
    'cloud.provider': 'aws',
    'cloud.region': 'us-east-1',
  },
};

const mockWarnLog: LogRecord = {
  timestamp: new Date().toISOString(),
  severity: 'WARN',
  message: 'Rate limit approaching threshold: 85% capacity',
  service: 'api-gateway',
  host: 'gateway-pod-1x2y3z',
  attributes: {
    'rate_limit.current': 850,
    'rate_limit.max': 1000,
    'rate_limit.window': '1m',
    'client.ip': '192.168.1.100',
  },
};

const mockDebugLog: LogRecord = {
  timestamp: new Date().toISOString(),
  severity: 'DEBUG',
  message: 'Cache hit for key: user_profile_12345',
  service: 'cache-service',
  attributes: {
    'cache.key': 'user_profile_12345',
    'cache.ttl_remaining_ms': 45000,
  },
};

const mockLogWithTraceId: LogRecord = {
  timestamp: new Date().toISOString(),
  severity: 'INFO',
  message: 'Request processed successfully',
  service: 'api-gateway',
  trace_id: 'trace-id-click-me-12345',
  span_id: 'span-abc-123',
  host: 'gateway-pod-7x8y9z',
  attributes: {
    'http.method': 'POST',
    'http.path': '/api/orders',
    'http.status_code': 201,
    'response.time_ms': 45,
  },
};

// ============================================================================
// Panel Wrapper
// ============================================================================

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
  );
}

// ============================================================================
// Stories
// ============================================================================

export const InfoLog: Story = () => {
  return (
    <PanelWrapper>
      <LogDetailPanel log={mockInfoLog} onClose={() => {}} />
    </PanelWrapper>
  );
};
InfoLog.meta = {
  description: 'Basic INFO level log with standard attributes',
};

export const ErrorLog: Story = () => {
  return (
    <PanelWrapper>
      <LogDetailPanel log={mockErrorLog} onClose={() => {}} />
    </PanelWrapper>
  );
};
ErrorLog.meta = {
  description: 'ERROR level log with stack trace and full attributes',
};

export const WarnLog: Story = () => {
  return (
    <PanelWrapper>
      <LogDetailPanel log={mockWarnLog} onClose={() => {}} />
    </PanelWrapper>
  );
};
WarnLog.meta = {
  description: 'WARNING level log',
};

export const DebugLog: Story = () => {
  return (
    <PanelWrapper>
      <LogDetailPanel log={mockDebugLog} onClose={() => {}} />
    </PanelWrapper>
  );
};
DebugLog.meta = {
  description: 'DEBUG level log with minimal attributes',
};

export const WithTraceId: Story = () => {
  const handleTraceClick = (traceId: string) => {
    alert(`Navigate to trace: ${traceId}`);
  };

  return (
    <PanelWrapper>
      <LogDetailPanel
        log={mockLogWithTraceId}
        onClose={() => {}}
        onTraceClick={handleTraceClick}
      />
    </PanelWrapper>
  );
};
WithTraceId.meta = {
  description: 'Log with clickable trace_id that triggers navigation',
};

export const MinimalLog: Story = () => {
  const minimalLog: LogRecord = {
    timestamp: new Date().toISOString(),
    severity: 'INFO',
    message: 'Simple log message without extras',
    service: 'simple-service',
  };

  return (
    <PanelWrapper>
      <LogDetailPanel log={minimalLog} onClose={() => {}} />
    </PanelWrapper>
  );
};
MinimalLog.meta = {
  description: 'Minimal log with only required fields',
};

export const Hidden: Story = () => (
  <PanelWrapper>
    <LogDetailPanel log={null} onClose={() => {}} />
  </PanelWrapper>
);
Hidden.meta = {
  description: 'Panel when no log is selected (hidden state)',
};

export const Interactive: Story = () => {
  const [log, setLog] = useState<LogRecord | null>(null);
  const logs = [mockInfoLog, mockErrorLog, mockWarnLog, mockDebugLog];
  const [selectedIndex, setSelectedIndex] = useState(0);

  const handleTraceClick = (traceId: string) => {
    alert(`Navigate to trace: ${traceId}`);
  };

  return (
    <div className="space-y-4">
      <div className="flex gap-2 flex-wrap">
        {logs.map((l, i) => (
          <button
            key={i}
            onClick={() => {
              setSelectedIndex(i);
              setLog(l);
            }}
            className="px-3 py-1.5 rounded text-sm font-medium transition-colors"
            style={{
              backgroundColor: selectedIndex === i && log ? 'var(--color-accent)' : 'var(--color-paper)',
              color: selectedIndex === i && log ? 'white' : 'var(--color-text-primary)',
              border: '1px solid var(--color-border)',
            }}
          >
            {l.severity} Log
          </button>
        ))}
        <button
          onClick={() => setLog(null)}
          className="px-3 py-1.5 rounded text-sm font-medium"
          style={{
            backgroundColor: 'var(--color-paper)',
            color: 'var(--color-text-secondary)',
            border: '1px solid var(--color-border)',
          }}
        >
          Close Panel
        </button>
      </div>

      <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Press Escape or click outside the panel to close
      </p>

      <PanelWrapper>
        <LogDetailPanel
          log={log}
          onClose={() => setLog(null)}
          onTraceClick={handleTraceClick}
        />
      </PanelWrapper>
    </div>
  );
};
Interactive.meta = {
  description: 'Interactive demo with open/close functionality and keyboard support',
};
