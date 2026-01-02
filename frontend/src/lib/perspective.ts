/**
 * Perspective worker setup
 *
 * Perspective is a high-performance data visualization library that uses
 * WebAssembly and web workers for efficient rendering.
 */

import perspective, { type PerspectiveWorker } from '@finos/perspective';

// Singleton worker instance
let worker: PerspectiveWorker | null = null;

/**
 * Get the shared Perspective worker instance.
 * Creates the worker on first call, returns cached instance thereafter.
 */
export async function getPerspectiveWorker(): Promise<PerspectiveWorker> {
  if (!worker) {
    worker = await perspective.worker();
  }
  return worker;
}

/**
 * Cleanup the Perspective worker when no longer needed.
 * Call this during app cleanup if necessary.
 */
export async function terminatePerspectiveWorker(): Promise<void> {
  if (worker) {
    await worker.terminate();
    worker = null;
  }
}
