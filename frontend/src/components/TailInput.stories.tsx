import { useState } from 'react';
import type { Story } from '@ladle/react';
import { TailInput, type TailSignal } from './TailInput';

export default {
  title: 'Components/TailInput',
};

/**
 * Default state - idle with no service selected.
 * Start button is disabled until a service is chosen.
 */
export const Default: Story = () => {
  const [service, setService] = useState('');
  const [signal, setSignal] = useState<TailSignal>('logs');

  return (
    <TailInput
      service={service}
      signal={signal}
      isStreaming={false}
      onServiceChange={setService}
      onSignalChange={setSignal}
      onStartStop={() => console.log('Start:', service, signal)}
    />
  );
};

/**
 * Service selected, ready to start streaming.
 * Start button is now enabled.
 */
export const WithService: Story = () => {
  const [service, setService] = useState('api-gateway');
  const [signal, setSignal] = useState<TailSignal>('logs');

  return (
    <TailInput
      service={service}
      signal={signal}
      isStreaming={false}
      onServiceChange={setService}
      onSignalChange={setSignal}
      onStartStop={() => console.log('Start:', service, signal)}
    />
  );
};

/**
 * With traces signal selected instead of logs.
 */
export const TracesSelected: Story = () => {
  const [service, setService] = useState('payment-service');
  const [signal, setSignal] = useState<TailSignal>('traces');

  return (
    <TailInput
      service={service}
      signal={signal}
      isStreaming={false}
      onServiceChange={setService}
      onSignalChange={setSignal}
      onStartStop={() => console.log('Start:', service, signal)}
    />
  );
};

/**
 * Streaming state - live indicator visible, Stop button shown.
 * Service and signal controls are disabled during streaming.
 */
export const Streaming: Story = () => {
  return (
    <TailInput
      service="api-gateway"
      signal="logs"
      isStreaming={true}
      onServiceChange={() => {}}
      onSignalChange={() => {}}
      onStartStop={() => console.log('Stop')}
      recordCount={127}
    />
  );
};

/**
 * Streaming with dropped records indicator.
 */
export const StreamingWithDropped: Story = () => {
  return (
    <TailInput
      service="auth-service"
      signal="traces"
      isStreaming={true}
      onServiceChange={() => {}}
      onSignalChange={() => {}}
      onStartStop={() => console.log('Stop')}
      recordCount={500}
      droppedCount={23}
    />
  );
};

/**
 * Fully interactive story - demonstrates complete start/stop flow.
 */
export const Interactive: Story = () => {
  const [service, setService] = useState('');
  const [signal, setSignal] = useState<TailSignal>('logs');
  const [isStreaming, setIsStreaming] = useState(false);
  const [recordCount, setRecordCount] = useState(0);

  const handleStartStop = () => {
    if (isStreaming) {
      // Stop streaming
      setIsStreaming(false);
      setRecordCount(0);
      // Clear any running interval
      if ((window as any).__tailInputInterval) {
        clearInterval((window as any).__tailInputInterval);
        delete (window as any).__tailInputInterval;
      }
    } else {
      // Start streaming
      setIsStreaming(true);
      setRecordCount(0);
      // Simulate incoming records
      const interval = setInterval(() => {
        setRecordCount((prev) => prev + Math.floor(Math.random() * 5) + 1);
      }, 500);
      (window as any).__tailInputInterval = interval;
    }
  };

  return (
    <div className="space-y-4">
      <p className="text-sm" style={{ color: 'var(--color-text-muted)' }}>
        Select a service and click Start to begin streaming. Click Stop to end.
      </p>
      <TailInput
        service={service}
        signal={signal}
        isStreaming={isStreaming}
        onServiceChange={setService}
        onSignalChange={setSignal}
        onStartStop={handleStartStop}
        recordCount={recordCount}
      />
    </div>
  );
};
