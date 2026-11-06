import { Project, rationalTimeToFrames } from "../types/project";

export interface TimelineOverwriteSplit {
  itemId: string;
  frame: number;
  rightItemId: string;
}

export interface TimelineOverwritePlan {
  trackId: string;
  insertFrame: number;
  durationFrames: number;
  splits: TimelineOverwriteSplit[];
  deleteItemIds: string[];
}

export function getTimelineOverwritePlan(
  project: Project,
  trackId: string,
  insertFrame: number,
  durationFrames: number,
  idPrefix: string,
): TimelineOverwritePlan {
  if (!Number.isInteger(insertFrame) || insertFrame < 0) {
    throw new Error("Overwrite position must be a non-negative project frame.");
  }
  if (!Number.isInteger(durationFrames) || durationFrames <= 0) {
    throw new Error("Overwrite duration must be a positive number of project frames.");
  }
  const track = project.tracks.find((candidate) => candidate.track_id === trackId);
  if (!track) throw new Error("Overwrite target track no longer exists.");
  if (track.locked) throw new Error(`Track '${track.name}' is locked.`);

  const overwriteEnd = insertFrame + durationFrames;
  if (!Number.isSafeInteger(overwriteEnd)) throw new Error("Overwrite range exceeds the supported timeline range.");

  const splits: TimelineOverwriteSplit[] = [];
  const deleteItemIds: string[] = [];
  const ordered = track.items
    .map((item) => ({
      item,
      start: rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate),
      duration: rationalTimeToFrames(item.timeline_range.duration, project.settings.frame_rate),
    }))
    .sort((left, right) => left.start - right.start || left.item.item_id.localeCompare(right.item.item_id));

  for (const { item, start, duration } of ordered) {
    const end = start + duration;
    if (start >= overwriteEnd || end <= insertFrame) continue;

    let coveredItemId = item.item_id;
    if (start < insertFrame && insertFrame < end) {
      coveredItemId = `${idPrefix}_right_${item.item_id}`;
      splits.push({ itemId: item.item_id, frame: insertFrame, rightItemId: coveredItemId });
    }
    if (start < overwriteEnd && overwriteEnd < end) {
      const tailItemId = `${idPrefix}_tail_${item.item_id}`;
      splits.push({ itemId: coveredItemId, frame: overwriteEnd, rightItemId: tailItemId });
      // The current item becomes the covered left fragment; the right split is
      // preserved as the post-overwrite tail.
      deleteItemIds.push(coveredItemId);
      continue;
    }
    deleteItemIds.push(coveredItemId);
  }

  return { trackId, insertFrame, durationFrames, splits, deleteItemIds };
}
