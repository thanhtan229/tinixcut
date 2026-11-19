import { useEffect, useRef } from "react";
import { Project, TimelineItem } from "../types/project";
import { DEFAULT_SHORTCUTS, resolveShortcut, type ShortcutMap } from "./shortcutMap";

export interface TimelineShortcutsCallbacks {
  enabled?: boolean;
  project: Project | null;
  selectedItemIds: Set<string>;
  primarySelectedItemId: string | null;
  playheadFrame: number;
  onPlayPauseToggle: () => void;
  onSplitClip: (itemId: string, splitFrame: number) => void;
  onDuplicatePrimary: (itemId: string) => void;
  onCopySelected: () => void;
  onPasteSelected: () => void;
  onDeleteSelected: (itemIds: string[]) => void;
  onRippleDeletePrimary: (itemId: string) => void;
  onTrimHeadToPlayhead: (itemId: string, newStart: number, newDuration: number) => void;
  onTrimTailToPlayhead: (itemId: string, newDuration: number) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onMarkerNavigation?: (direction: 1 | -1) => void;
  onClipBoundaryNavigation?: (direction: 1 | -1) => void;
  onToggleSnapping: () => void;
  /** Effective shortcut map (built-in chords plus the user's overrides). */
  shortcuts?: ShortcutMap;
  onShuttle?: (key: "J" | "K" | "L") => void;
  onMarkIn?: () => void;
  onMarkOut?: () => void;
  onClearIn?: () => void;
  onClearOut?: () => void;
}

export function shouldSuppressTimelineShortcutsForTarget(
  targetElement?: { tagName?: string; isContentEditable?: boolean; role?: string } | null
): boolean {
  const active =
    targetElement !== undefined
      ? targetElement
      : typeof document !== "undefined"
      ? document.activeElement
      : null;

  if (!active) return false;
  const tagName = active.tagName ? active.tagName.toLowerCase() : "";
  if (["input", "textarea", "select", "button"].includes(tagName)) return true;
  if ("isContentEditable" in active && (active as { isContentEditable?: boolean }).isContentEditable) {
    return true;
  }
  const role = "role" in active ? String((active as { role?: unknown }).role ?? "").toLowerCase() : "";
  return ["button", "combobox", "listbox", "menuitem", "option", "slider", "spinbutton", "tab"].includes(role);
}

export function isTimelineClipShortcutTarget(
  targetElement?: { closest?: (selector: string) => unknown } | null
): boolean {
  const active =
    targetElement !== undefined
      ? targetElement
      : typeof document !== "undefined"
      ? document.activeElement
      : null;
  return Boolean(active && typeof active.closest === "function" && active.closest('[data-timeline-clip="true"]'));
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
  const enabled = callbacks.enabled ?? true;

  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (callbacksRef.current.enabled === false) return;

      const action = resolveShortcut(e, callbacksRef.current.shortcuts ?? DEFAULT_SHORTCUTS);
      if (action === null) return;
      const isClipAllowedShortcut = action === "copy" || action === "paste" || action === "previous_marker" || action === "next_marker" || action === "previous_clip_boundary" || action === "next_clip_boundary";
      const dialogIsOpen = typeof document !== "undefined" &&
        Boolean(document.querySelector(".dialog-backdrop, [role='dialog']"));

      // Timeline clips use role=button; clipboard and marker navigation remain
      // available there while editable fields and other controls stay isolated.
      if (
        dialogIsOpen ||
        (shouldSuppressTimelineShortcutsForTarget() &&
          !(isClipAllowedShortcut && isTimelineClipShortcutTarget()))
      ) {
        return;
      }

      const cb = callbacksRef.current;

      if (action === "previous_marker" || action === "next_marker") {
        e.preventDefault();
        cb.onMarkerNavigation?.(action === "previous_marker" ? -1 : 1);
        return;
      }
      if (action === "previous_clip_boundary" || action === "next_clip_boundary") {
        e.preventDefault();
        cb.onClipBoundaryNavigation?.(action === "previous_clip_boundary" ? -1 : 1);
        return;
      }

      // Ctrl/Cmd+C and Ctrl/Cmd+V -> same-project clip clipboard
      if (action === "copy") {
        if (cb.selectedItemIds.size > 0) {
          e.preventDefault();
          cb.onCopySelected();
        }
        return;
      }
      if (action === "paste") {
        e.preventDefault();
        cb.onPasteSelected();
        return;
      }

      // Space -> Play / Pause
      if (action === "play_pause") {
        e.preventDefault();
        cb.onPlayPauseToggle();
        return;
      }

      // Ctrl+K -> Split at Playhead
      if (action === "split") {
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
      if (action === "duplicate") {
        e.preventDefault();
        if (cb.primarySelectedItemId) {
          cb.onDuplicatePrimary(cb.primarySelectedItemId);
        }
        return;
      }

      // Shift+Delete -> Ripple Delete Primary
      if (action === "ripple_delete") {
        e.preventDefault();
        if (cb.primarySelectedItemId) {
          cb.onRippleDeletePrimary(cb.primarySelectedItemId);
        }
        return;
      }

      // Delete / Backspace -> Delete Selected Items
      if (action === "delete") {
        e.preventDefault();
        if (cb.selectedItemIds.size > 0) {
          cb.onDeleteSelected(Array.from(cb.selectedItemIds));
        }
        return;
      }

      // [ -> Normal Trim Head to Playhead (non-ripple)
      if (action === "trim_head") {
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
                cb.onTrimHeadToPlayhead(it.item_id, playhead, newDuration);
              }
              break;
            }
          }
        }
        return;
      }

      // ] -> Normal Trim Tail to Playhead (non-ripple)
      if (action === "trim_tail") {
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
      if (action === "zoom_in") {
        e.preventDefault();
        cb.onZoomIn();
        return;
      }

      // - -> Timeline Zoom Out
      if (action === "zoom_out") {
        e.preventDefault();
        cb.onZoomOut();
        return;
      }

      // S -> Toggle Snapping (ephemeral)
      if (action === "toggle_snapping") {
        e.preventDefault();
        cb.onToggleSnapping();
        return;
      }

      // J / K / L shuttle and I / O marks (see shuttle.ts and inOutRange.ts)
      if (action === "shuttle_reverse" || action === "shuttle_pause" || action === "shuttle_forward") {
        e.preventDefault();
        cb.onShuttle?.(action === "shuttle_reverse" ? "J" : action === "shuttle_pause" ? "K" : "L");
        return;
      }
      if (action === "mark_in" || action === "mark_out" || action === "clear_in" || action === "clear_out") {
        e.preventDefault();
        if (action === "mark_in") cb.onMarkIn?.();
        else if (action === "mark_out") cb.onMarkOut?.();
        else if (action === "clear_in") cb.onClearIn?.();
        else cb.onClearOut?.();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [enabled]);
}
