// frontend/src/lib/parseCommand.ts

export type Signal = 'logs' | 'traces';

export type ParseResult =
  | { type: 'query'; sql: string }
  | { type: 'tail'; service: string; signal: Signal; limit: number }
  | { type: 'error'; message: string };

const TAIL_REGEX = /^tail\s+(\S+)\s+(logs|traces)(?:\s+limit\s+(\d+))?$/i;
const DEFAULT_LIMIT = 500;
const MIN_LIMIT = 1;
const MAX_LIMIT = 10000;

/**
 * Parse user input to determine if it's a SQL query or TAIL command.
 *
 * TAIL syntax: TAIL <service> <signal> [LIMIT <n>]
 * Examples:
 *   TAIL payment-api logs
 *   TAIL auth-service traces LIMIT 1000
 */
export function parseCommand(input: string): ParseResult {
  const trimmed = input.trim();

  if (!trimmed) {
    return { type: 'error', message: 'Input is empty' };
  }

  // Check if it starts with TAIL (case-insensitive)
  if (!trimmed.toLowerCase().startsWith('tail ')) {
    // It's a SQL query
    return { type: 'query', sql: trimmed };
  }

  // Parse TAIL command
  const match = trimmed.match(TAIL_REGEX);
  if (!match) {
    return {
      type: 'error',
      message: 'Invalid TAIL syntax. Use: TAIL <service> <signal> [LIMIT <n>]',
    };
  }

  const [, service, signalStr, limitStr] = match;
  const signal = signalStr.toLowerCase() as Signal;
  const limit = limitStr ? parseInt(limitStr, 10) : DEFAULT_LIMIT;

  if (limit < MIN_LIMIT || limit > MAX_LIMIT) {
    return {
      type: 'error',
      message: `LIMIT must be between ${MIN_LIMIT} and ${MAX_LIMIT}`,
    };
  }

  return { type: 'tail', service, signal, limit };
}

/**
 * Check if input looks like a TAIL command (for UI hints).
 */
export function isTailCommand(input: string): boolean {
  return input.trim().toLowerCase().startsWith('tail ');
}
