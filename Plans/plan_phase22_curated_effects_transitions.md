# Phase 22 — Curated Effects and Transitions

**Status:** APPROVED FOR IMPLEMENTATION  
**Scope:** A small deterministic visual preset pack and one additional cut-owned transition, expressed through the existing project, command, composition, preview, and export paths.

## Objective

Add visible polish without turning TinixCut into a host for arbitrary FFmpeg graphs, browser filters, shaders, or third-party effect code. Every selected effect and transition must be canonical editable project state, pass through the Command Service, and be compiled by the existing normalized composition plan.

## Product scope

### Visual effect presets

The initial application-owned pack is intentionally small:

| Canonical preset | Preview mapping | Export mapping | Purpose |
|---|---|---|---|
| `None` | no CSS filter | no FFmpeg color filter | Explicit default and reset action. |
| `Mono` | `saturate(0%)` | `eq=saturation=0` | Desaturated editorial treatment. |
| `Vivid` | `contrast(112%) saturate(125%)` | `eq=contrast=1.12:saturation=1.25` | Higher contrast and saturation. |
| `Muted` | `contrast(96%) saturate(70%)` | `eq=contrast=0.96:saturation=0.70` | Reduced contrast and saturation. |

`VisualEffectPreset` is a typed field on a visual `TimelineItem`. It defaults to `None`. It is deliberately a preset identity, not a free-form brightness, contrast, saturation, CSS, FFmpeg, or shader payload.

The presets apply to Video and Image items. Text items retain their existing typography and template semantics; phase 22 does not add text colour grading.

### Cut-owned transition

Keep the existing `Track.transitions: Vec<CutTransition>` topology. Add only one new kind:

| Canonical kind | Behaviour |
|---|---|
| `CrossDissolve` | Existing overlapping crossfade semantics, unchanged. |
| `DipToBlack` | A centred cut interval that fades the outgoing participant to an opaque black matte and fades the incoming participant up from that matte. |

`DipToBlack` uses the same adjacent-video validation, exact rational cut window, handle proof, duration edit, removal, history, automation, and persistence path as `CrossDissolve`. It does not overlap persisted clip ranges or introduce a clip-owned duplicate transition record.

The pre-existing item-level `FadeIn`, `FadeOut`, and legacy `FadeBlack` values remain unchanged. `DipToBlack` is allowed only for a `CutTransition`; item-level transition validation rejects it clearly.

## Canonical behaviour

1. Add `TimelineItem.visual_effect: VisualEffectPreset`, defaulting to `None`.
2. Add `TransitionKind::DipToBlack` while preserving `CrossDissolve` and existing item-transition variants.
3. Compile a visual preset into a `CompositionVisualLayer` value. Preview and export consume that compiled value rather than read raw item state independently.
4. Compile a dip-to-black transition into the existing `TransitionPlan`. Its sampled preview representation contains the participant opacity and an explicit black matte requirement; export emits the matching finite filtergraph branch. No FFmpeg graph is persisted.
5. Use `ClipVisualEffectSet`, `CutTransitionCreate`, `CutTransitionUpdateDuration`, and `CutTransitionRemove` through the existing revision-checked Command Service. Effect change and transition edits must use the ordinary undo/redo transaction history.
6. Expose the selected effect and transition kind through the same automation command schemas used by desktop UI. Automation remains a client of the Command Service.

## Compatibility and migration

- Increase the project schema from v7 to v8.
- The v7-to-v8 migration initializes `visual_effect = None` on every existing timeline item and preserves all tracks, item fades, cut transitions, transforms, media references, keyframes, and canvas state.
- `#[serde(default)]` protects optional backward-compatible reads, but the explicit migration remains the versioned proof that an older project cannot silently lose a selected effect when saved by a newer build.
- Existing cut transitions deserialize as `CrossDissolve`. No transition is inferred for an old project.
- An item-level `DipToBlack` from malformed external JSON fails validation rather than being activated.

## Preview/export parity contract

Phase 22 defines parity at the normalized-composition boundary:

- the exact preset identity and its fixed numeric parameters appear in both preview-style and export-filter fixtures;
- `None` produces neither a preview filter nor an export filter;
- transition fixtures prove the same cut interval, participants, duration, black-matte requirement, and backend transition branch;
- canonical project timing, handle validation, and source mappings remain independent of renderer-specific mechanics.

Browser CSS and FFmpeg do not promise byte-identical colour output across their colour-management implementations. The tests therefore verify deterministic canonical intent and generated backend mappings; Phase 26 performs the native visual qualification. This avoids claiming pixel equality that the two platforms cannot guarantee.

## UI

- The Inspector shows an **Effect** selector for a selected Video or Image item, including **None**.
- The outgoing-cut transition control offers **Cross dissolve** and **Dip to black** before creation.
- A selected cut transition shows its kind, duration, remove action, and contextual handle failure reason using the existing error contract.
- Disabled states preserve the existing rules: transitions require adjacent Video participants with provable handles; effects are unavailable for nonvisual items.

No marketplace, effect browser, keyframed effect parameters, LUT import, shader editor, arbitrary colour controls, or user-authored filter graph is part of this phase.

## Implementation order

1. Add typed domain values, validation, schema v8 migration, and serialization tests.
2. Add the revision-checked visual-effect command, inverse history behaviour, and automation schema support.
3. Carry effects and dip-to-black semantics through composition, sampled preview, parity fixtures, and FFmpeg export.
4. Add Inspector controls and command dispatch without maintaining local semantic state.
5. Add focused regressions for migration, command validation, undo/redo, save/reopen, composition/export mappings, transition handles, and automation/manual equivalence.
6. Run the relevant Rust, frontend, build, formatting, diff, and runtime-restart checks. Leave native manual qualification for Phase 26.

## Acceptance criteria

- **P22-AC-01:** Each visual preset is a typed canonical field, survives save/reopen, and has a lossless v7-to-v8 migration default.
- **P22-AC-02:** Setting or clearing an effect uses the Command Service, revision checks, undo, redo, and the automation route.
- **P22-AC-03:** Composition, preview mappings, and export mappings agree on every supported preset; no raw CSS or FFmpeg filter graph is persisted.
- **P22-AC-04:** `DipToBlack` is cut-owned, exact-time centred, duration-invariant, and uses the existing handle/error contract.
- **P22-AC-05:** Existing CrossDissolve and item-level fade behaviour remain covered by regression tests.
- **P22-AC-06:** The Inspector provides actionable effect/transition controls and exposes validation failure without creating invalid state.
- **P22-AC-07:** Targeted regression, full automated validation, build, and exact project-owned runtime restart pass. Native visual qualification is retained for Phase 26.

