import { Project, rationalTimeToFrames } from "../types/project";

/** The exclusive end frame shared by seeking and continuous playback. */
export function timelineEndFrame(project: Project | null): number {
  if (!project) return 0;
  const rate = project.settings.frame_rate;
  return project.tracks.reduce((end, track) => track.items.reduce((trackEnd, item) =>
    Math.max(trackEnd, rationalTimeToFrames(item.timeline_range.start, rate)
      + rationalTimeToFrames(item.timeline_range.duration, rate)), end), 0);
}

export type FrameSeekParseResult =
  | { frame: number; error: null }
  | { frame: null; error: string };

export function resolveFrameSeekInputValue(
  inputValue: string | undefined,
  draftValue: string | null,
  playheadFrame: number,
): string {
  return inputValue ?? draftValue ?? String(playheadFrame);
}

export function shouldCommitFrameSeekOnBlur(
  inputValue: string,
  playheadFrame: number,
  isDirty: boolean,
): boolean {
  return isDirty || inputValue !== String(playheadFrame);
}

export function parseFrameSeekInput(value: string, maxFrame: number): FrameSeekParseResult {
  const normalized = value.trim();
  if (!/^\d+$/.test(normalized)) {
    return { frame: null, error: "Enter a whole frame number." };
  }

  const frame = Number(normalized);
  if (!Number.isSafeInteger(frame)) {
    return { frame: null, error: "Frame number is too large." };
  }

  const lastFrame = Math.max(0, Math.floor(maxFrame));
  if (frame > lastFrame) {
    return { frame: null, error: `Enter a frame from 0 to ${lastFrame}.` };
  }

  return { frame, error: null };
}
