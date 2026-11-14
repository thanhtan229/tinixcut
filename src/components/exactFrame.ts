// Helpers for the exact (export-rendered) frame shown over the live preview while paused.
import type { ParityFrame } from "../services/editorApi";
import type { PreviewSampledLayer } from "../types/preview";

export const EXACT_FRAME_STORAGE_KEY = "tinix.exactFrame";
/** Pause time before the exact frame is requested, so scrubbing never starts a render. */
export const EXACT_FRAME_DELAY_MS = 350;

export interface ExactFrameRequestState {
  enabled: boolean;
  isPlaying: boolean;
  /** A canvas drag/crop gesture is in progress: the live layer shows transient state. */
  gestureActive: boolean;
  hasProject: boolean;
  viewportWidth: number;
  devicePixelRatio: number;
}

/** Output width in device pixels, or null when no exact frame should be requested. */
export function exactFrameWidth(state: ExactFrameRequestState): number | null {
  if (!state.enabled || state.isPlaying || state.gestureActive || !state.hasProject) return null;
  const width = Math.round(state.viewportWidth * Math.max(1, state.devicePixelRatio));
  return Number.isFinite(width) && width >= 16 ? width : null;
}

/** The frame still describes what is on the timeline. */
export function exactFrameIsCurrent(
  frame: ParityFrame | null,
  playheadFrame: number,
  revision: number | undefined,
  width: number | null,
): frame is ParityFrame {
  return (
    frame !== null &&
    frame.pngBase64 !== null &&
    frame.frame === playheadFrame &&
    frame.revision === revision &&
    frame.width === width
  );
}

/**
 * z-index that puts the exact frame above every picture layer and below text. The backend only
 * produces a frame when all text sits above all pictures; text with an equal z-index wins by
 * DOM order because the image is rendered first.
 */
export function exactFrameZIndex(layers: Pick<PreviewSampledLayer, "kind" | "z_index">[]): number {
  let top = 0;
  for (const layer of layers) {
    if (layer.kind !== "Text") top = Math.max(top, layer.z_index);
  }
  return top + 1;
}
