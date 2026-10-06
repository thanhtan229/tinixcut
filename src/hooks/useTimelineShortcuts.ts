import { useEffect, useRef } from "react";
import { Project, TimelineItem } from "../types/project";

export interface TimelineShortcutsCallbacks {
  project: Project | null;
  selectedItemIds: Set<string>;
  primarySelectedItemId: string | null;
  playheadFrame: number;
  onPlayPauseToggle: () => void;
  onSplitClip: (itemId: string, splitFrame: number) => void;
  onDuplicatePrimary: (itemId: string) => void;
  onDeleteSelected: (itemIds: string[]) => void;
  onRippleDeletePrimary: (itemId: string) => void;
  onTrimHeadToPlayhead: (itemId: string, newStart: number, newDuration: number, newSourceStart: number) => void;
  onTrimTailToPlayhead: (itemId: string, newDuration: number) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onToggleSnapping: () => void;
}

export function isFocusInInput(
  targetElement?: { tagName?: string; isContentEditable?: boolean } | null
): boolean {
  const active =
    targetElement !== undefined
      ? targetElement
      : typeof document !== "undefined"
      ? document.activeElement
      : null;

  if (!active) return false;
  const tagName = active.tagName ? active.tagName.toLowerCase() : "";
  if (tagName === "input" || tagName === "textarea") return true;
  if ("isContentEditable" in active && (active as { isContentEditable?: boolean }).isContentEditable) {
    return true;
  }
  return false;
}

export function findClipForSplitAtPlayhead(
  project: Project | null,
  primarySelectedItemId: string | null,
  playheadFrame: number
): TimelineItem | null {
  if (!project) return null;

  // 1. If primary selected item contains playhead in its strict interior:
  if (primarySelectedItemId) {
    for (const track of project.tracks) {
      const it = track.items.find((i) => i.item_id === primarySelectedItemId);
      if (it) {
        const start = it.timeline_range.start.value;
        const end = start + it.timeline_range.duration.value;
        if (playheadFrame > start && playheadFrame < end) {
          return it;
        }
      }
    }
  }

  // 2. Otherwise inspect tracks in canonical order:
  for (const track of project.tracks) {
    for (const item of track.items) {
      const start = item.timeline_range.start.value;
      const end = start + item.timeline_range.duration.value;
      if (playheadFrame > start && playheadFrame < end) {
        return item;
      }
    }
  }

  // 3. None found -> clean no-op
  return null;
}

export function useTimelineShortcuts(callbacks: TimelineShortcutsCallbacks) {
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // 1. Suppress all timeline shortcuts while editing inside text/input controls
      if (isFocusInInput()) {
        return;
      }

      const cb = callbacksRef.current;
      const isCtrl = e.ctrlKey || e.metaKey;
      const isShift = e.shiftKey;
      const isAlt = e.altKey;

      // Space -> Play / Pause
      if (e.code === "Space" && !isCtrl && !isShift && !isAlt) {
        e.preventDefault();
        cb.onPlayPauseToggle();
        return;
      }

      // Ctrl+K -> Split at Playhead
      if (isCtrl && !isShift && !isAlt && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        const clipToSplit = findClipForSplitAtPlayhead(
          cb.project,
          cb.primarySelectedItemId,
          cb.playheadFrame
        );
        if (clipToSplit) {
          cb.onSplitClip(clipToSplit.item_id, cb.playheadFrame);
        }
        return;
      }

      // Ctrl+D -> Duplicate Primary
      if (isCtrl && !isShift && !isAlt && (e.key === "d" || e.key === "D")) {
        e.preventDefault();
        if (cb.primarySelectedItemId) {
          cb.onDuplicatePrimary(cb.primarySelectedItemId);
        }
        return;
      }

      // Shift+Delete -> Ripple Delete Primary
      if (isShift && !isCtrl && !isAlt && (e.key === "Delete" || e.code === "Delete")) {
        e.preventDefault();
        if (cb.primarySelectedItemId) {
          cb.onRippleDeletePrimary(cb.primarySelectedItemId);
        }
        return;
      }

      // Delete / Backspace -> Delete Selected Items
      if (!isShift && !isCtrl && !isAlt && (e.key === "Delete" || e.key === "Backspace")) {
        e.preventDefault();
        if (cb.selectedItemIds.size > 0) {
          cb.onDeleteSelected(Array.from(cb.selectedItemIds));
        }
        return;
      }

      // [ -> Normal Trim Head to Playhead (non-ripple)
      if (!isCtrl && !isShift && !isAlt && e.key === "[") {
        e.preventDefault();
        if (cb.primarySelectedItemId && cb.project) {
          for (const track of cb.project.tracks) {
            const it = track.items.find((i) => i.item_id === cb.primarySelectedItemId);
            if (it) {
              const start = it.timeline_range.start.value;
              const dur = it.timeline_range.duration.value;
              const end = start + dur;
              const playhead = cb.playheadFrame;
              // Playhead must be strictly within clip or at start to trim head
              if (playhead > start && playhead < end) {
                const delta = playhead - start;
                const newDuration = dur - delta;
                const newSourceStart = it.source_range.start.value + delta;
                cb.onTrimHeadToPlayhead(it.item_id, playhead, newDuration, newSourceStart);
              }
              break;
            }
          }
        }
        return;
      }

      // ] -> Normal Trim Tail to Playhead (non-ripple)
      if (!isCtrl && !isShift && !isAlt && e.key === "]") {
        e.preventDefault();
        if (cb.primarySelectedItemId && cb.project) {
          for (const track of cb.project.tracks) {
            const it = track.items.find((i) => i.item_id === cb.primarySelectedItemId);
            if (it) {
              const start = it.timeline_range.start.value;
              const dur = it.timeline_range.duration.value;
              const end = start + dur;
              const playhead = cb.playheadFrame;
              if (playhead > start && playhead < end) {
                const newDuration = playhead - start;
                cb.onTrimTailToPlayhead(it.item_id, newDuration);
              }
              break;
            }
          }
        }
        return;
      }

      // = / + -> Timeline Zoom In
      if (!isCtrl && !isAlt && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        cb.onZoomIn();
        return;
      }

      // - -> Timeline Zoom Out
      if (!isCtrl && !isAlt && (e.key === "-" || e.key === "_")) {
        e.preventDefault();
        cb.onZoomOut();
        return;
      }

      // S -> Toggle Snapping (ephemeral)
      if (!isCtrl && !isShift && !isAlt && (e.key === "s" || e.key === "S")) {
        e.preventDefault();
        cb.onToggleSnapping();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);
}
