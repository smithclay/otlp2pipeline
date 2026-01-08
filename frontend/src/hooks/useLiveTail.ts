// frontend/src/hooks/useLiveTail.ts

import { useRef, useState, useCallback, useEffect } from 'react';
import type { Signal } from '../lib/parseCommand';

export type TailStatus =
  | { state: 'idle' }
  | { state: 'connecting' }
  | { state: 'connected' }
  | { state: 'reconnecting'; attempt: number }
  | { state: 'error'; message: string };

export interface TailRecord {
  [key: string]: unknown;
}

interface WebSocketMessage {
  type: 'connected' | 'record' | 'dropped';
  message?: string;
  data?: TailRecord;
  count?: number;
}

interface UseLiveTailResult {
  start: () => void;
  stop: () => void;
  status: TailStatus;
  records: TailRecord[];
  droppedCount: number;
  parseErrorCount: number;
}

const MAX_RECONNECT_ATTEMPTS = 3;
const RECONNECT_DELAYS = [1000, 2000, 4000]; // Exponential backoff

export function useLiveTail(
  workerUrl: string | null,
  service: string,
  signal: Signal,
  limit: number
): UseLiveTailResult {
  const [status, setStatus] = useState<TailStatus>({ state: 'idle' });
  const [records, setRecords] = useState<TailRecord[]>([]);
  const [droppedCount, setDroppedCount] = useState(0);
  const [parseErrorCount, setParseErrorCount] = useState(0);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptRef = useRef(0);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const isStoppingRef = useRef(false);
  // Use ref for limit so onmessage handler always gets latest value
  const limitRef = useRef(limit);
  limitRef.current = limit;

  const cleanup = useCallback(() => {
    if (reconnectTimeoutRef.current !== null) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
  }, []);

  const connect = useCallback(() => {
    if (!workerUrl) {
      setStatus({ state: 'error', message: 'Worker URL not configured' });
      return;
    }

    cleanup();
    isStoppingRef.current = false;

    // Build WebSocket URL
    const wsUrl = workerUrl
      .replace(/^https:/, 'wss:')
      .replace(/^http:/, 'ws:')
      .replace(/\/$/, '');
    const fullUrl = `${wsUrl}/v1/tail/${encodeURIComponent(service)}/${encodeURIComponent(signal)}`;

    setStatus(
      reconnectAttemptRef.current > 0
        ? { state: 'reconnecting', attempt: reconnectAttemptRef.current }
        : { state: 'connecting' }
    );

    const ws = new WebSocket(fullUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      reconnectAttemptRef.current = 0;
      setStatus({ state: 'connected' });
    };

    ws.onmessage = (event) => {
      try {
        const msg: WebSocketMessage = JSON.parse(event.data);

        if (msg.type === 'record' && msg.data) {
          setRecords((prev) => {
            const next = [...prev, msg.data as TailRecord];
            // Sliding window: remove oldest if over limit (use ref for latest value)
            const currentLimit = limitRef.current;
            if (next.length > currentLimit) {
              return next.slice(next.length - currentLimit);
            }
            return next;
          });
        } else if (msg.type === 'dropped' && msg.count) {
          const count = msg.count;
          setDroppedCount((prev) => prev + count);
        }
        // 'connected' message is handled by onopen
      } catch (err) {
        console.error(
          'Failed to parse WebSocket message:',
          err,
          'Raw:',
          typeof event.data === 'string' ? event.data.slice(0, 100) : event.data
        );
        setParseErrorCount((prev) => prev + 1);
      }
    };

    ws.onerror = () => {
      // Error details not available in browser WebSocket API
      // onclose will fire after this
    };

    ws.onclose = () => {
      if (isStoppingRef.current) {
        setStatus({ state: 'idle' });
        return;
      }

      // Attempt reconnection
      if (reconnectAttemptRef.current < MAX_RECONNECT_ATTEMPTS) {
        const delay = RECONNECT_DELAYS[reconnectAttemptRef.current] ?? 4000;
        reconnectAttemptRef.current += 1;
        setStatus({ state: 'reconnecting', attempt: reconnectAttemptRef.current });

        reconnectTimeoutRef.current = window.setTimeout(() => {
          if (!isStoppingRef.current) {
            connect();
          }
        }, delay);
      } else {
        setStatus({ state: 'error', message: 'Connection failed after 3 attempts' });
        reconnectAttemptRef.current = 0;
      }
    };
  }, [workerUrl, service, signal, cleanup]);

  const start = useCallback(() => {
    setRecords([]);
    setDroppedCount(0);
    setParseErrorCount(0);
    reconnectAttemptRef.current = 0;
    connect();
  }, [connect]);

  const stop = useCallback(() => {
    isStoppingRef.current = true;
    cleanup();
    setStatus({ state: 'idle' });
  }, [cleanup]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      isStoppingRef.current = true;
      cleanup();
    };
  }, [cleanup]);

  return {
    start,
    stop,
    status,
    records,
    droppedCount,
    parseErrorCount,
  };
}
