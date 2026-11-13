/**
 * J/K/L shuttle state machine. HTML video cannot play backwards, so reverse (and fast-forward
 * above 1x) is done by stepping the playhead on a timer with the audio muted; plain 1x forward
 * uses normal playback. The pure parts below decide what each key does and how far each tick
 * moves, so the behaviour is testable without a DOM.
 */

export type ShuttleDirection = -1 | 0 | 1;
export type ShuttleRate = 1 | 2 | 4;

export interface ShuttleState {
  direction: ShuttleDirection;
  rate: ShuttleRate;
}

export const SHUTTLE_IDLE: ShuttleState = { direction: 0, rate: 1 };
/** One stepping tick every ~33 ms (30 per second). */
export const SHUTTLE_TICK_MS = 33;

function faster(rate: ShuttleRate): ShuttleRate {
  return rate === 1 ? 2 : 4;
}

/** K stops; pressing a direction again raises the speed, pressing the other one reverses. */
export function nextShuttle(state: ShuttleState, key: "J" | "K" | "L"): ShuttleState {
  if (key === "K") return SHUTTLE_IDLE;
  const direction: ShuttleDirection = key === "L" ? 1 : -1;
  if (state.direction === direction) return { direction, rate: faster(state.rate) };
  return { direction, rate: 1 };
}

/** Frames the playhead moves per tick; 1x forward is real playback and does not step. */
export function shuttleFramesPerTick(state: ShuttleState): number {
  if (state.direction === 0) return 0;
  if (state.direction === 1 && state.rate === 1) return 0;
  return state.direction * state.rate;
}

export function usesRealPlayback(state: ShuttleState): boolean {
  return state.direction === 1 && state.rate === 1;
}

/** Playhead after one tick, clamped to the timeline; reports when the end was reached. */
export function stepPlayhead(
  playhead: number,
  state: ShuttleState,
  lastFrame: number
): { playhead: number; stopped: boolean } {
  const next = playhead + shuttleFramesPerTick(state);
  const clamped = Math.min(Math.max(next, 0), Math.max(0, lastFrame));
  return { playhead: clamped, stopped: clamped !== next };
}

export function describeShuttle(state: ShuttleState): string {
  if (state.direction === 0) return "";
  return `${state.direction === 1 ? "▶" : "◀"} ${state.rate}×`;
}
