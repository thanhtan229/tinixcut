import { describe, expect, it } from "vitest";
import { createLatestOnlyRunner } from "./latestOnlyRunner";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("createLatestOnlyRunner", () => {
  it("runs one request at a time and drops results superseded while running", async () => {
    const gates = new Map<number, ReturnType<typeof deferred<string>>>();
    const started: number[] = [];
    const delivered: string[] = [];
    const runner = createLatestOnlyRunner(
      (frame: number) => {
        started.push(frame);
        const gate = deferred<string>();
        gates.set(frame, gate);
        return gate.promise;
      },
      { onResult: (r) => delivered.push(r), onError: () => undefined, onBusy: () => undefined }
    );

    runner.request(1);
    runner.request(2);
    runner.request(3); // replaces 2: only the newest queued request survives
    expect(started).toEqual([1]);

    gates.get(1)!.resolve("one");
    await flush();
    expect(started).toEqual([1, 3]);
    expect(delivered).toEqual([]); // "one" is stale and was dropped

    gates.get(3)!.resolve("three");
    await flush();
    expect(delivered).toEqual(["three"]);
  });

  it("reports errors only for the newest request and clears busy", async () => {
    const busy: boolean[] = [];
    const errors: unknown[] = [];
    const runner = createLatestOnlyRunner(
      async (frame: number) => {
        if (frame === 2) throw new Error("boom");
        return frame;
      },
      { onResult: () => undefined, onError: (e) => errors.push(e), onBusy: (b) => busy.push(b) }
    );
    runner.request(2);
    await flush();
    expect(errors).toHaveLength(1);
    expect(busy[busy.length - 1]).toBe(false);
  });

  it("delivers nothing after dispose", async () => {
    const delivered: number[] = [];
    const gate = deferred<number>();
    const runner = createLatestOnlyRunner(() => gate.promise, {
      onResult: (r) => delivered.push(r),
      onError: () => undefined,
      onBusy: () => undefined,
    });
    runner.request(1);
    runner.dispose();
    gate.resolve(7);
    await flush();
    expect(delivered).toEqual([]);
  });
});
