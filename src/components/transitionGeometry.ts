import type { CutTransitionKind, TransitionKind } from "../types/project";

/**
 * Preview geometry of the cut transitions that are a pure function of progress. The formulas
 * follow FFmpeg `vf_xfade.c` (the export path): `t` is the elapsed fraction 0..1 (xfade's own
 * `progress` is 1 - t). They are checked against edges measured on real exports in
 * `transitionGeometry.test.ts` (fixture `transitionGolden.json`).
 */

export const GEOMETRIC_KINDS = [
  "WipeLeft",
  "WipeRight",
  "WipeUp",
  "WipeDown",
  "SlideLeft",
  "SlideRight",
  "CircleOpen",
] as const;

export type GeometricKind = (typeof GEOMETRIC_KINDS)[number];

export function isGeometricKind(kind: TransitionKind | string): kind is GeometricKind {
  return (GEOMETRIC_KINDS as readonly string[]).includes(kind);
}

export interface TransitionShape {
  /** CSS `clip-path` for the incoming clip, when it is revealed by an edge. */
  incomingClipPath?: string;
  /** CSS `mask-image` for the incoming clip (circle). */
  incomingMask?: string;
  outgoingTransform?: string;
  incomingTransform?: string;
  /** Edge position as a fraction of the frame width/height (linear kinds). */
  boundary: number | null;
  /** Radius where the incoming clip is 50 % visible, as a fraction of the half diagonal. */
  radiusFraction: number | null;
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function smoothstep(x: number): number {
  const t = clamp01(x);
  return t * t * (3 - 2 * t);
}

/** Weight of the incoming clip at normalised radius `rr` (0 centre, 1 half diagonal). */
export function circleOpenWeight(rr: number, t: number): number {
  const p = (0.5 - t) * 3; // xfade: (progress - 0.5) * 3 with progress = 1 - t
  return 1 - smoothstep(rr + p);
}

function circleMask(t: number): string {
  const stops: string[] = [];
  for (let i = 0; i <= 16; i += 1) {
    const rr = i / 16;
    stops.push(`rgba(0,0,0,${circleOpenWeight(rr, t).toFixed(4)}) ${(rr * 100).toFixed(2)}%`);
  }
  // `farthest-corner` makes 100 % the half diagonal, the same unit as xfade's radius.
  return `radial-gradient(circle farthest-corner at 50% 50%, ${stops.join(", ")})`;
}

export function transitionShape(kind: GeometricKind, progress: number): TransitionShape {
  const t = clamp01(progress);
  const rest = ((1 - t) * 100).toFixed(4);
  const done = (t * 100).toFixed(4);
  switch (kind) {
    case "WipeLeft":
      return { incomingClipPath: `inset(0 0 0 ${rest}%)`, boundary: 1 - t, radiusFraction: null };
    case "WipeRight":
      return { incomingClipPath: `inset(0 ${rest}% 0 0)`, boundary: t, radiusFraction: null };
    case "WipeUp":
      return { incomingClipPath: `inset(${rest}% 0 0 0)`, boundary: 1 - t, radiusFraction: null };
    case "WipeDown":
      return { incomingClipPath: `inset(0 0 ${rest}% 0)`, boundary: t, radiusFraction: null };
    case "SlideLeft":
      return {
        outgoingTransform: `translateX(-${done}%)`,
        incomingTransform: `translateX(${rest}%)`,
        boundary: 1 - t,
        radiusFraction: null,
      };
    case "SlideRight":
      return {
        outgoingTransform: `translateX(${done}%)`,
        incomingTransform: `translateX(-${rest}%)`,
        boundary: t,
        radiusFraction: null,
      };
    case "CircleOpen":
      return { incomingMask: circleMask(t), boundary: null, radiusFraction: Math.max(0, 3 * t - 1) };
  }
}

export const CUT_TRANSITION_OPTIONS: Array<{ value: CutTransitionKind; label: string }> = [
  { value: "CrossDissolve", label: "Cross dissolve" },
  { value: "DipToBlack", label: "Dip to black" },
  { value: "WipeLeft", label: "Wipe left" },
  { value: "WipeRight", label: "Wipe right" },
  { value: "WipeUp", label: "Wipe up" },
  { value: "WipeDown", label: "Wipe down" },
  { value: "SlideLeft", label: "Slide left" },
  { value: "SlideRight", label: "Slide right" },
  { value: "CircleOpen", label: "Circle open" },
];

export function cutTransitionLabel(kind: TransitionKind): string {
  return CUT_TRANSITION_OPTIONS.find((option) => option.value === kind)?.label ?? kind;
}
