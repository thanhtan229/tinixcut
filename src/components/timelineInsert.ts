import { Project, rationalTimeToFrames } from "../types/project";

export interface TimelineInsertPlan {
  trackId: string;
  splitItemId: string | null;
  downstreamItemIds: string[];
  insertFrame: number;
  durationFrames: number;
}

export function getTimelineInsertPlan(
  project: Project,
  trackId: string,
  insertFrame: number,
  durationFrames: number,
  splitItemId: string,
): TimelineInsertPlan {
  if (!Number.isInteger(insertFrame) || insertFrame < 0) {
    throw new Error("Insert position must be a non-negative project frame.");
  }
  if (!Number.isInteger(durationFrames) || durationFrames <= 0) {
    throw new Error("Insert duration must be a positive number of project frames.");
  }

  const track = project.tracks.find((candidate) => candidate.track_id === trackId);
  if (!track) throw new Error("Insert target track no longer exists.");
  if (track.locked) throw new Error(`Track '${track.name}' is locked.`);

  let splitItem: string | null = null;
  const downstream: Array<{ itemId: string; startFrame: number }> = [];
  for (const item of track.items) {
    const start = rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate);
    const duration = rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate);
    const end = start + duration;
    if (start < insertFrame && insertFrame < end) splitItem = item.item_id;
    else if (start >= insertFrame) downstream.push({ itemId: item.item_id, startFrame: start });
  }
  if (splitItem) downstream.push({ itemId: splitItemId, startFrame: insertFrame });
  downstream.sort((left, right) => left.startFrame - right.startFrame || left.itemId.localeCompare(right.itemId));

  return {
    trackId,
    splitItemId: splitItem,
    downstreamItemIds: downstream.map(({ itemId }) => itemId),
    insertFrame,
    durationFrames,
  };
}
