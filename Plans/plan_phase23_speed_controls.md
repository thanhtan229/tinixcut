# Phase 23 — Constant-Speed Clip Controls

**Status:** APPROVED FOR IMPLEMENTATION  
**Scope:** Canonical, bounded constant-rate playback for Video and Audio clips, compiled through the existing command, composition, preview, and export paths.

## Objective

Add useful speed controls without changing the editable timeline into floating-point time or introducing speed ramps, reverse playback, optical flow, or beat-sync behaviour.

The initial control set is deliberately finite:

| Canonical speed | UI label | Timeline-to-source mapping |
|---|---|---|
| `Half` | `0.5×` | One timeline frame consumes half a source frame. |
| `Normal` | `1×` | One timeline frame consumes one source frame. |
| `Double` | `2×` | One timeline frame consumes two source frames. |

## Canonical behaviour

1. `TimelineItem.speed` is a typed `ClipSpeed` value, defaulting to `Normal`.
2. The timeline range remains fixed when a speed is selected. The command changes the source-range duration to the exact amount required by the selected speed, starting at the existing source in-point.
3. A speed command fails before mutation if:
   - the item is not Video or Audio;
   - half-speed cannot be represented exactly at the canonical frame rate;
   - the required source range is outside the resolved asset duration;
   - the command would invalidate an active cut transition or audio crossfade.
4. The canonical duration invariant changes from `source_duration == timeline_duration` to `source_duration == timeline_duration × speed`. No floating-point seconds enter project state.
5. Trim and split preserve the same invariant. A split that cannot produce exact source ranges for the selected speed is rejected clearly rather than rounded.
6. Composition carries `speed` in each visual and audio layer. Preview source sampling and cut-transition source sampling use the same mapping.
7. Preview sets each HTML media element’s `playbackRate` from the sampled layer and explicitly preserves pitch for audio playback.
8. Export trims the canonical source coverage then time-scales:
   - visual: `setpts=PTS/<speed>`;
   - audio: `atempo=<speed>`, which keeps the normal pitch.
9. The new command uses the existing revision-checked Command Service and ordinary transaction history. It has no local-only UI state and remains available to automation via the same command schema.

## Compatibility and migration

- Increase project schema from v8 to v9.
- The v8-to-v9 migration assigns `speed = Normal` to every existing timeline item.
- `#[serde(default)]` protects reads of compatible legacy JSON, while the explicit migration preserves a stable canonical project on its next save.

## Product boundaries

This phase does not add:

- speed curves or keyframed speed;
- reverse playback;
- frame interpolation or optical flow;
- pitch-shifting controls;
- beat synchronization;
- automatic ripple edits.

Pitch policy is selected explicitly: audio keeps its normal pitch while speed changes. Native browser media uses `preservesPitch`, and export uses FFmpeg `atempo`.

## UI

- The Inspector shows a **Speed** selector for selected Video and Audio clips.
- It displays `0.5×`, `1×`, and `2×`.
- Invalid source coverage is surfaced through the existing command error route; the UI never silently clamps or changes timeline duration.
- Image and Text items do not show the control.

## Implementation order

1. Add the typed domain value, exact duration helpers, validation, schema v9 migration, and serialization tests.
2. Add `ClipSpeedSet`, including asset coverage validation, transition/crossfade validation, undo/redo, and split/trim invariant handling.
3. Carry speed through normalized composition, preview sampling, cut transition sampling, and FFmpeg visual/audio render paths.
4. Add Inspector dispatch and browser playback-rate/pitch behaviour.
5. Add focused regressions for migration, command history, invalid media coverage, split/trim exactness, preview mapping, export mapping, and normal-speed compatibility.
6. Run the focused and full automated checks, build, formatting, diff validation, and restart the project-owned development runtime. Native playback qualification remains batched in Phase 26.

## Acceptance criteria

- **P23-AC-01:** `ClipSpeed` is typed, persisted, defaults to `Normal`, and migrates v8 projects losslessly.
- **P23-AC-02:** Setting `0.5×`, `1×`, or `2×` is a revision-checked command with undo/redo and no local semantic state.
- **P23-AC-03:** Timeline/source duration and timestamp mapping stay exact at every supported speed; invalid fractional mappings and insufficient source coverage fail clearly.
- **P23-AC-04:** Preview, transitions, and export consume the same normalized speed mapping; audio retains pitch at non-normal rates.
- **P23-AC-05:** Inspector exposes speed only for Video and Audio clips and reflects command failure safely.
- **P23-AC-06:** Existing normal-speed trim, split, transition, audio, and export regressions continue to pass.
- **P23-AC-07:** Targeted regression, full automated validation, build, and exact project-owned runtime restart pass. Native playback qualification is retained for Phase 26.
