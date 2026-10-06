import { Project } from "../types/project";

export interface SelectionState {
  selectedItemIds: Set<string>;
  primarySelectedItemId: string | null;
}

export function createInitialSelection(): SelectionState {
  return {
    selectedItemIds: new Set<string>(),
    primarySelectedItemId: null,
  };
}

/**
 * Normal click:
 * - replaces selection with exactly that item;
 * - makes that item primary.
 */
export function selectSingle(itemId: string): SelectionState {
  return {
    selectedItemIds: new Set([itemId]),
    primarySelectedItemId: itemId,
  };
}

/**
 * Ctrl+Click:
 * - unselected item: add item, make it primary;
 * - selected item: remove it; if it was primary, choose the most recently added
 *   remaining selected item (last element in iteration order), or null if empty.
 */
export function toggleSelect(current: SelectionState, itemId: string): SelectionState {
  const newSet = new Set(current.selectedItemIds);
  let newPrimary = current.primarySelectedItemId;

  if (newSet.has(itemId)) {
    newSet.delete(itemId);
    if (newPrimary === itemId) {
      if (newSet.size === 0) {
        newPrimary = null;
      } else {
        // Pick the most recently added remaining element (last in Set iteration order)
        const arr = Array.from(newSet);
        newPrimary = arr[arr.length - 1];
      }
    }
  } else {
    newSet.add(itemId);
    newPrimary = itemId;
  }

  return {
    selectedItemIds: newSet,
    primarySelectedItemId: newPrimary,
  };
}

/**
 * Empty canvas click:
 * - clears selection;
 * - sets primary null.
 */
export function clearSelection(): SelectionState {
  return createInitialSelection();
}

/**
 * Canonical refresh reconciliation:
 * - intersects selected IDs with current canonical item IDs;
 * - if primary no longer exists, choose an existing selected fallback;
 * - if no selected IDs remain, set primary null.
 */
export function reconcileSelection(
  current: SelectionState,
  project: Project | null
): SelectionState {
  if (!project) {
    return createInitialSelection();
  }

  const validIds = new Set<string>();
  for (const track of project.tracks) {
    for (const item of track.items) {
      validIds.add(item.item_id);
    }
  }

  const reconciledSet = new Set<string>();
  for (const id of current.selectedItemIds) {
    if (validIds.has(id)) {
      reconciledSet.add(id);
    }
  }

  let reconciledPrimary: string | null = null;
  if (current.primarySelectedItemId && validIds.has(current.primarySelectedItemId)) {
    reconciledPrimary = current.primarySelectedItemId;
  } else if (reconciledSet.size > 0) {
    const arr = Array.from(reconciledSet);
    reconciledPrimary = arr[arr.length - 1];
  }

  return {
    selectedItemIds: reconciledSet,
    primarySelectedItemId: reconciledPrimary,
  };
}
