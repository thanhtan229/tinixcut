export type SnapTargetKind = "playhead" | "zero" | "clipEdge";

export interface SnapTarget {
  frame: number;
  kind: SnapTargetKind;
  sourceItemId?: string;
  stableOrder: number;
}

export interface SnapCandidate {
  anchorFrame: number;
  anchorKind: "start" | "end";
}

export interface SnapResult {
  snapped: boolean;
  snappedDeltaFrames: number;
  target?: SnapTarget;
  distancePx?: number;
}

export interface SnappingOptions {
  zoom: number; // pixels per frame
  snapEnabled: boolean;
  altKeyHeld: boolean;
  playheadFrame: number;
  unselectedClipEdges: Array<{ start: number; end: number; itemId: string }>;
}

export const SNAP_THRESHOLD_PX = 8;

/**
 * Builds all valid snap targets from timeline environment.
 * Targets:
 * 1. Playhead
 * 2. Timeline zero
 * 3. Start/end edges of unselected clips
 */
export function buildSnapTargets(
  playheadFrame: number,
  unselectedClipEdges: Array<{ start: number; end: number; itemId: string }>
): SnapTarget[] {
  const targets: SnapTarget[] = [];
  let order = 0;

  // 1. Playhead target
  targets.push({
    frame: playheadFrame,
    kind: "playhead",
    stableOrder: order++,
  });

  // 2. Timeline zero target
  targets.push({
    frame: 0,
    kind: "zero",
    stableOrder: order++,
  });

  // 3. Clip edges of unselected clips
  for (const edge of unselectedClipEdges) {
    targets.push({
      frame: edge.start,
      kind: "clipEdge",
      sourceItemId: edge.itemId,
      stableOrder: order++,
    });
    targets.push({
      frame: edge.end,
      kind: "clipEdge",
      sourceItemId: edge.itemId,
      stableOrder: order++,
    });
  }

  return targets;
}

/**
 * Pure snapping calculator conforming strictly to Phase 16 specifications:
 * - Eligibility: abs(targetFrame - proposedAnchorFrame) * zoom <= 8 CSS pixels
 * - No minimum one-frame threshold
 * - Deterministic tie-breaking:
 *   1. Smallest pixel distance
 *   2. Target kind priority: playhead > zero > clipEdge
 *   3. Earlier semantic target time (frame)
 *   4. Stable target insertion order
 *   5. Stable source item ID
 * - Alt bypass suppresses snapping
 */
export function calculateSnap(
  candidates: SnapCandidate[],
  rawDeltaFrames: number,
  targets: SnapTarget[],
  zoom: number,
  snapEnabled: boolean,
  altKeyHeld: boolean
): SnapResult {
  if (!snapEnabled || altKeyHeld || zoom <= 0) {
    return {
      snapped: false,
      snappedDeltaFrames: rawDeltaFrames,
    };
  }

  interface BestMatch {
    target: SnapTarget;
    candidate: SnapCandidate;
    snappedDeltaFrames: number;
    distancePx: number;
    kindPriority: number; // 0 for playhead, 1 for zero, 2 for clipEdge
  }

  let best: BestMatch | null = null;

  for (const candidate of candidates) {
    const proposedAnchor = candidate.anchorFrame + rawDeltaFrames;

    for (const target of targets) {
      const distancePx = Math.abs(target.frame - proposedAnchor) * zoom;

      // Exact 8 CSS pixel threshold
      if (distancePx <= SNAP_THRESHOLD_PX) {
        const snappedDeltaFrames = target.frame - candidate.anchorFrame;

        const kindPriority =
          target.kind === "playhead" ? 0 : target.kind === "zero" ? 1 : 2;

        const currentMatch: BestMatch = {
          target,
          candidate,
          snappedDeltaFrames,
          distancePx,
          kindPriority,
        };

        if (!best) {
          best = currentMatch;
        } else {
          // Tie-breaking rules
          if (currentMatch.distancePx < best.distancePx - 1e-6) {
            best = currentMatch;
          } else if (Math.abs(currentMatch.distancePx - best.distancePx) <= 1e-6) {
            // Equal distance
            if (currentMatch.kindPriority < best.kindPriority) {
              best = currentMatch;
            } else if (currentMatch.kindPriority === best.kindPriority) {
              if (currentMatch.target.frame < best.target.frame) {
                best = currentMatch;
              } else if (currentMatch.target.frame === best.target.frame) {
                if (currentMatch.target.stableOrder < best.target.stableOrder) {
                  best = currentMatch;
                } else if (
                  currentMatch.target.stableOrder === best.target.stableOrder &&
                  currentMatch.target.sourceItemId &&
                  best.target.sourceItemId &&
                  currentMatch.target.sourceItemId < best.target.sourceItemId
                ) {
                  best = currentMatch;
                }
              }
            }
          }
        }
      }
    }
  }

  if (best) {
    return {
      snapped: true,
      snappedDeltaFrames: best.snappedDeltaFrames,
      target: best.target,
      distancePx: best.distancePx,
    };
  }

  return {
    snapped: false,
    snappedDeltaFrames: rawDeltaFrames,
  };
}
