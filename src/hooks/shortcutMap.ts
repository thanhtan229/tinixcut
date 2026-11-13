/**
 * Keyboard shortcut map: action ids, built-in chords, event -> chord normalisation, and the
 * validation used when the user remaps an action. Pure functions only, so every global key
 * handler resolves keys the same way and the rules are unit-tested.
 *
 * A chord is "Ctrl+Shift+K"-style text: modifiers in the order Ctrl, Alt, Shift, then the key.
 * Shift is part of the chord only for letters, digits and named keys; for printable symbols
 * (`+`, `[`, `-`) the key itself already says which one was typed.
 */

export const SHORTCUT_ACTIONS = [
  "play_pause",
  "copy",
  "paste",
  "split",
  "duplicate",
  "delete",
  "ripple_delete",
  "trim_head",
  "trim_tail",
  "zoom_in",
  "zoom_out",
  "toggle_snapping",
  "shuttle_reverse",
  "shuttle_pause",
  "shuttle_forward",
  "mark_in",
  "mark_out",
  "clear_in",
  "clear_out",
] as const;

export type ShortcutAction = (typeof SHORTCUT_ACTIONS)[number];
export type ShortcutMap = Record<ShortcutAction, string[]>;

export const DEFAULT_SHORTCUTS: ShortcutMap = {
  play_pause: ["Space"],
  copy: ["Ctrl+C"],
  paste: ["Ctrl+V"],
  split: ["Ctrl+K"],
  duplicate: ["Ctrl+D"],
  delete: ["Delete", "Backspace"],
  ripple_delete: ["Shift+Delete"],
  trim_head: ["["],
  trim_tail: ["]"],
  zoom_in: ["=", "+"],
  zoom_out: ["-", "_"],
  toggle_snapping: ["S"],
  shuttle_reverse: ["J"],
  shuttle_pause: ["K"],
  shuttle_forward: ["L"],
  mark_in: ["I"],
  mark_out: ["O"],
  clear_in: ["Alt+I"],
  clear_out: ["Alt+O"],
};

export const SHORTCUT_LABELS: Record<ShortcutAction, string> = {
  play_pause: "Play / pause",
  copy: "Copy clips",
  paste: "Paste clips",
  split: "Split at playhead",
  duplicate: "Duplicate clip",
  delete: "Delete clips",
  ripple_delete: "Ripple delete",
  trim_head: "Trim start to playhead",
  trim_tail: "Trim end to playhead",
  zoom_in: "Timeline zoom in",
  zoom_out: "Timeline zoom out",
  toggle_snapping: "Toggle snapping",
  shuttle_reverse: "Shuttle reverse (J)",
  shuttle_pause: "Shuttle stop (K)",
  shuttle_forward: "Shuttle forward (L)",
  mark_in: "Set In point",
  mark_out: "Set Out point",
  clear_in: "Clear In point",
  clear_out: "Clear Out point",
};

/** Chords the operating system or the webview already uses; never assignable. */
export const RESERVED_CHORDS = new Set([
  "Alt+F4",
  "F5",
  "F11",
  "F12",
  "Ctrl+R",
  "Ctrl+W",
  "Ctrl+Q",
  "Ctrl+N",
  "Ctrl+Shift+I",
  "Escape",
  "Tab",
  "Enter",
]);

export interface KeyEventLike {
  key: string;
  code?: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
}

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph"]);

/** Normalised chord text for a key event, or null for a bare modifier press. */
export function chordFromEvent(event: KeyEventLike): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  let key = event.code === "Space" || event.key === " " ? "Space" : event.key;
  const printableSymbol = key.length === 1 && !/[a-zA-Z0-9]/.test(key);
  if (event.shiftKey && !printableSymbol) parts.push("Shift");
  if (key.length === 1) key = key.toUpperCase();
  parts.push(key);
  return parts.join("+");
}

/** Built-in map with the user's overrides applied (unknown actions are ignored). */
export function effectiveShortcuts(overrides: Record<string, string[]> | undefined): ShortcutMap {
  const map: ShortcutMap = { ...DEFAULT_SHORTCUTS };
  for (const action of SHORTCUT_ACTIONS) {
    const chords = overrides?.[action];
    if (Array.isArray(chords) && chords.length > 0) map[action] = [...chords];
  }
  return map;
}

/** The action a key event triggers under `map`, or null. */
export function resolveShortcut(event: KeyEventLike, map: ShortcutMap): ShortcutAction | null {
  const chord = chordFromEvent(event);
  if (!chord) return null;
  for (const action of SHORTCUT_ACTIONS) {
    if (map[action].includes(chord)) return action;
  }
  return null;
}

/** Chords bound to more than one action. */
export function findConflicts(map: ShortcutMap): Array<{ chord: string; actions: ShortcutAction[] }> {
  const owners = new Map<string, ShortcutAction[]>();
  for (const action of SHORTCUT_ACTIONS) {
    for (const chord of map[action]) owners.set(chord, [...(owners.get(chord) ?? []), action]);
  }
  return [...owners.entries()]
    .filter(([, actions]) => actions.length > 1)
    .map(([chord, actions]) => ({ chord, actions }));
}

/** Why `chord` cannot be assigned to `action`, or null when it can. */
export function validateRemap(map: ShortcutMap, action: ShortcutAction, chord: string): string | null {
  if (!/^((Ctrl|Alt|Shift)\+){0,3}.+$/.test(chord)) return "Not a valid key combination";
  if (RESERVED_CHORDS.has(chord)) return `${chord} is reserved by the system`;
  for (const other of SHORTCUT_ACTIONS) {
    if (other !== action && map[other].includes(chord)) {
      return `${chord} is already used by "${SHORTCUT_LABELS[other]}"`;
    }
  }
  return null;
}

/** Overrides to persist: only actions that differ from the built-in chords. */
export function overridesFromMap(map: ShortcutMap): Record<string, string[]> {
  const overrides: Record<string, string[]> = {};
  for (const action of SHORTCUT_ACTIONS) {
    const same =
      map[action].length === DEFAULT_SHORTCUTS[action].length &&
      map[action].every((chord, index) => chord === DEFAULT_SHORTCUTS[action][index]);
    if (!same) overrides[action] = map[action];
  }
  return overrides;
}
