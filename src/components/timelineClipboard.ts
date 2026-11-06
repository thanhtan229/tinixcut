import { OperationEnvelope } from "../types/command";
import { Project, rationalTimeToFrames } from "../types/project";

export interface TimelineClipboard {
  projectId: string;
  itemIds: string[];
}

export function captureTimelineClipboard(
  project: Project | null,
  selectedItemIds: ReadonlySet<string>,
): TimelineClipboard | null {
  if (!project || selectedItemIds.size === 0) return null;

  const selectedItems = project.tracks
    .flatMap((track) => track.items.map((item) => ({ track, item })))
    .filter(({ item }) => selectedItemIds.has(item.item_id))
    .sort((left, right) => {
      const leftFrame = rationalTimeToFrames(left.item.timeline_range.start, project.settings.frame_rate);
      const rightFrame = rationalTimeToFrames(right.item.timeline_range.start, project.settings.frame_rate);
      return leftFrame - rightFrame || project.tracks.indexOf(left.track) - project.tracks.indexOf(right.track);
    });

  if (selectedItems.length === 0) return null;
  return { projectId: project.project_id, itemIds: selectedItems.map(({ item }) => item.item_id) };
}

export function buildTimelinePasteOperations(
  project: Project,
  clipboard: TimelineClipboard,
  pasteFrame: number,
  createId: () => string = () => crypto.randomUUID(),
): OperationEnvelope[] {
  if (clipboard.projectId !== project.project_id) {
    throw new Error("Timeline clipboard belongs to a different project.");
  }
  if (!Number.isInteger(pasteFrame) || pasteFrame < 0) {
    throw new Error("Paste position must be a non-negative project frame.");
  }

  const copiedIds = new Set(clipboard.itemIds);
  const entries = project.tracks.flatMap((track, trackIndex) =>
    track.items
      .filter((item) => copiedIds.has(item.item_id))
      .map((item) => ({ item, trackId: track.track_id, trackIndex })),
  );
  if (entries.length !== copiedIds.size) {
    throw new Error("One or more copied clips no longer exist in this project.");
  }
  if (entries.length === 0) return [];

  const ordered = entries.sort((left, right) => {
    const leftFrame = rationalTimeToFrames(left.item.timeline_range.start, project.settings.frame_rate);
    const rightFrame = rationalTimeToFrames(right.item.timeline_range.start, project.settings.frame_rate);
    return leftFrame - rightFrame || left.trackIndex - right.trackIndex;
  });
  const firstFrame = Math.min(
    ...ordered.map(({ item }) => rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate)),
  );
  const rate = project.settings.frame_rate;

  return ordered.map(({ item, trackId }, index) => {
    const targetFrame = pasteFrame + rationalTimeToFrames(item.timeline_range.start, rate) - firstFrame;
    const operationId = createId();
    return {
      operation_id: `op_timeline_paste_${operationId}`,
      operation: {
        type: "ClipDuplicate",
        payload: {
          item_id: item.item_id,
          target_start: { value: targetFrame, rate },
          target_track_id: trackId,
          new_item_id: `clip_paste_${index}_${operationId}`,
        },
      },
    };
  });
}
