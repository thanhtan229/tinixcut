export interface LatestOnlyHandlers<T> {
  onResult: (result: T) => void;
  onError: (error: unknown) => void;
  onBusy: (busy: boolean) => void;
}

/**
 * Runs one request at a time. While one is in flight only the newest later request is kept;
 * the in-flight result is then stale and is dropped instead of delivered. This bounds pending
 * work to one running + one queued job, however fast the playhead moves.
 */
export function createLatestOnlyRunner<A, T>(
  run: (argument: A) => Promise<T>,
  handlers: LatestOnlyHandlers<T>
): { request: (argument: A) => void; dispose: () => void } {
  let inFlight = false;
  let pending: { argument: A } | null = null;
  let disposed = false;

  const start = async (argument: A): Promise<void> => {
    inFlight = true;
    handlers.onBusy(true);
    let result: { value: T } | { error: unknown };
    try {
      result = { value: await run(argument) };
    } catch (error) {
      result = { error };
    }
    inFlight = false;
    const next = pending;
    pending = null;
    if (disposed) return;
    if (next) {
      // A newer request is waiting; this result describes an older frame.
      await start(next.argument);
      return;
    }
    if ("value" in result) handlers.onResult(result.value);
    else handlers.onError(result.error);
    handlers.onBusy(false);
  };

  return {
    request: (argument) => {
      if (disposed) return;
      if (inFlight) pending = { argument };
      else void start(argument);
    },
    dispose: () => {
      disposed = true;
      pending = null;
    },
  };
}
