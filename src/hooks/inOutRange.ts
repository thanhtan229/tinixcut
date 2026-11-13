import type { OperationEnvelope } from "../types/command";
import type { Project } from "../types/project";

/** Transient In/Out selection on the timeline, in project frames. Never persisted. */
export interface InOutMarks {
  markIn: number | null;
  markOut: number | null;
}

/** The selected span `[start, end)`, or null while either mark is missing or they are not ordered. */
export function selectedSpan(marks: InOutMarks): { start: number; end: number } | null {
  if (marks.markIn === null || marks.markOut === null) return null;
  return marks.markOut > marks.markIn ? { start: marks.markIn, end: marks.markOut } : null;
}

/** Export range for the backend, clamped to the timeline; null when nothing valid is selected. */
export function exportRangeFor(
  marks: InOutMarks,
  timelineFrames: number
): { start_frame: number; end_frame: number } | null {
  const span = selectedSpan(marks);
  if (!span) return null;
  const end = Math.min(span.end, timelineFrames);
  return end > span.start ? { start_frame: span.start, end_frame: end } : null;
}

/** Setting a mark past the opposite one drops the opposite one, so the pair stays ordered. */
export function setMark(marks: InOutMarks, which: "in" | "out", frame: number): InOutMarks {
  const clamped = Math.max(0, Math.round(frame));
  if (which === "in") {
    return {
      markIn: clamped,
      markOut: marks.markOut !== null && marks.markOut > clamped ? marks.markOut : null,
    };
  }
  return {
    markIn: marks.markIn !== null && marks.markIn < clamped ? marks.markIn : null,
    markOut: clamped,
  };
}

export interface RippleDeletePlan {
  operations: OperationEnvelope[];
  /** Tracks skipped because they are locked. */
  skippedLockedTracks: string[];
  /** Number of clips that lose frames. */
  affectedClips: number;
}

/**
 * Operations that remove the span `[start, end)` from every unlocked track and close the gap, as
 * one transaction. Straddling clips are split first (with caller-supplied ids for the new right
 * halves, so later operations can refer to them), then each piece inside the span is ripple
 * deleted. `newId` must return a fresh valid id per call.
 */
export function planRippleDeleteRange(
  project: Project,
  span: { start: number; end: number },
  newId: () => string
): RippleDeletePlan {
  const rate = project.settings.frame_rate;
  const at = (frame: number) => ({ value: frame, rate });
  const splits: OperationEnvelope[] = [];
  const deletes: OperationEnvelope[] = [];
  const skipped: string[] = [];
  let affected = 0;
  let opIndex = 0;
  const operationId = (kind: string) => `op_range_${kind}_${opIndex++}`;

  for (const track of project.tracks) {
    const inside = track.items.filter(
      (item) =>
        item.timeline_range.start.value < span.end &&
        item.timeline_range.start.value + item.timeline_range.duration.value > span.start
    );
    if (inside.length === 0) continue;
    if (track.locked) {
      skipped.push(track.track_id);
      continue;
    }
    for (const item of inside) {
      affected += 1;
      const start = item.timeline_range.start.value;
      const end = start + item.timeline_range.duration.value;
      let middleId = item.item_id;
      if (start < span.start) {
        const rightId = newId();
        splits.push({
          operation_id: operationId("split_in"),
          operation: {
            type: "ClipSplit",
            payload: { item_id: middleId, split_time: at(span.start), new_right_item_id: rightId },
          },
        });
        middleId = rightId;
      }
      if (end > span.end) {
        const rightId = newId();
        splits.push({
          operation_id: operationId("split_out"),
          operation: {
            type: "ClipSplit",
            payload: { item_id: middleId, split_time: at(span.end), new_right_item_id: rightId },
          },
        });
      }
      deletes.push({
        operation_id: operationId("delete"),
        operation: { type: "ClipRippleDelete", payload: { item_id: middleId } },
      });
    }
  }
  return { operations: [...splits, ...deletes], skippedLockedTracks: skipped, affectedClips: affected };
}
