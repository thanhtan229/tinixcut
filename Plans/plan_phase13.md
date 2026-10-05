# plan_phase13.md — Transition Compositor & CrossDissolve

## 0. Status

**Phase:** 13 — Transition Compositor & CrossDissolve  
**Plan status:** COMPLETE — accepted by ChatGPT  
**Research gate:** COMPLETE  
**Implementation:** COMPLETE  
**Governing ADR:** `ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md`

Implementation completed and accepted by ChatGPT.

---

## 1. Objective

Deliver one production-supported, deterministic **video-only adjacent-cut `CrossDissolve`** that is:

- cut-owned rather than clip-owned;
- persisted in Schema v2;
- editable through the authoritative Command Service;
- preserved correctly across move/trim/split/delete/duplicate and undo/redo;
- consumed through one normalized transition semantic by both Preview and Export;
- previewed with the approved isolated `PreviewTransitionGroup` + `plus-lighter` path;
- exported through normalized RGBA branches → premultiply → bounded `xfade=fade` → unpremultiply → existing overlay chain;
- covered by migration, command, persistence, preview/export parity, automation, and regression tests.

Phase 13 must implement the accepted CD-01..CD-43 contract without expanding into a general transition/compositor framework.

---

## 2. Governing Sources

Implementation must follow, in order of authority:

1. repository code — actual current implementation;
2. `ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md`;
3. Phase 13 CD-01..CD-43 contract in `RESEARCH.md`;
4. relevant transition/composition/persistence sections of `ARCHITECTURE.md`;
5. Phase 13 roadmap section in `PLAN.md`;
6. `PROJECT_STATUS.md`.

If implementation reality contradicts ADR-003 or the CD contract, stop that contradictory work and report the evidence. Do not invent a parallel solution.

---

## 3. In Scope

### 3.1 Domain and Schema v2

Add a track-level cut-transition representation equivalent to:

```text
Track
├── items[]
└── transitions[]
    └── CutTransition
        ├── transition_id
        ├── kind = CrossDissolve
        ├── outgoing/left item id
        ├── incoming/right item id
        └── duration: RationalTime
```

Requirements:

- `CrossDissolve` is owned by the cut relation between two immediate adjacent video clips on the same track.
- No persisted physical overlap.
- No persisted `CutAlignment` field in Phase 13.
- Centered semantics are derived only from deterministic L/R tick splitting.
- Existing production-supported item-level Fade semantics remain intact.
- Backend-specific state must not enter the project schema.

### 3.2 Schema v1 → v2 Migration

Implement deterministic migration:

- bump current project schema to v2;
- v1 → v2 initializes track-level cut transition collections empty;
- preserve all supported v1 item-level Fade behavior losslessly;
- never invent a `CrossDissolve` during migration;
- malformed/unsupported legacy `CrossDissolve` state must fail explicitly or remain explicitly unsupported according to the migration/error policy;
- migration failure must not overwrite the original project;
- save/reopen migrated projects as valid Schema v2.

### 3.3 Cut Transition Commands

Introduce explicit typed cut-transition mutations through the existing Command Service. Exact Rust/API names may follow repository conventions, but the semantic operations must cover:

- create `CrossDissolve` for an eligible adjacent pair;
- change duration;
- remove transition.

Do not overload or reinterpret existing clip-owned Fade semantics merely to avoid introducing a cut-owned operation.

All UI and automation callers must use the same authoritative operations.

### 3.4 Handle and Eligibility Validation

Before create or duration update:

- both participants must be video clips;
- same track;
- immediate adjacency;
- `A.end == B.start`;
- duration > 0;
- duration is rescaled to the canonical project timeline rate;
- `L_ticks = floor(D_ticks / 2)`;
- `R_ticks = D_ticks - L_ticks`;
- `T0 = C - L`;
- `T1 = C + R`;
- transition interval is half-open `[T0, T1)`;
- outgoing editorial extent >= L;
- incoming editorial extent >= R;
- outgoing postroll >= R;
- incoming preroll >= L.

Stable error semantics:

- known but insufficient source handle → `INSUFFICIENT_TRANSITION_HANDLE`;
- source duration/handle availability cannot be proven → `TRANSITION_HANDLE_UNVERIFIABLE`;
- image/synthetic/non-video participant → `UNSUPPORTED_TRANSITION_MEDIA`.

No implicit clamp, auto-shortening, frame freeze, or approximate fallback.

### 3.5 Dependent Editing Semantics

Existing core edits must preserve the cut-transition invariants atomically.

#### Move
- If the exact participant pair remains immediate-adjacent on the same track and validation still passes, preserve transition identity/duration.
- If the original cut is destroyed, remove the dependent transition atomically.
- Never reconnect the transition to a different neighbor automatically.

#### Trim
- If trim destroys the cut, remove the transition atomically.
- If cut remains but editorial extent or source handles become invalid, reject the whole trim atomically.
- Never auto-shorten the transition.

#### Delete
- Deleting either participant removes the cut transition in the same logical edit/history entry.

#### Duplicate
- A duplicated clip does not inherit or clone the cut transition.
- If insertion destroys the original adjacency, remove the original transition by the structural invalidation rule.

#### Split
Use the real fragment-ID mapping produced by the reducer:

- split outside the media region consumed by the transition may retarget the transition to the resulting fragment still incident to the cut;
- outgoing split uses the fragment that remains adjacent to the incoming participant;
- incoming split uses the fragment that remains adjacent to the outgoing participant;
- split inside the transition-used region is rejected in Phase 13;
- transition logic must not assume array position is identity.

All dependent mutations belong to the same snapshot-history entry as the triggering edit.

### 3.6 Normalized Composition Semantics

Extend the normalized composition layer with an explicit two-input transition semantic equivalent to `TransitionPlan`.

It must provide enough backend-independent information for:

- transition identity;
- track/order;
- participant identities;
- `[T0, T1)` interval;
- evaluated source mapping including handles;
- linear progress;
- existing transform/crop/placement/opacity evaluation;
- endpoint-hold visual/keyframe behavior outside each clip's normal editorial interval.

Canonical pixel semantic:

```text
RenderedA = fully evaluated premultiplied RGBA contribution
RenderedB = fully evaluated premultiplied RGBA contribution

TransitionResult(t) = (1 - p(t)) * RenderedA(t) + p(t) * RenderedB(t)
```

`TransitionResult` is one contribution which is then composited normally into the track/layer stack.

Do not introduce an arbitrary node/effect graph.

### 3.7 Preview

Implement the authoritative Phase 13 preview path:

- `PreviewFrameState` explicitly represents a transition group rather than emitting the pair as ordinary sibling active layers;
- use an isolated `PreviewTransitionGroup` DOM stacking/compositing context;
- outgoing/incoming contributions retain existing clip opacity and multiply by `(1-p)` / `p`;
- `plus-lighter` is used only inside the isolated transition group;
- the completed group is composited normally into the existing stack;
- ordinary two-layer source-over CrossDissolve is prohibited;
- no Canvas/offscreen or second semantic fallback in Phase 13.

Verify that the packaged/current WebView2 baseline supports the required `plus-lighter` behavior. If it does not, Phase 13 is not complete; do not silently degrade to source-over.

### 3.8 Export

Compile the same normalized transition semantic into FFmpeg:

```text
normalized full-canvas RGBA A/B
→ premultiply
→ bounded aligned xfade=transition=fade
→ unpremultiply
→ one transition-result contribution
→ existing overlay chain
```

Requirements:

- transition branches use source handles;
- matching resolution, FPS, pixel format, and timebase;
- PTS normalized before `xfade`;
- transition boundaries derive from `RationalTime`, not independent float rounding;
- ordinary A and B contributions are suppressed in `[T0, T1)`;
- outside `[T0, T1)`, current overlay behavior remains unchanged;
- total timeline/export duration is unchanged;
- audio remains a butt-cut and must not receive `acrossfade`.

### 3.9 Minimal UI Integration

Provide the smallest coherent editor interaction needed to make the feature usable:

- select an eligible adjacent video cut/pair;
- create CrossDissolve;
- adjust duration;
- remove CrossDissolve;
- display structured validation failures.

Reuse existing inspector/transition UI patterns where practical. Do not build a transition marketplace/browser, draggable transition library, animation presets, or generalized transition UI.

### 3.10 Automation Surface

Update capability/automation behavior so the supported cut-transition operations are available through the same Command Service.

Required:

- inspect/capabilities advertises `CrossDissolve` only after implementation is genuinely supported;
- dry-run validates the exact same eligibility/handle rules as apply;
- apply uses the same cut-transition commands as UI;
- schema v2 save/reopen is respected by CLI and long-lived stdio session;
- stale revision, idempotency, transaction rollback, and receipts remain unchanged;
- no second automation-only reducer or transition mutation path.

---

## 4. Explicitly Out of Scope

Phase 13 must not add:

- audio crossfade / `acrossfade`;
- non-linear transition easing;
- start/end/custom transition alignment modes;
- Fade redesign or re-homing;
- arbitrary physical overlap on the canonical timeline;
- wipes, pushes, zooms, 3D transitions, masks, shader transitions;
- generalized compositor/effect graph;
- WebGL/GPU compositor redesign;
- transition plugin framework;
- non-linear Bezier keyframes;
- spectral/multi-channel waveform work;
- HDR/linear-light/color-management redesign;
- H.265/AV1 or unrelated codec work;
- hardware acceleration qualification;
- MCP implementation.

---

## 5. Implementation Sequence

Implementation should proceed in gated slices. Do not begin later rendering/UI slices until the preceding semantic slice is green.

### Slice A — Schema v2 + Migration Foundation

Implement:

- `CutTransition` domain representation;
- `Track.transitions`;
- schema version bump;
- v1 → v2 migration;
- serialization/deserialization and domain validation;
- preservation of existing item-level Fade behavior.

Required tests before proceeding:

- clean v1 fixture → v2 migration;
- existing Fade semantics preserved;
- v2 save/reopen exact semantic round trip;
- migration does not invent CrossDissolve;
- malformed unsupported legacy CrossDissolve is handled explicitly;
- invalid/dangling cut transition is rejected;
- migration failure leaves source file safe.

**Gate A:** migration/domain tests green.

### Slice B — Command Service + Edit Lifecycle

Implement:

- create/update/remove cut-transition operations;
- eligibility/handle validator;
- stable structured errors;
- dependent mutation rules for move/trim/delete/duplicate/split;
- snapshot history/undo/redo behavior;
- dry-run equivalence.

Required tests:

- adjacent valid create;
- zero duration rejected;
- video/video restriction;
- different track/gap/overlap rejected;
- exact sufficient handles accepted;
- one-tick short handles rejected;
- unknown duration rejected as unverifiable;
- move/trim/delete/duplicate/split lifecycle cases;
- undo/redo exact transition identity restoration;
- failed operation/transaction leaves canonical state unchanged.

**Gate B:** domain/command/history tests green.

### Slice C — Normalized TransitionPlan

Implement:

- deterministic L/R and `[T0,T1)` derivation;
- transition source mapping with preroll/postroll;
- endpoint-hold property evaluation;
- one normalized two-input transition representation consumed by both backends.

Required tests:

- odd-duration deterministic split;
- p=0 / midpoint / last sample before T1 / exact T1;
- source timestamps on both sides of cut;
- transform/crop/opacity state included before blend;
- no backend-specific syntax in canonical project state.

**Gate C:** semantic composition tests green.

### Slice D — PreviewTransitionGroup

Implement the isolated DOM `PreviewTransitionGroup`.

Required verification:

- `plus-lighter` capability on target WebView2;
- opaque full-frame A/B;
- clip opacity < 1;
- transformed partial overlap;
- crop exposing lower tracks;
- seek/scrub determinism;
- no ordinary source-over fallback.

**Gate D:** preview semantic and targeted UI tests/manual QA green.

### Slice E — FFmpeg Export Transition Segment

Implement bounded transition compilation:

- normalized full-canvas RGBA branches;
- premultiply;
- bounded aligned `xfade=fade`;
- unpremultiply;
- splice/overlay integration;
- suppression of ordinary A/B contributions inside transition interval.

Use the pinned/app-owned Phase 12 FFmpeg path; do not regress to system PATH.

Required tests:

- valid real-media CrossDissolve export;
- exact output duration preservation;
- odd-duration/frame-boundary cases;
- transform/crop/opacity participant cases;
- cancellation/failure output safety unchanged;
- offline media failure remains explicit;
- no audio crossfade.

**Gate E:** export integration tests green.

### Slice F — Preview/Export Parity Gate

Extend the existing Gate E parity suite rather than creating an unrelated testing system.

Minimum deterministic fixtures:

- opaque full-frame A/B;
- participant opacity < 1;
- transformed partial overlap;
- crop revealing lower track/background;
- alpha-capable case if supported by the existing video path;
- p=0;
- midpoint;
- last active sample before T1;
- exact T1 boundary;
- odd `RationalTime` duration;
- exact maximum valid duration;
- exact-sufficient A postroll;
- exact-sufficient B preroll;
- one-tick-short A;
- one-tick-short B;
- offline media with persisted duration;
- save/reopen schema v2;
- v1→v2 migration.

Add a negative control that intentionally uses wrong blend/progress/source-over behavior and verify the parity gate fails.

**Gate F:** semantic, geometry/timing, and raster parity meet existing project tolerances.

### Slice G — Minimal UI + Automation + Full Regression

Complete:

- minimal UI create/duration/remove interaction;
- capabilities/inspect exposure;
- CLI/stdio dry-run/apply/save-reopen;
- automation E2E;
- regression suite.

**Gate G:** full Phase 13 acceptance evidence complete.

---

## 6. Acceptance Criteria

### Domain / Persistence

**P13-AC-01** — Schema v2 persists track-level cut-owned `CrossDissolve` without physical timeline overlap.  
**P13-AC-02** — v1→v2 migration is deterministic and preserves all supported v1 Fade semantics.  
**P13-AC-03** — Migration never invents CrossDissolve and handles malformed unsupported legacy CrossDissolve explicitly.  
**P13-AC-04** — Save/reopen preserves exact transition identity, participant IDs, duration, and canonical state.  
**P13-AC-05** — No backend-specific transition state is persisted.

### Time / Handles / Commands

**P13-AC-06** — Duration uses `RationalTime`; L/R odd-duration semantics and `[T0,T1)` boundaries are deterministic.  
**P13-AC-07** — Only immediate adjacent video/video pairs on the same track may own a CrossDissolve.  
**P13-AC-08** — Known insufficient handles return `INSUFFICIENT_TRANSITION_HANDLE`.  
**P13-AC-09** — Unprovable handle availability returns `TRANSITION_HANDLE_UNVERIFIABLE`.  
**P13-AC-10** — Unsupported participant media returns `UNSUPPORTED_TRANSITION_MEDIA`.  
**P13-AC-11** — No create/update/edit path silently clamps, shortens, freezes, or approximates missing handles.  
**P13-AC-12** — Move/trim/delete/duplicate/split obey the approved dependent-mutation rules atomically.  
**P13-AC-13** — Undo/redo restores exact transition identity and canonical state.

### Composition / Preview

**P13-AC-14** — A single normalized two-input transition semantic drives both preview and export.  
**P13-AC-15** — Canonical blend is premultiplied RGBA `(1-p)A + pB` after participant visual evaluation.  
**P13-AC-16** — Preview represents CrossDissolve as an isolated `PreviewTransitionGroup` using `plus-lighter`; ordinary source-over A/B opacity is not used as fallback.  
**P13-AC-17** — Required WebView2 preview capability is verified on the supported runtime.

### Export / Parity

**P13-AC-18** — Export uses normalized RGBA → premultiply → bounded aligned `xfade=fade` → unpremultiply → existing overlay chain.  
**P13-AC-19** — Ordinary participant contributions are suppressed inside `[T0,T1)` and total output duration remains unchanged.  
**P13-AC-20** — CrossDissolve does not alter audio; audio remains an existing butt-cut.  
**P13-AC-21** — Preview/export semantic, timing/geometry, and raster parity meet existing Gate E tolerances across the approved fixtures.  
**P13-AC-22** — A negative-control blend/progress mutation causes the parity gate to fail.

### UI / Automation / Regression

**P13-AC-23** — Minimal UI can create, adjust duration, and remove an eligible CrossDissolve through the Command Service.  
**P13-AC-24** — Automation capability discovery, dry-run, apply, save/reopen, idempotency, and stale-revision behavior work with Schema v2 and CrossDissolve.  
**P13-AC-25** — Existing supported Fade transitions, core editing, persistence/recovery, export safety, automation, packaging binary resolution, and unrelated features have no Phase 13 regression.  
**P13-AC-26** — Full required test/typecheck/lint/build suite is green.

---

## 7. Required Validation Evidence

At minimum, the Codex completion report must include evidence for:

### Rust / Domain
- targeted schema v2 and migration tests;
- cut-transition validation tests;
- handle/error tests;
- move/trim/split/delete/duplicate lifecycle tests;
- undo/redo tests;
- composition semantic tests;
- export integration tests;
- automation tests;
- full `cargo test`;
- `cargo clippy -- -D warnings`;
- `cargo fmt -- --check`.

### Frontend
- targeted PreviewTransitionGroup tests;
- minimal UI tests;
- existing Vitest suite;
- `pnpm run typecheck`;
- production frontend build.

### Media / Parity
- real pinned/bundled FFmpeg execution for CrossDissolve fixture export;
- duration/metadata validation with ffprobe;
- transition parity fixture results;
- negative-control sensitivity;
- explicit manual/runtime verification of `plus-lighter` on the supported WebView2 environment.

### Regression
- FadeIn/FadeOut/FadeBlack behavior unchanged;
- save/reopen and migration behavior;
- CLI/stdio automation scenario;
- export cancellation/failure-safe output behavior;
- Phase 12 `MediaBinaryResolver` packaged/dev boundary not bypassed.

---

## 8. Implementation Stop Conditions

Codex must stop and report rather than silently redesign if any of these occur:

1. `plus-lighter` cannot produce the approved premultiplied transition result on the supported WebView2 runtime.
2. FFmpeg 9.0.2 packaged build cannot implement the approved premultiply → xfade → unpremultiply path with acceptable parity.
3. Existing project model/migration mechanics cannot preserve current Fade semantics during Schema v2 migration without data loss.
4. Correct handle validation requires changing canonical media/source semantics beyond ADR-003.
5. A required dependent edit cannot be made atomic through the current Command Service/history model.
6. Implementing CrossDissolve would require a general compositor or arbitrary effect graph.
7. Preview/export parity cannot meet the existing Gate E tolerance without violating the accepted semantic contract.

Any such finding reopens the relevant architecture/research decision before further implementation.

---

## 9. Deferred Items Recorded by Phase 13

Remain deferred after this phase unless separately approved:

- audio crossfade;
- future transition alignment modes;
- non-linear transition easing;
- Bezier/custom keyframe easing;
- spectral/multi-channel waveform visualization;
- additional transition kinds;
- generalized compositor/shader framework;
- MCP adapter;
- hardware acceleration qualification;
- true clean Windows VM qualification;
- HDR / advanced color management;
- H.265 / AV1;
- cross-platform release parity.

---

## 10. Completion Gate

Phase 13 may be proposed as complete only when:

1. P13-AC-01 through P13-AC-26 are individually accounted for with evidence;
2. schema v2 migration and save/reopen are proven;
3. no current Fade behavior regresses;
4. dependent edit lifecycle is proven;
5. preview and export implement the same canonical premultiplied transition semantic;
6. parity fixtures and negative control pass;
7. automation/UI use the same Command Service path;
8. full regression suite is green;
9. no unclassified blocker remains;
10. deferred items remain explicitly out of scope.

Codex must **not** mark Phase 13 complete solely in project documentation. It must return a completion report with evidence for ChatGPT review.

After Codex report:
`ChatGPT review → completion decision → durable project-state reconciliation → next-roadmap decision`.

---

## 11. Codex Completion Report Format

Report:

1. files changed;
2. implementation by Slice A–G;
3. P13-AC-01..P13-AC-26 evidence matrix;
4. schema v1→v2 migration evidence;
5. command/dependent-edit behavior;
6. preview `PreviewTransitionGroup` verification;
7. export FFmpeg filtergraph verification;
8. parity fixtures + negative control;
9. UI and automation evidence;
10. full test/typecheck/lint/build outputs;
11. manual/runtime QA;
12. regressions/known issues;
13. deferred items;
14. architecture deviations, if any;
15. proposed phase classification:
   - `COMPLETE`
   - `COMPLETE WITH DEFERRED ITEMS`
   - `INCOMPLETE`
   - `BLOCKED`

Do not begin any next phase.
