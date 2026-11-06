import { Project, rationalTimeToFrames } from "../types/project";

export interface TimelineGapClosePlan {
  trackId: string;
  itemIds: string[];
  gapFrames: number;
}

export function getTimelineGapClosePlan(
  project: Project | null,
  selectedItemId: string | null,
): TimelineGapClosePlan | null {
  if (!project || !selectedItemId) return null;
  const track = project.tracks.find((candidate) => candidate.items.some((item) => item.item_id === selectedItemId));
  if (!track || track.locked) return null;

  const ordered = track.items
    .map((item) => ({ item, start: rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate), duration: rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate) }))
    .sort((left, right) => left.start - right.start);
  const selectedIndex = ordered.findIndex(({ item }) => item.item_id === selectedItemId);
  if (selectedIndex <= 0) return null;

  const previous = ordered[selectedIndex - 1];
  const selectedStart = ordered[selectedIndex].start;
  const gapFrames = selectedStart - (previous.start + previous.duration);
  if (gapFrames <= 0) return null;

  return {
    trackId: track.track_id,
    itemIds: ordered.slice(selectedIndex).map(({ item }) => item.item_id),
    gapFrames,
  };
}
