# Phase 14 — Advanced Keyframe Interpolation

## Status

Status: COMPLETE — accepted by ChatGPT and ratified by user after implementation

Governance note:
- The plan was originally authored as DRAFT FOR USER REVIEW.
- Implementation was executed before the expected explicit approval gate.
- This deviation was detected during ChatGPT completion review.
- The implementation was subsequently reviewed against the full plan,
  remediated, verified, and explicitly ratified by the user.
- This does not rewrite history as pre-implementation approval.


---

## 1. Objective

Deliver production-ready advanced keyframe interpolation for TinixCut while repairing the previously discovered full-export keyframe correctness gap.

Phase 14 must:

1. promote the project format from Schema v2 to Schema v3 with lossless migration;
2. add canonical outgoing easing descriptors to existing clip-local keyframes;
3. implement one deterministic Rust/core interpolation evaluator supporting Linear, EaseIn, EaseOut, EaseInOut, and bounded Custom CubicBezier;
4. make all keyframe time equality/order semantics use canonical RationalTime comparison rather than raw stored tick values;
5. preserve current command upsert behavior while extending it safely for easing;
6. correct keyframe lifecycle semantics for split/trim/duplicate;
7. repair `P14-PREREQ-01`: dynamic linear keyframes must affect full export, not only preview/single-frame export;
8. make full video export consume Rust-evaluated per-frame literal automation schedules;
9. make full audio Volume export consume a Rust-evaluated sample-accurate gain envelope;
10. preserve Preview / single-frame export / full export semantic parity;
11. expose the same semantics through UI and automation via the shared Command Service;
12. preserve Phase 13 CrossDissolve invariants and export behavior.

This phase is governed by ADR-004 and must remain compatible with ADR-003.

---

## 2. Authoritative Inputs

Implementation must follow the latest reconciled durable state:

- `PLAN.md`
- `PROJECT_STATUS.md`
- `ARCHITECTURE.md`
- `RESEARCH.md`
- `ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md`
- `ADR_004_CANONICAL_KEYFRAME_EASING_CLIP_LOCAL_LIFECYCLE_AND_SAMPLED_EXPORT_AUTOMATION_SEMANTICS.md`
- retained POC evidence in `src-tauri/tests/export_automation_poc.rs`

Source of truth order remains:

1. repository code = actual implementation;
2. ADR-004 / architecture = authoritative Phase-14 semantics;
3. this phase plan = approved implementation scope and acceptance criteria;
4. durable roadmap/status docs = project state;
5. chat = coordination only.

If implementation reality conflicts with ADR-004 or this plan, stop and reconcile explicitly rather than silently redesigning behavior.

---

## 3. Historical Prerequisite

### P14-PREREQ-01 — Legacy dynamic keyframe full-export correctness defect

Phase 9 accepted production-supported linear keyframes with preview/export expectations, but the current repo evaluates dynamic keyframes in:

- interactive preview;
- single-frame export/extraction;

while full video/audio export still uses static transform/opacity/volume values.

Phase 14 owns remediation of this later-discovered defect.

Rules:

- Do NOT reopen Phase 9.
- Do NOT document the old behavior as an intentional preview-only contract.
- Full export must honor existing Linear keyframes before advanced easing can be considered production-complete.
- A negative-control regression test must fail if full export reverts to static values.

---

## 4. Scope

### 4.1 In scope

#### Schema / domain
- Schema v3 promotion.
- Lossless v2 → v3 migration.
- `Keyframe.outgoing_easing`.
- Canonical easing enum/descriptor.
- Domain validation for easing values and last-keyframe invariant.
- Explicit failure for unsupported future easing variants.

#### Canonical evaluator
- semantic RationalTime ordering/equality;
- exact endpoint handling;
- Linear;
- EaseIn;
- EaseOut;
- EaseInOut;
- Custom CubicBezier;
- deterministic fixed 32-iteration bisection;
- numeric scalar interpolation;
- no interpolation-layer range clamping.

#### Existing seven keyframe channels
- PositionX
- PositionY
- ScaleX
- ScaleY
- Rotation
- Opacity
- Volume

No new keyframe-capable properties are authorized.

#### Command semantics
- extend `ClipKeyframeAdd`;
- preserve current upsert/replacement behavior;
- semantic RationalTime collision handling;
- extend `ClipKeyframeMove`;
- add explicit easing-set operation;
- dry-run/apply parity;
- undo/redo;
- structured errors;
- automation exposure.

#### Editing lifecycle
- split distribution/rebase;
- exact boundary representation;
- split-through-segment boundary synthesis;
- cubic curve subdivision via de Casteljau;
- non-destructive tail trim;
- non-destructive head trim with keyframe offset shift;
- duplicate preservation;
- atomic history semantics.

#### Composition / preview
- shared evaluator integration;
- no frontend interpolation engine;
- preview sampling parity;
- mixed-rate keyframe timing.

#### Full video export
- derived ExportAutomationSchedule;
- Rust evaluation on final output-frame RationalTime grid;
- literal runtime command schedule;
- frame 0 initial options;
- midpoint dispatch for later frames;
- position/scale/rotation/opacity mapping;
- Phase 13 transition-branch targeting;
- temporary command-file lifecycle.

#### Full audio Volume export
- Rust evaluation on final audio sample grid;
- streamed mono f32le gain envelope;
- sample-aligned FFmpeg `amultiply`;
- disk-space preflight;
- success/failure/cancellation cleanup.

#### UI / automation
- minimal easing selection;
- minimal numeric Custom CubicBezier control entry;
- no curve editor;
- same Command Service path for UI/CLI/session automation;
- capability inspection of supported easing kinds/properties.

#### Verification
- migration;
- domain;
- command;
- lifecycle;
- preview;
- full export;
- CrossDissolve coexistence;
- automation;
- persistence;
- undo/redo;
- failure cleanup;
- parity;
- negative controls;
- full regression/build/typecheck/lint/format.

### 4.2 Explicitly out of scope

Do NOT add:

- overshoot cubic curves;
- y control points outside [0,1];
- spring/physics easing;
- Step/Hold interpolation;
- motion paths;
- spatial tangents;
- shortest-path rotation redesign;
- expressions;
- animation graph;
- generalized graph/curve editor;
- per-axis linked vector curves;
- transition easing;
- audio crossfade;
- multi-channel/spectral waveform expansion;
- MCP adapter;
- GPU compositor redesign;
- stable keyframe IDs;
- project-persisted sampled values;
- project-persisted FFmpeg commands/filter names/temp paths;
- renderer-specific easing semantics.

These remain deferred unless separately approved later.

---

## 5. Canonical Phase-14 Contract

The implementation MUST preserve the accepted ADR-004 contract.

### KF-01 — Clip-local keyframes
Keyframes remain clip-local property samples.

### KF-02 — Supported properties
Exactly:
`PositionX`, `PositionY`, `ScaleX`, `ScaleY`, `Rotation`, `Opacity`, `Volume`.

### KF-03 — Persisted keyframe fields
Schema v3 keyframes persist:
- property;
- offset: RationalTime;
- value: f32;
- outgoing_easing.

### KF-04 — Easing ownership
`K_i.outgoing_easing` governs only segment `K_i → K_(i+1)`.

### KF-05 — Last-keyframe invariant
The last keyframe in each property channel has no outgoing segment and its canonical `outgoing_easing` is Linear.

### KF-06 — Last-keyframe easing edit
Setting non-Linear easing on the last keyframe is rejected.

### KF-07 — Supported easing kinds
Exactly:
- Linear
- EaseIn
- EaseOut
- EaseInOut
- CubicBezier { x1, y1, x2, y2 }

### KF-08 — Control-point validation
All Custom CubicBezier control values are finite f64 values inside `[0,1]`.

### KF-09 — No overshoot
Phase 14 does not support overshoot curves.

### KF-10 — Solver
Cubic presets/custom use one deterministic Rust/core solver with exactly 32 bisection iterations.

### KF-11 — Exact endpoints
Semantic segment endpoints bypass the solver and return exact endpoint keyframe values.

### KF-12 — Rotation
Rotation remains numeric scalar interpolation, not shortest-path angular interpolation.

### KF-13 — Property validation boundary
Interpolation does not silently repair/clamp invalid endpoint property values.

### KF-14 — Semantic time
Keyframe equality/order must use semantic RationalTime logic, never raw `.offset.value` across potentially different rates.

### KF-15 — Mixed-rate identity
Equivalent timestamps at different rates represent one semantic keyframe location.

### KF-16 — Add behavior
Existing add/upsert behavior remains.

### KF-17 — Legacy add compatibility
When replacing an existing semantic keyframe:
- explicit easing replaces easing;
- omitted easing preserves existing easing.
New keyframes default to Linear if easing is omitted.

### KF-18 — Move collision
Move carries the full keyframe payload. Moving onto an occupied semantic timestamp replaces the destination according to existing semantics.

### KF-19 — Tail trim
Tail trim leaves all keyframes/easing unchanged; post-duration keyframes remain hidden persisted state.

### KF-20 — Head trim
Head trim by H shifts every keyframe offset by `-H`.

### KF-21 — Negative hidden offsets
Negative offsets produced by non-destructive head trim are valid retained canonical state.

### KF-22 — Head extension
Extending the head shifts retained keyframes back and restores prior animation timing.

### KF-23 — Split partition
Split partitions keyframes by semantic split time.

### KF-24 — Right rebase
Right-fragment keyframe offsets are rebased by `-S`.

### KF-25 — Boundary representation
A boundary keyframe exists on both fragments.

### KF-26 — Segment-crossing split
If split crosses an active segment, the boundary value is computed with the canonical evaluator.

### KF-27 — Nonlinear subdivision
Cubic/preset segments crossing a split are subdivided using canonical solver parameter resolution + de Casteljau.

### KF-28 — Split continuity
Evaluation before and after split must preserve the original animation function within the approved numeric tolerance.

### KF-29 — No keyframe IDs
No stable keyframe IDs are introduced.

### KF-30 — Schema v3
Schema v3 is mandatory.

### KF-31 — v2 → v3 migration
All existing v2 keyframes migrate with `outgoing_easing = Linear`, preserving previous visual semantics.

### KF-32 — Preserve existing v2 state
Migration preserves all Phase-13 transitions, fades, project metadata, assets, track/item state, history-relevant canonical state, and existing supported fields.

### KF-33 — Unknown easing
Unknown future easing kinds fail explicitly.

### KF-34 — Shared evaluator
Preview, single-frame export and full-export schedule generation share one canonical Rust evaluator.

### KF-35 — Full video sampling
Animated visual keyframes are evaluated once per final output video frame in Rust.

### KF-36 — Literal FFmpeg video application
FFmpeg video receives literal evaluated values through runtime commands only.

### KF-37 — No FFmpeg easing evaluator
FFmpeg does not implement Linear/CubicBezier interpolation semantics.

### KF-38 — Full audio sampling
Volume is evaluated on the final output audio sample grid in Rust.

### KF-39 — Gain envelope
The evaluated volume sequence is provided as a streamed temporary mono f32 gain envelope.

### KF-40 — Audio application
FFmpeg applies the envelope sample-wise using `amultiply`.

### KF-41 — Derived export artifacts
Command files and gain envelopes are derived temporary execution artifacts, never project state.

### KF-42 — Resource safety
Export automation artifact generation is bounded, cancellable, streamed, disk-preflighted and cleaned on every terminal path.

### KF-43 — Historical defect
Static full-export behavior for dynamic keyframes is a correctness bug and must be remediated.

### KF-44 — Parity detection
Parity/negative-control tests must detect ignored, linearized, mistimed, or frozen easing.

### KF-45 — Scope boundary
Transition easing, motion paths, springs, expressions, graph-editor generalization, waveform expansion, MCP and GPU compositor work remain outside Phase 14.

---

## 6. Schema v3 and Migration

### 6.1 Required schema shape

Introduce an easing descriptor concept equivalent to:

```rust
enum KeyframeEasing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}
```

and:

```rust
struct Keyframe {
    property: KeyframeProperty,
    offset: RationalTime,
    value: f32,
    outgoing_easing: KeyframeEasing,
}
```

Exact Rust naming may follow repository naming conventions, but persisted semantics must match ADR-004.

### 6.2 Migration

Implement deterministic v2 → v3 migration:

- preserve project identity;
- preserve revision semantics;
- preserve assets;
- preserve all track/item state;
- preserve source/timeline ranges;
- preserve transforms/crop/static opacity/volume;
- preserve Phase-13 CutTransitions;
- preserve supported single-clip fades;
- preserve every existing keyframe property/offset/value;
- set every migrated keyframe easing to Linear;
- preserve command ledger/client registry and other existing v2 canonical state;
- produce no visual change for valid existing projects.

### 6.3 Reader safety

- v3 is the new production schema after Phase 14 implementation.
- older readers limited to v2 must reject v3 as unsupported.
- unknown future easing variants must not silently degrade to Linear.
- migration failure must never overwrite the source project.

---

## 7. Canonical Easing Evaluator

### 7.1 Presets

Use the accepted numerical definitions:

- EaseIn = `(0.42, 0.0, 1.0, 1.0)`
- EaseOut = `(0.0, 0.0, 0.58, 1.0)`
- EaseInOut = `(0.42, 0.0, 0.58, 1.0)`

Presets should remain persisted as named variants when directly selected.

Split-derived curves may become Custom CubicBezier.

### 7.2 Mathematical evaluation

For keyframes `K0`, `K1`, semantic times `T0 < T1`, and sample `T`:

- `T ==semantic T0` → return `K0.value` exactly;
- `T ==semantic T1` → return `K1.value` exactly;
- otherwise compute semantic normalized progress `u = (T-T0)/(T1-T0)`;
- Linear: `e = u`;
- Cubic/preset:
  - solve `x(q)=u` with exactly 32 bisection iterations;
  - `e = y(q)`;
- property value:
  - `v = v0 + (v1-v0)*e`.

The dimensionless ratio may use f64 after exact RationalTime ordering/subtraction has established the interval.

### 7.3 Before/after key range

Preserve established endpoint-hold behavior:

- before first keyframe → first keyframe value;
- after last keyframe → last keyframe value.

### 7.4 Ordering

All per-property keyframe views must be ordered by semantic RationalTime ordering.

Do not sort by raw `.offset.value`.

Mixed-rate equal semantic timestamps collapse under upsert semantics.

---

## 8. Command Service Work

### 8.1 `ClipKeyframeAdd`

Extend payload in a backward-compatible way for existing internal/automation callers.

Required behavior:

- locate collisions by `(property, semantic time)`;
- if absent:
  - insert value;
  - use provided easing or Linear;
- if present:
  - replace value;
  - replace easing only when caller explicitly provided one;
  - otherwise preserve existing easing;
- canonical ordering restored deterministically;
- last-keyframe invariant normalized.

### 8.2 `ClipKeyframeRemove`

- semantic time lookup;
- remove matching keyframe;
- outgoing segment disappears with removed keyframe;
- if predecessor becomes final, normalize predecessor easing to Linear;
- no synthetic curve remapping.

### 8.3 `ClipKeyframeMove`

- semantic source lookup;
- semantic destination lookup;
- semantic source == destination → no-op;
- payload moves with value + easing;
- occupied destination is replaced;
- deterministic ordering;
- normalize final-keyframe invariant.

### 8.4 Easing edit command

Add one explicit repository-consistent command, conceptually:

`ClipKeyframeEasingSet`

Inputs must identify:
- item;
- property;
- semantic offset;
- easing descriptor.

Validation:
- item/keyframe exists;
- easing descriptor valid;
- target keyframe has a successor;
- property is supported;
- invalid custom controls reject atomically.

### 8.5 Errors

Use stable structured errors. Exact enum naming may follow current conventions, but externally visible categories must distinguish at least:

- invalid keyframe easing;
- no outgoing segment;
- invalid keyframe time where applicable;
- unsupported keyframe property;
- unsupported keyframe easing/schema.

Do not collapse all advanced-easing failures into ambiguous generic text if automation needs machine-actionable handling.

### 8.6 Dry-run / undo / automation

All commands must use the existing shared Command Service behavior:

- base revision;
- dry-run;
- atomic mutation;
- history snapshot;
- undo/redo;
- durable idempotency where applicable;
- CLI/session automation same command path.

No second reducer.

---

## 9. Keyframe Editing Lifecycle

### 9.1 Duplicate

Duplicate clip/item:

- deep-copy all keyframes;
- copy easing descriptors exactly;
- no keyframe IDs exist or need regeneration;
- duplicate remains independent after item clone.

### 9.2 Split — exact keyframe at boundary

Split at clip-local semantic time `S`.

For each property channel:

- offset `< S` → left only, unchanged;
- offset `> S` → right only, rebased `offset - S`;
- offset `== S`:
  - left copy at S;
  - right copy at 0;
  - exact value preserved;
  - left terminal boundary easing Linear;
  - right boundary retains original outgoing easing only if it still has a successor; otherwise Linear.

Left keeps original item ID; right gets a new item ID.

### 9.3 Split — crossing Linear segment

If `K0 < S < K1`:

- evaluate canonical value at S;
- synthesize left boundary keyframe at S;
- synthesize right boundary keyframe at 0;
- Linear remains Linear on both derived segments;
- preserve evaluation equivalence before/after split.

### 9.4 Split — crossing cubic/preset segment

Use the authoritative procedure:

1. compute semantic `u_s`;
2. solve cubic parameter `q` with the canonical solver;
3. evaluate boundary value;
4. split original cubic at `q` using de Casteljau;
5. normalize left and right subcurves to unit `(0,0) → (1,1)` coordinates;
6. store derived curves as Custom CubicBezier when necessary;
7. ensure all resulting controls remain valid under Phase-14 no-overshoot constraints;
8. preserve animation function on both fragments within numeric tolerance.

If subdivision would produce an invalid/non-representable canonical curve under the accepted contract, stop and report rather than approximating silently.

### 9.5 Tail trim

- do not delete/rebase keyframes;
- hidden post-duration keys remain persisted;
- extending tail restores visibility;
- no synthetic boundary key.

### 9.6 Head trim

For head trim by H:

- update existing timeline/source ranges according to current edit semantics;
- shift every keyframe offset by `-H`;
- allow resulting negative offsets as retained hidden canonical state;
- preserve animation relative to retained source/timeline content;
- no destructive deletion.

Head extension shifts retained offsets back appropriately.

### 9.7 History

Split/trim plus all dependent keyframe/easing mutations must be one logical history entry.

Undo/redo must restore exact:
- item ranges;
- keyframe set;
- RationalTime offsets;
- values;
- easing descriptors.

---

## 10. Composition and Preview

### 10.1 Evaluator extraction

Prefer extracting mathematical interpolation from `CompositionVisualLayer::evaluate_property` into one pure core helper usable by visual and audio channels.

Do not duplicate Bézier solving in:
- React;
- PreviewCanvas;
- FFmpeg expression strings;
- export schedule compiler;
- automation.

### 10.2 Preview

Preview must consume canonical evaluated state from Rust exactly as today.

Frontend remains presentation/edit UI, not authoritative interpolation engine.

### 10.3 Mixed-rate correctness

Add fixtures with semantically equal/differently represented RationalTimes.

All:
- lookup;
- collision;
- sorting;
- endpoint tests;
- split boundaries;
- trim behavior;
- evaluator progress

must obey semantic comparison.

---

## 11. Full Video Export Automation

### 11.1 Derived schedule

Introduce a derived/non-persisted execution representation equivalent to:

```text
ExportAutomationSchedule
  visual schedules by clip/branch/frame
  audio gain-envelope descriptors
  temporary-artifact metadata
```

Do not persist it in `.tcproj`.

### 11.2 Sampling

For every final output video frame intersecting a clip contribution:

- derive canonical frame RationalTime;
- derive clip-local offset;
- evaluate animated properties in Rust;
- produce literal transform/opacity values.

Do not use arbitrary float seconds as canonical project time.

### 11.3 Runtime command transport

POC-validated rule:

- frame 0 values installed as initial filter options;
- frame `n > 0` commands dispatched between `T[n-1]` and `T[n]`, using the proven midpoint strategy.

Command timestamps are backend dispatch details only.

### 11.4 Filter mapping

Maintain existing transform order and geometry semantics.

Map literal values to named runtime-command-capable filters:

- ScaleX/ScaleY → scale dimensions;
- Rotation → rotate angle;
- PositionX/PositionY → overlay x/y;
- Opacity → alpha multiplier.

Use unique deterministic filter instance targeting.

### 11.5 Windows path safety

Respect FFmpeg filter-option escaping requirements for command-file paths, including Windows drive-colon escaping.

No raw user/agent string may become arbitrary filter syntax.

### 11.6 Dynamic-size safety

Preserve the POC-discovered RGBA/dynamic-buffer requirements needed to avoid auto-inserted static scale locking.

Do not change project/render semantics to work around backend behavior.

---

## 12. Phase-13 CrossDissolve Coexistence

Phase 14 must extend rather than bypass ADR-003.

For clips participating in CrossDissolve:

- ordinary contributions receive automation outside the transition interval;
- ordinary A/B remain suppressed inside `[T0,T1)`;
- transition participant branches receive their own correctly timed evaluated automation;
- transition branches continue:
  - handle extraction;
  - normalized canvas RGBA;
  - premultiply;
  - bounded `xfade=transition=fade:duration=D:offset=0`;
  - unpremultiply;
  - reintegration;
- no double rendering;
- no persisted timeline overlap;
- total project/export duration unchanged.

At least one integration test must animate a property continuously through `T0 → C → T1` and prove it does not freeze.

---

## 13. Full Audio Volume Export

### 13.1 Sampling

For each keyframed Volume clip:

- choose final export audio sample rate from the actual output profile/current supported export policy;
- evaluate canonical Volume at every final sample RationalTime;
- write one mono f32 gain value per sample.

Do not hard-code 48 kHz as project truth if current export profile can differ; the schedule must use the actual final sample rate.

### 13.2 Envelope transport

- streamed sequential write;
- bounded memory;
- app-controlled temporary path;
- deterministic length/alignment;
- no project persistence.

### 13.3 FFmpeg application

Use the POC-proven `amultiply` path with channel handling that preserves unity gain exactly across supported output layouts.

### 13.4 Resource safety

Before generation:
- estimate required temporary bytes;
- fail safely if space is insufficient.

During generation/render:
- cancellation checks;
- no unbounded RAM accumulation.

Every terminal path:
- success cleanup;
- failure cleanup;
- cancellation cleanup.

The POC evidence of approximately 691.2 MB/hour at 48kHz mono f32 must be considered in tests and UX/error handling.

---

## 14. UI Scope

Minimum required UI only.

### 14.1 Existing keyframe controls

Preserve current add/remove/jump/move behavior.

Do not regress existing inspector/timeline interactions.

### 14.2 Easing selection

Expose an interpolation/easing selector for a keyframe with an outgoing segment:

- Linear
- Ease In
- Ease Out
- Ease In Out
- Custom

### 14.3 Custom fields

For Custom:
- x1
- y1
- x2
- y2

with inline finite/range validation.

### 14.4 Last keyframe UX

Do not present a meaningful non-Linear outgoing easing state on the last keyframe.

UI should reflect command/domain rule cleanly.

### 14.5 Explicit UI deferrals

No:
- draggable curve editor;
- graph editor;
- timeline tangent handles;
- velocity view;
- preset library expansion;
- motion-path editor.

---

## 15. Automation Surface

Reuse existing automation/session transport.

Capability inspection must expose enough structured information for an agent/client to discover:

- supported keyframe properties;
- supported easing kinds;
- custom control bounds;
- schema/project capability version;
- easing-set command availability.

Automation must be able to:
- inspect persisted easing;
- dry-run easing mutation;
- apply easing mutation;
- add/move/remove keyframes using semantic RationalTime behavior;
- save/reopen;
- retry idempotently;
- receive structured validation failures.

No automation-only keyframe evaluator.

---

## 16. Implementation Slices

Implementation should proceed in the following gated order.

### Slice A — Schema v3 + canonical time/easing domain

Deliver:
- easing domain enum/descriptor;
- Keyframe schema extension;
- v2→v3 migration;
- project/load validation;
- semantic time ordering helpers for keyframe paths;
- mixed-rate cleanup;
- easing validation;
- last-keyframe invariant support;
- migration tests.

Gate A:
- old valid v2 fixtures migrate losslessly;
- all migrated keyframes are Linear;
- v3 save/reopen exact;
- old/future unsupported schema behavior explicit;
- no Phase-13 transition/fade regression;
- no raw mixed-rate keyframe equality/order remains in correctness paths.

### Slice B — Canonical evaluator + commands

Deliver:
- pure canonical easing evaluator;
- preset definitions;
- 32-step bisection;
- exact endpoint semantics;
- add/remove/move semantic-time remediation;
- backward-compatible add/upsert easing behavior;
- easing-set command;
- structured errors;
- undo/redo;
- automation command exposure.

Gate B:
- Linear behavior regression-identical;
- preset/custom numeric fixtures pass;
- mixed-rate tests pass;
- invalid controls reject atomically;
- last-keyframe rule enforced;
- command dry-run/apply/history parity passes.

### Slice C — Keyframe lifecycle remediation

Deliver:
- duplicate verification;
- split partition/rebase;
- boundary synthesis;
- Linear continuity;
- Cubic/preset de Casteljau subdivision;
- head/tail trim semantics;
- negative hidden-offset handling;
- undo/redo exactness.

Gate C:
- split continuity passes for Linear, preset and custom cubic;
- trim extension restores hidden automation;
- no clone-all/no-rebase split behavior remains;
- dependent mutations are atomic.

### Slice D — Preview / composition integration

Deliver:
- shared evaluator integration;
- visual/audio sampling cleanup as needed;
- semantic ordering;
- frontend model/types updated for schema/easing;
- minimal easing UI.

Gate D:
- preview samples correct for all easing kinds;
- frontend contains no authoritative interpolation math;
- existing UI behavior remains stable;
- mixed-rate preview fixtures pass.

### Slice E — P14-PREREQ-01 + full video export

Deliver first:
- existing Linear keyframes affect full video export.

Then:
- ExportAutomationSchedule video portion;
- per-frame Rust evaluation;
- command-file generation;
- runtime filter instance targeting;
- midpoint dispatch;
- temp lifecycle;
- CrossDissolve transition-branch automation.

Gate E:
- legacy Linear full export no longer static;
- Position/Scale/Rotation/Opacity parity;
- frame timing at integer and fractional project rates;
- CrossDissolve animation through T0/C/T1;
- negative control detects ignored/frozen/one-frame-late automation;
- output duration unchanged.

### Slice F — Full audio Volume export

Deliver:
- actual final-sample-rate schedule;
- streamed f32le envelope;
- amultiply mapping;
- temp-space preflight;
- lifecycle cleanup;
- cancellation safety.

Gate F:
- sample-index parity;
- stereo/current supported layout correctness;
- one-sample-shift negative control;
- output duration unchanged;
- low-disk failure safe;
- success/failure/cancel cleanup verified.

### Slice G — Automation/UI completeness + full parity/regression

Deliver:
- capability discovery;
- inspect/dry-run/apply;
- persistence;
- final UI wiring;
- full semantic/raster parity fixtures;
- negative controls;
- docs/status notes needed for implementation reporting only.

Gate G:
- all acceptance criteria pass;
- full Rust suite;
- frontend tests;
- typecheck;
- clippy;
- fmt;
- frontend production build;
- cargo build;
- targeted real-media export tests with app-owned FFmpeg 9.0.2.

Do not update Phase 14 to COMPLETE here; completion still requires Codex report → ChatGPT review → durable final reconciliation.

---

## 17. Acceptance Criteria

### P14-AC-01 — Schema v3
Production project schema is v3 after Phase 14 implementation.

### P14-AC-02 — Lossless v2 migration
Valid v2 projects migrate without losing existing project/transition/fade/keyframe state.

### P14-AC-03 — Linear migration
Every migrated v2 keyframe has canonical Linear outgoing easing and preserves previous evaluated results.

### P14-AC-04 — Future-reader safety
Older supported readers reject v3 rather than silently dropping easing; unknown future schema/easing is explicit failure.

### P14-AC-05 — Easing kinds
Exactly the approved five easing kinds are production-supported.

### P14-AC-06 — Control validation
Custom controls are finite and inside `[0,1]`; invalid values reject atomically.

### P14-AC-07 — Deterministic solver
Cubic/preset easing uses the single 32-iteration Rust/core bisection solver.

### P14-AC-08 — Exact endpoints
Sampling exactly at semantic segment endpoints returns exact stored endpoint values.

### P14-AC-09 — Semantic RationalTime
All keyframe correctness paths use semantic equality/order; mixed-rate equivalent timestamps behave as one location.

### P14-AC-10 — Linear regression
Existing valid Linear preview/single-frame results remain equivalent within existing numeric tolerances.

### P14-AC-11 — Upsert compatibility
Existing add/upsert behavior is preserved, including easing-preservation for legacy callers omitting easing.

### P14-AC-12 — Move semantics
Move carries value/easing; semantic no-op and occupied-destination replacement match approved contract.

### P14-AC-13 — Last-keyframe invariant
Last keyframe cannot hold effective non-Linear outgoing easing.

### P14-AC-14 — Easing command
UI and automation can mutate easing through the shared Command Service with dry-run/apply/undo/redo.

### P14-AC-15 — Duplicate
Duplicate deep-copies keyframes/easing without shared mutable state or semantic drift.

### P14-AC-16 — Split exact-boundary
Exact split-boundary keyframes are represented/rebased correctly on both fragments.

### P14-AC-17 — Split Linear continuity
Splitting inside a Linear segment preserves pre-split evaluation across both fragments.

### P14-AC-18 — Split nonlinear continuity
Splitting inside preset/custom cubic segments uses exact subdivision and preserves the original function within evaluator tolerance.

### P14-AC-19 — Tail trim
Tail trim retains hidden future keyframes and extension restores them.

### P14-AC-20 — Head trim
Head trim shifts all keyframe offsets by `-H`; hidden negative offsets are retained and head extension restores timing.

### P14-AC-21 — Atomic lifecycle history
Split/trim dependent keyframe changes are one logical history operation and undo/redo restores exact state.

### P14-AC-22 — Preview
Preview uses the canonical Rust evaluator for all seven supported channels.

### P14-AC-23 — No frontend evaluator
Production TypeScript/React code does not implement authoritative easing/interpolation math.

### P14-AC-24 — P14-PREREQ-01 fixed
Full export honors existing dynamic Linear keyframes.

### P14-AC-25 — Full visual export schedule
Position/Scale/Rotation/Opacity animated values are evaluated per final output frame in Rust and applied literally by FFmpeg.

### P14-AC-26 — Frame-exact dispatch
Runtime video command dispatch produces the intended value on every tested output frame for integer and fractional rates.

### P14-AC-27 — No FFmpeg easing
No Linear/Cubic easing formula becomes duplicated inside FFmpeg expressions/filter syntax.

### P14-AC-28 — CrossDissolve coexistence
Animated transition participants are correctly automated inside ADR-003 transition branches without double rendering or duration change.

### P14-AC-29 — Full audio Volume export
Volume automation is evaluated per final audio sample and applied through the gain envelope/amultiply path.

### P14-AC-30 — Audio sample accuracy
Selected decoded samples match source × canonical gain within approved float tolerance; one-sample-shift negative control fails.

### P14-AC-31 — Resource safety
Command files/envelopes are streamed/bounded, disk-preflighted, cancellable and cleaned on success/failure/cancel.

### P14-AC-32 — Persistence
Schema-v3 keyframe/easing state round-trips exactly through save/reopen.

### P14-AC-33 — Automation
Inspect/dry-run/apply/idempotency/stale-revision behavior works through the existing automation path.

### P14-AC-34 — UI minimum
The approved minimal easing selector and Custom numeric inputs are usable without introducing a graph editor.

### P14-AC-35 — Semantic parity
Preview, single-frame export and full-export schedules agree on canonical evaluated values at representative timestamps.

### P14-AC-36 — Visual parity
Existing approved geometry/raster parity tolerances are retained for relevant fixtures.

### P14-AC-37 — Negative controls
Tests fail when:
- full export ignores keyframes;
- Custom easing is replaced with Linear;
- raw `.offset.value` ordering is used on mixed-rate fixtures;
- transition-branch automation freezes;
- video schedule is shifted one frame;
- audio envelope is shifted one sample;
- split skips required nonlinear subdivision.

### P14-AC-38 — Regression health
No regression to Phase 13 CrossDissolve, fades, export safety, automation idempotency, persistence, recovery, or existing editing behavior outside approved keyframe lifecycle changes.

### P14-AC-39 — App-owned media binary
Real-media Phase-14 export integration tests execute with the Phase-12 app-owned/pinned FFmpeg 9.0.2 resolver path, not ambient PATH.

### P14-AC-40 — Verification gate
Before completion:
- targeted Phase-14 tests pass;
- full `cargo test` passes;
- `cargo clippy -- -D warnings` passes;
- `cargo fmt -- --check` passes;
- frontend tests pass;
- TypeScript typecheck passes;
- frontend production build passes;
- `cargo build` passes.

---

## 18. Required Test Matrix

At minimum include deterministic fixtures for:

- easing math and exact endpoints;
- invalid/NaN/Inf control points;
- mixed-rate RationalTime equality/order;
- Schema v2→v3 and v3 round-trip;
- command upsert/move/remove/easing-set/history;
- split exact boundary / Linear / preset / Custom;
- head/tail trim and restore;
- preview all seven properties;
- full video export Position/Scale/Rotation/Opacity;
- integer and fractional frame rates;
- CrossDissolve automation through T0/C/T1;
- full audio Volume sample accuracy;
- temp lifecycle success/failure/cancel/low-disk;
- automation inspect/dry-run/apply;
- all required negative controls.

---

## 19. Performance / Resource Guardrails

Phase 14 is not the general performance-optimization phase, but implementation must not introduce obviously unbounded behavior.

Required constraints:

- visual schedule generation may be O(output_frames × animated_visual_channels);
- audio envelope generation must stream with bounded memory;
- command file generation must stream or remain bounded for expected project sizes;
- temporary storage requirement must be estimated before audio envelope generation;
- cancellation must interrupt long schedule/envelope generation;
- no derived schedule becomes permanent project state.

POC evidence showed:
- dynamic video command transport has measurable overhead versus static export;
- 60-minute 48kHz mono f32 envelope is approximately 691.2 MB.

These are accepted planning costs, not permission to ignore pathological scaling.

If representative real projects reveal an unexpectedly severe performance cliff that makes the approved architecture impractical, stop and report rather than silently replacing ADR-004.

---

## 20. Stop Conditions

Pause implementation and report to ChatGPT if any of the following occurs:

1. Schema v3 migration cannot preserve valid v2 project state losslessly.
2. Older-reader fail-fast boundary cannot be maintained.
3. Canonical cubic subdivision under the no-overshoot contract cannot preserve split continuity for valid supported curves.
4. A required keyframe path needs stable keyframe IDs to be correct.
5. Full video automation cannot coexist with production CrossDissolve branches using the POC-validated architecture.
6. FFmpeg runtime command behavior differs materially from the retained POC under production graph construction.
7. Audio envelope alignment cannot remain sample-accurate under actual export timing/layout.
8. Temp resource usage cannot be bounded/preflighted/cleaned safely.
9. Implementing Phase 14 would require a generalized compositor, graph editor, motion-path system, transition easing framework, MCP work, or other explicitly deferred architecture.
10. Preview/export semantic parity cannot be bounded under the approved evaluator.
11. Implementation requires changing ADR-004 semantics rather than routine coding choices.

Do not weaken acceptance criteria to bypass a stop condition.

---

## 21. Completion Evidence Required from Codex

The final Codex implementation report must include:

1. files changed;
2. Schema v3 shape;
3. v2→v3 migration behavior and tests;
4. canonical easing evaluator implementation;
5. semantic RationalTime remediation evidence;
6. command/API changes;
7. split behavior with Linear/preset/custom evidence;
8. trim behavior with hidden-key restoration evidence;
9. `P14-PREREQ-01` remediation proof;
10. full-video ExportAutomationSchedule architecture as actually implemented;
11. Phase-13 CrossDissolve coexistence evidence;
12. full-audio gain-envelope implementation and sample-accuracy evidence;
13. temp resource/cancellation cleanup evidence;
14. UI changes;
15. automation capability/command changes;
16. preview/export parity evidence;
17. negative-control results;
18. exact targeted test counts/results;
19. full test count/result;
20. clippy/fmt/typecheck/frontend build/cargo build results;
21. real-media app-owned FFmpeg 9.0.2 proof;
22. regressions/known issues;
23. deviations from ADR-004 or this plan, if any;
24. deferred items;
25. proposed classification:
   - COMPLETE
   - COMPLETE WITH DEFERRED ITEMS
   - INCOMPLETE
   - BLOCKED

Codex must not update Phase 14 to COMPLETE before ChatGPT review.

---

## 22. Phase Completion Rule

Required lifecycle remains:

```text
Approved plan
→ Codex implementation
→ Codex completion report
→ ChatGPT independent review
→ completion classification
→ durable PLAN / PROJECT_STATUS / architecture / debt reconciliation
→ next milestone decision
```

Do not select or plan Phase 15 until Phase 14 has completed the full lifecycle.

---

## 23. Approval

Current state:

**RATIFIED POST-IMPLEMENTATION**

- User explicit ratification: `Duyệt Phase 14` (following full review, remediation, and verification).
- Independent review: Accepted by ChatGPT.

