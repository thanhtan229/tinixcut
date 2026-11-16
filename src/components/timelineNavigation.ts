import { FrameRate, Project, rationalTimeToFrames } from "../types/project";

export function adjacentMarkerFrame(project: Project | null, playhead: number, direction: 1 | -1): number | null {
  if (!project) return null;
  const frames=(project.markers ?? []).map(marker=>rationalTimeToFrames(marker.position,project.settings.frame_rate));
  const eligible=frames.filter(frame=>direction===1 ? frame>playhead : frame<playhead);
  return eligible.length ? (direction===1 ? Math.min(...eligible) : Math.max(...eligible)) : null;
}

export const TIMELINE_HEADER_WIDTH = 160;
export const DEFAULT_TIMELINE_ZOOM = 3;
export const MIN_TIMELINE_ZOOM = 0.000001;
export const MAX_TIMELINE_ZOOM = 10;
const FIT_PADDING = 24;
export interface TimelineViewRange { start: number; end: number }

export function timelineViewRange(project: Project | null, selected?: ReadonlySet<string>): TimelineViewRange | null {
  if (!project) return null;
  let start = Infinity, end = -Infinity;
  for (const track of project.tracks) for (const item of track.items) {
    if (selected && !selected.has(item.item_id)) continue;
    const first = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const last = first + rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    if (!Number.isFinite(first) || !Number.isFinite(last) || last <= first) continue;
    start = Math.min(start, first); end = Math.max(end, last);
  }
  return Number.isFinite(start) ? { start: selected ? start : 0, end } : null;
}

export function clampTimelineZoom(zoom: number): number {
  return Number.isFinite(zoom) ? Math.min(MAX_TIMELINE_ZOOM, Math.max(MIN_TIMELINE_ZOOM, zoom)) : DEFAULT_TIMELINE_ZOOM;
}

export function stepTimelineZoom(zoom: number, direction: 1 | -1): number {
  return clampTimelineZoom(zoom * (direction === 1 ? 1.5 : 1 / 1.5));
}

export function fitTimelineRange(range: TimelineViewRange | null, width: number) {
  if (!range) return { zoom: DEFAULT_TIMELINE_ZOOM, scrollLeft: 0 };
  const available = Math.max(1, width - TIMELINE_HEADER_WIDTH - 2 * FIT_PADDING);
  const zoom = clampTimelineZoom(available / Math.max(1, range.end - range.start));
  return { zoom, scrollLeft: Math.max(0, range.start * zoom - FIT_PADDING) };
}

export function anchoredTimelineScroll(oldZoom: number, newZoom: number, scrollLeft: number, width: number, playhead: number) {
  const visibleWidth = Math.max(0, width - TIMELINE_HEADER_WIDTH);
  const playheadX = playhead * oldZoom - scrollLeft;
  const anchorFrame = playheadX >= 0 && playheadX <= visibleWidth
    ? playhead : (scrollLeft + visibleWidth / 2) / oldZoom;
  return Math.max(0, scrollLeft + anchorFrame * (newZoom - oldZoom));
}

function niceCeiling(value: number): number {
  const magnitude = 10 ** Math.floor(Math.log10(Math.max(Number.EPSILON, value)));
  for (const factor of [1, 2, 5, 10]) if (factor * magnitude >= value) return factor * magnitude;
  return magnitude * 10;
}

function frameLabel(frame: number, fps: number, showFrames: boolean) {
  const millis = Math.round(frame / fps * 1000);
  const seconds = Math.floor(millis / 1000);
  const minutes = Math.floor(seconds / 60);
  const fraction = millis % 1000;
  const time = `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`
    + (fraction ? `.${String(fraction).padStart(3, "0").replace(/0+$/, "")}` : "");
  return showFrames ? `${time} (${frame}f)` : time;
}

/** Only materialize the visible ruler, with a small overscan on each side. */
export function timelineRulerTicks(rate: FrameRate, zoom: number, scrollLeft: number, width: number, totalFrames: number) {
  const fps = rate.numerator / rate.denominator;
  if (!(fps > 0) || !Number.isFinite(fps) || !(zoom > 0) || !Number.isFinite(zoom) || width <= 0) return [];
  const first = Math.max(0, (scrollLeft - 100) / zoom);
  const last = Math.min(totalFrames, (scrollLeft + Math.max(0, width - TIMELINE_HEADER_WIDTH) + 100) / zoom);
  const majorSeconds = Math.max(1, niceCeiling(90 / (fps * zoom)));
  const minorFrames = zoom >= 1 ? Math.max(1, niceCeiling(12 / zoom)) : niceCeiling(12 / (fps * zoom)) * fps;
  const ticks = new Map<number, boolean>();
  const add = (step: number, major: boolean) => {
    for (let n = Math.max(0, Math.ceil(first / step)); n * step <= last; n++) {
      const frame = Math.round(n * step);
      ticks.set(frame, major || (ticks.get(frame) ?? false));
    }
  };
  add(minorFrames, false); add(majorSeconds * fps, true);
  return [...ticks].sort(([a], [b]) => a - b).map(([f, isMajor]) => ({
    f, x: TIMELINE_HEADER_WIDTH + f * zoom, isMajor,
    label: isMajor ? frameLabel(f, fps, zoom >= 2.5) : null,
  }));
}
