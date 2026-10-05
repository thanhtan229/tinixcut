# plan_phase7.md

> Implementation plan for **Phase 7 — Preview / Export Parity POC (POC 2)**.
>
> Audience: Codex/coding agent working directly in the TinixCut repository.
>
> Canonical precedence: `AGENTS.md` → `PLAN.md` → `ARCHITECTURE.md` → `DEVELOPMENT_WORKFLOW.md` → `PROJECT_STATUS.md`.
>
> If this file conflicts with a canonical repository document, the canonical document wins.

---

## 1. Objective

Complete the Preview / Export Parity gate by proving that the real preview backend completed in Phase 6 and the FFmpeg export backend consume the **same normalized composition semantics** closely enough to avoid semantic drift.

Central POC question:

> Can preview and export consume the same normalized composition semantics closely enough to avoid semantic drift?

Phase 7 is about **correctness validation and targeted fixes**, not feature expansion.

The desired architecture remains:

```text
Canonical Project State
        |
        v
Normalized Composition
        |
        +-----------------------+
        |                       |
        v                       v
Real Preview Backend        FFmpeg Export Backend
(WebView2)                   (production render graph)
        |                       |
        v                       v
Preview Capture            Export Frame
        |                       |
        +-----------+-----------+
                    |
                    v
               Parity Harness
                    |
                    v
          PASS / DIFF / EVIDENCE
```

---

## 2. Phase Entry Assumptions

Before editing, verify the repository actually matches these Phase 6 results:

- real decoded video preview exists;
- canonical timeline → source time mapping exists;
- normalized transforms are used;
- crop, opacity, layer order and captions are supported as reported;
- WebView/proxy-assisted preview backend is selected;
- preview work is bounded/supersedable;
- missing media is handled safely;
- existing composition parity regression tests are green.

Do not reimplement Phase 6 unless Phase 7 evidence identifies a real correctness defect.

---

## 3. Scope

### In scope

- audit preview/export semantic pipelines;
- define the parity contract;
- deterministic fixture projects/media;
- semantic snapshots of normalized composition;
- deterministic preview capture;
- deterministic export-frame capture;
- image normalization;
- numerical visual diff;
- tolerance policy;
- failure artifacts;
- targeted parity bug fixes;
- regression tests;
- POC 2 evidence/report;
- required documentation updates.

### Out of scope

Do not add:

- Whisper/STT;
- MCP;
- waveform UI;
- keyframe editor UI;
- Bezier editor;
- masks/tracking;
- advanced color grading;
- new codecs/presets;
- 4K60 launch requirement;
- HDR pipeline;
- macOS/Linux;
- installer/signing;
- cloud rendering;
- distributed workers;
- unrelated UI redesign.

Do not broaden this phase because future features could use the work.

---

## 4. Architectural Rules

### 4.1 One canonical source of truth

The Project Domain Model remains authoritative.

Do not create a separate:

```text
PreviewProjectModel
ExportProjectModel
```

Backend-specific structures must be derived/runtime representations only.

### 4.2 One normalized composition semantic layer

Target:

```text
Project
  -> normalized CompositionPlan
       -> Preview adapter
       -> Export adapter
```

Preview and export implementation mechanisms may differ.

Their semantic meaning must not.

### 4.3 Canonical time

Authoritative timeline calculations remain integer/rational.

Floating-point seconds may appear only at external boundaries such as:

- HTML video `currentTime`;
- FFmpeg timestamps;
- rendering APIs.

Conversions must be explicit and testable.

### 4.4 Renderer-specific state

Never persist into the project model:

- WebView handles;
- CSS matrices;
- FFmpeg filter strings;
- decoder handles;
- cache paths;
- parity output paths.

### 4.5 FFmpeg boundary

Frontend code must not construct unrestricted shell commands.

Parity tooling should reuse the production media/export abstraction or repository test tooling.

---

## 5. Required Deliverables

Phase 7 must produce:

1. semantic inventory for preview/export;
2. deterministic parity fixture set;
3. normalized composition snapshot tests;
4. real preview capture at canonical project time;
5. production export capture at the same time;
6. automated comparison/diff harness;
7. explicit tolerance policy;
8. isolated parity tests for supported shared semantics;
9. regression tests for any bugs discovered;
10. POC 2 report;
11. updated `PROJECT_STATUS.md`;
12. architecture/workflow docs updated only where necessary.

---

# Milestone 0 — Audit Existing Pipelines

## Goal

Trace current semantics before changing code.

## Read first

1. `AGENTS.md`
2. Phase 7 section in `PLAN.md`
3. relevant preview/export/composition sections in `ARCHITECTURE.md`
4. `PROJECT_STATUS.md`
5. `DEVELOPMENT_WORKFLOW.md`
6. Phase 6 implementation/report
7. `RESEARCH.md` POC 2 notes if present

Inspect at minimum:

- `src-tauri/src/modules/composition/`
- `src-tauri/src/modules/preview/`
- export/render module(s)
- FFmpeg service/module
- timeline/time utilities
- `src/components/PreviewCanvas.tsx`
- preview TS models
- existing composition parity tests
- export integration tests
- test fixture infrastructure

## Produce a semantic inventory

Document:

```text
Semantic          Normalized source    Preview consumer    Export consumer
---------------------------------------------------------------------------
source time       ...
position          ...
scale             ...
rotation          ...
crop              ...
opacity           ...
z-order           ...
caption            ...
canvas size       ...
```

Classify each property:

```text
A. supported by preview and export
B. preview-only
C. export-only
D. not implemented
```

Only category A is required for strict Phase 7 parity.

## Exit criteria

For every category A property, Codex can identify where it is:

1. created;
2. normalized;
3. consumed by preview;
4. consumed by export.

---

# Milestone 1 — Define the Parity Contract

Parity has three layers.

## Level 1 — Semantic parity

Both paths receive equivalent normalized values.

Examples:

- source time;
- position;
- scale;
- rotation;
- crop;
- opacity;
- z-order;
- canvas dimensions;
- caption inputs.

These should be exact where practical.

## Level 2 — Timing/geometry parity

Both outputs must show the same composition at the same project time.

Examples:

- same active clip;
- same source frame;
- same layer order;
- same project-space geometry;
- same crop intent;
- same opacity intent.

## Level 3 — Raster parity

Pixels should be sufficiently close where both renderers are expected to match.

Do **not** assume exact pixel equality because legitimate differences may come from:

- WebView video scaling;
- FFmpeg scaling;
- browser vs FFmpeg font rasterization;
- anti-aliasing;
- YUV/RGB conversion;
- hardware decoder behavior.

A semantic failure must never be hidden by a loose raster tolerance.

---

# Milestone 2 — Deterministic Fixture Media

## Goal

Generate small media that makes timing and geometry errors obvious.

Preferred fixture video:

```text
fixed frame rate
fixed resolution
visible frame number
colored quadrants
orientation marker
border/grid
```

Example:

```text
+----------------------------+
| RED          | GREEN       |
|              |             |
|     FRAME 0123             |
|              |             |
| BLUE         | YELLOW      |
+----------------------------+
```

This allows easy diagnosis of:

- source frame selection;
- crop;
- rotation;
- position;
- scaling.

Use repository-supported FFmpeg tooling if appropriate.

Do not commit large binary fixtures unless repository conventions require it.

Document fixture generation exactly.

---

# Milestone 3 — Fixture Project Matrix

Use small isolated fixtures rather than one giant project.

## P7-F01 Baseline

```text
one video clip
timeline start 0
no transform
no crop
opacity 1
```

Validate baseline frame, canvas mapping and basic raster delta.

## P7-F02 Timeline offset

Clip begins after time 0.

Validate:

- gap before clip;
- exact activation boundary;
- first active frame.

## P7-F03 Trim / non-zero source in-point

Validate:

```text
project time -> clip-local time -> source time
```

## P7-F04 Position

Test X, Y and combined X/Y offsets.

## P7-F05 Scale

Use non-default scaling.

## P7-F06 Rotation

Use obvious orientation markers and supported angles.

## P7-F07 Crop

Test:

- left;
- right;
- top;
- bottom;
- combined crop.

## P7-F08 Opacity

Overlay a deterministic layer on a deterministic background.

## P7-F09 Layer order

Use two obviously different overlapping clips.

## P7-F10 Caption/text

Only if both current production paths support captions.

Validate timing, content and project-space placement.

## P7-F11 Boundary timestamps

Capture:

```text
one frame before start
start
one frame after start
middle
one frame before end
end
```

## P7-F12 Combined representative composition

After isolated fixtures pass, combine several supported semantics to catch interaction bugs.

---

# Milestone 4 — Normalized Composition Snapshot

## Goal

Detect semantic drift before pixel comparison.

Add a deterministic test/debug projection of the **existing normalized composition**, for example conceptually:

```text
ParitySemanticSnapshot {
  project_time,
  canvas,
  layers: [
    {
      asset_id,
      source_time,
      z_index,
      transform,
      crop,
      opacity,
      caption_data
    }
  ]
}
```

Do not create a second composition model.

Prefer serialization/projection from existing structures.

Exclude irrelevant runtime fields such as:

- handles;
- process IDs;
- DOM IDs;
- cache paths.

Sort collections deterministically.

## Tests

For each fixture/time:

1. create normalized snapshot;
2. assert exact expected semantics;
3. verify both preview/export adapters derive from the same normalized plan.

## Exit criteria

A semantic mismatch can be diagnosed without looking at pixels.

---

# Milestone 5 — Deterministic Preview Capture

## Goal

Capture the real production preview at an exact canonical project time.

Phase 6 uses real WebView2 media. Phase 7 must test that actual path.

Preferred approach:

1. load fixture project;
2. pause playback;
3. set canonical playhead;
4. wait for preview sample;
5. wait for video seek completion / rendered frame;
6. capture preview content region at fixed dimensions.

Avoid arbitrary sleep when an explicit ready event/state exists.

## Determinism controls

Fix:

- preview viewport size;
- project canvas size;
- device scale factor where possible;
- playhead time;
- paused state;
- capture dimensions.

Exclude non-composition UI:

- transport controls;
- HUD;
- benchmark modal;
- selection chrome;
- debug overlays.

Do not implement a separate test renderer that differs from production preview.

---

# Milestone 6 — Deterministic Export Capture

## Goal

Render the same fixture/time through the **production export path**.

Preferred flow:

```text
fixture project
  -> production CompositionPlan
  -> production export graph
  -> FFmpeg render
  -> extract target frame
```

If production export supports single-frame rendering through the same semantic graph, use it.

Otherwise:

1. export a short deterministic segment;
2. extract the target frame through controlled FFmpeg tooling.

Requirements:

- same project revision;
- same project time;
- same normalized composition;
- same project canvas;
- fixed output resolution.

Do not manually rebuild transforms in a special test-only FFmpeg command.

---

# Milestone 7 — Image Normalization

Before numerical comparison, normalize both captures to a common comparison form:

```text
same width/height
RGB or RGBA
8-bit channels
known alpha handling
fixed orientation
```

Do not:

- resize one backend just to make it pass;
- crop away a mismatch;
- translate/rotate images during normalization.

Normalization removes format differences, not semantic differences.

Record color-space/range differences if relevant.

---

# Milestone 8 — Diff Metrics and Tolerances

Use multiple metrics.

Recommended minimum:

- mean absolute channel error;
- percentile or maximum error;
- ratio of pixels exceeding a per-channel threshold.

Optional:

- SSIM or another lightweight perceptual metric if already supported.

Do not add a large image-processing dependency without need.

## Tolerance classes

### Strict geometry fixtures

Generated hard-edged fixtures should have tight tolerance.

### Video fixtures

Allow small decode/scaler/color differences.

### Text fixtures

Allow anti-aliasing/font rasterization differences while still requiring correct:

- content;
- timing;
- placement;
- approximate size/geometry.

## Rule

Do not invent one universal rule such as:

```text
95% match = PASS
```

First collect correct baseline differences, then define documented thresholds.

Thresholds must be:

- explicit;
- version-controlled;
- stable;
- tight enough to detect meaningful drift.

---

# Milestone 9 — Failure Artifacts

On visual parity failure, generate:

```text
preview.png
export.png
diff.png
report.json
```

Optional:

```text
overlay.png
```

Suggested report data:

```json
{
  "fixture": "P7-F06",
  "project_time": "...",
  "semantic_result": "pass",
  "visual_result": "fail",
  "metrics": {},
  "thresholds": {}
}
```

Generated diagnostics belong in test/temp output, not canonical project data.

Do not commit transient diffs.

---

# Milestone 10 — Run Baseline and Classify Failures

For each failure, classify before fixing.

## Class A — Timing/source-frame mismatch

Examples:

- preview frame N vs export N+1;
- trim boundary mismatch.

Fix canonical conversion/rounding rules.

Do not add arbitrary milliseconds.

## Class B — Coordinate system mismatch

Examples:

- center origin vs top-left;
- different project-space interpretation.

Document and unify the semantic convention.

## Class C — Transform-order mismatch

Example:

```text
preview: crop -> scale -> rotate
export: scale -> crop -> rotate
```

Define one semantic order.

Backend implementation primitives may differ, semantic result may not.

## Class D — Crop mismatch

Check:

- units;
- source-space vs canvas-space;
- inset vs rectangle meaning;
- pre/post-transform semantics.

## Class E — Opacity/blending mismatch

Check:

- premultiplied vs straight alpha;
- when opacity is applied;
- blend ordering.

## Class F — Z-order mismatch

Ensure stable deterministic ordering.

## Class G — Text mismatch

Separate:

```text
semantic:
wrong content
wrong timing
wrong position
wrong size input

raster:
anti-aliasing
hinting
glyph renderer
```

Only semantic mismatches are architecture correctness failures.

---

# Milestone 11 — Fix Semantic Drift

For every discovered defect:

1. reproduce with isolated fixture;
2. identify the normalized semantic;
3. fix the shared semantic layer or adapter mapping;
4. add a regression test;
5. rerun fixture;
6. rerun related preview/export tests.

Do not fix parity by applying arbitrary frontend CSS offsets.

Do not loosen tolerance before root-cause analysis.

---

# Milestone 12 — Timing Parity Matrix

At minimum test:

```text
T0 = before clip
T1 = clip start
T2 = clip start + one frame
T3 = middle
T4 = clip end - one frame
T5 = clip end
```

Verify:

- active layer set;
- source time;
- source frame identity;
- preview capture;
- export capture.

Special attention:

- canonical rational → browser seconds;
- canonical rational → FFmpeg time;
- rounding;
- inclusive/exclusive clip boundaries.

Any rounding convention must be explicit.

---

# Milestone 13 — Geometry Parity Matrix

Test separately:

```text
position only
scale only
rotation only
crop only
opacity only
position + scale
scale + rotation
crop + scale
multi-layer overlap
combined representative fixture
```

Do not rely only on combined visual fixtures.

Isolated tests are required for diagnosis.

---

# Milestone 14 — Canvas / Viewport Semantics

Verify:

- project canvas width/height;
- source aspect ratio;
- preview UI fit scaling;
- export output resolution.

Critical distinction:

```text
project-space geometry
!=
preview-panel viewport scaling
```

Resizing the preview panel must not change normalized project geometry.

Add regression coverage where practical.

---

# Milestone 15 — Caption Parity

Only if captions are already supported by both production paths.

Validate:

- active interval;
- content;
- project-space position;
- anchor/alignment semantics;
- opacity;
- font-size semantics if shared.

Do not attempt to make WebView and FFmpeg font rasterization pixel-identical unless canonical architecture requires that.

Document expected raster differences.

---

# Milestone 16 — Missing Media Semantics

Test a missing-media fixture.

Expected behavior can legitimately differ:

```text
preview:
offline/missing placeholder

export:
controlled failure or documented inability to export
```

Parity does not mean identical error UI.

Both paths must preserve:

- asset identity;
- explicit missing state;
- no silent media substitution.

---

# Milestone 17 — Negative-Control Tests

The harness must prove it can detect drift.

Add a controlled test demonstrating failure for at least one deliberate mismatch such as:

- wrong source time;
- wrong position;
- wrong layer order.

Do not ship an intentional production defect.

The test simply proves sensitivity of the parity gate.

---

# Milestone 18 — Automated Parity Suite

Integrate with existing repository testing conventions.

Required layers:

## Fast semantic parity tests

Run in normal Rust/unit/integration workflow where practical.

## Visual parity smoke set

Small representative set.

## Extended parity set

If WebView/FFmpeg capture is expensive, provide a documented explicit command.

Do not mark Phase 7 complete if all real rendering comparison remains manual.

---

# Milestone 19 — CI / Runtime Strategy

Determine whether current CI can support:

- Tauri/WebView2 capture;
- FFmpeg;
- required Windows runtime;
- deterministic video capture.

If full visual parity cannot reliably run in CI:

1. keep semantic tests in CI;
2. run visual smoke tests where supported;
3. provide a deterministic local Windows parity command;
4. document the limitation explicitly.

Do not substitute a fake renderer merely to obtain a green CI badge.

---

# Milestone 20 — Phase 6 Regression Protection

Phase 7 may touch composition and preview code, so rerun Phase 6 checks.

Verify:

- real preview seek;
- play/pause;
- scrubbing;
- missing media;
- proxy-assisted preview where applicable;
- bounded/supersedable work;
- Phase 6 benchmark/smoke.

Do not solve parity by degrading production preview into a slow frame-transfer architecture.

In particular, do not introduce:

```text
raw frame -> base64/JSON IPC -> React
```

for continuous production playback.

---

## 21. Test Matrix

### Domain/time

- project time → clip local time;
- clip local time → source time;
- trim boundaries;
- gap boundaries;
- rational conversion;
- frame identity.

### Composition

- position;
- scale;
- rotation;
- crop;
- opacity;
- z-order;
- canvas dimensions;
- caption semantics if shared.

### Preview capture

- exact requested time;
- repeated seeks;
- ready-state synchronization;
- deterministic viewport;
- missing media.

### Export capture

- production export graph;
- exact requested time;
- deterministic frame extraction;
- missing media failure path.

### Diff harness

- known equal pair passes;
- known mismatch fails;
- failure artifact generation;
- tolerance boundaries.

### Regression

Run repository-standard:

- Rust tests;
- frontend tests;
- typecheck;
- clippy/lint;
- production build;
- runtime smoke;
- parity suite.

Use commands from `DEVELOPMENT_WORKFLOW.md`.

---

## 22. Acceptance Criteria

Phase 7 can pass only when all applicable items are true.

### Semantic architecture

- [ ] Preview/export use the same normalized composition source.
- [ ] No second authoritative renderer model exists.
- [ ] Coordinate conventions are documented.
- [ ] Transform order is documented.
- [ ] Crop meaning is documented.
- [ ] Layer ordering is deterministic.
- [ ] Canonical rational/integer time remains authoritative.

### Timing

- [ ] Clip activation boundaries match.
- [ ] Timeline offset behavior matches.
- [ ] Source in-point behavior matches.
- [ ] Trim-end behavior matches.
- [ ] Source frame identity matches on representative fixture times.
- [ ] Gap behavior is correct.

### Geometry

- [ ] Position semantics match.
- [ ] Scale semantics match.
- [ ] Rotation semantics match.
- [ ] Crop semantics match.
- [ ] Canvas-space mapping matches.
- [ ] Combined-transform fixture passes documented tolerances.

### Compositing

- [ ] Opacity intent matches.
- [ ] Z-order matches.
- [ ] Multi-layer fixture passes.

### Text, where shared

- [ ] Caption timing matches.
- [ ] Caption content matches.
- [ ] Caption placement semantics match.
- [ ] Rasterizer differences are documented separately.

### Harness

- [ ] Deterministic fixture media exists or can be regenerated.
- [ ] Semantic snapshot harness exists.
- [ ] Real preview capture exists.
- [ ] Production export capture exists.
- [ ] Diff metrics exist.
- [ ] Tolerances are explicit.
- [ ] Failure artifacts are produced.
- [ ] Negative-control test proves drift detection.

### Regression

- [ ] All relevant Rust tests pass.
- [ ] Frontend tests pass.
- [ ] Typecheck passes.
- [ ] Clippy/lint passes.
- [ ] Production build passes.
- [ ] Phase 6 preview regression checks pass.
- [ ] No known save/load/command/export regression introduced.

### Documentation

- [ ] POC 2 result recorded.
- [ ] `PROJECT_STATUS.md` updated.
- [ ] `ARCHITECTURE.md` updated if durable semantics changed.
- [ ] `DEVELOPMENT_WORKFLOW.md` updated if parity commands became standard workflow.

---

## 23. Pass Criteria

POC 2 passes when evidence supports:

```text
Preview and export consume the same normalized composition semantics
for the currently shared supported property set.

Remaining raster differences are bounded, documented and attributable
to renderer/decoder/scaler/font behavior rather than semantic drift.
```

These are **not sufficient** by themselves:

- "looks similar";
- a green build;
- only normalized unit tests;
- only manual preview/export inspection.

Real preview and real export paths must be compared.

---

## 24. Fail / Block Criteria

Phase 7 is blocked if any of the following remains unresolved:

1. preview/export require incompatible definitions of the same canonical property;
2. source-time mapping cannot be made consistent;
3. transform order cannot be represented consistently;
4. crop semantics cannot be reconciled without architecture change;
5. layer ordering is nondeterministic;
6. preview cannot be captured reliably at canonical time;
7. production export cannot provide a deterministic comparable frame;
8. visible semantic drift exceeds documented tolerance;
9. the proposed fix requires a second authoritative composition model.

Do not bypass a failed gate.

---

## 25. Stop / Escalation Conditions

Stop before a major redesign if:

- normalized composition lacks required information;
- Phase 6 backend choice is disproven by parity evidence;
- WebView timing cannot reliably map to canonical time;
- FFmpeg semantics force an incompatible canonical model;
- color differences expose an unresolved product-level color-management decision;
- test environment prevents meaningful POC evidence;
- the only fix requires renderer-specific canonical state;
- a destructive project migration would be needed.

Report:

```text
Phase: 7
Gate:
Blocker:
Evidence:
Fixture:
Root cause:
Attempted fixes:
Impact:
Architecture assumption affected:
Available options:
```

---

## 26. Recommended Execution Order

```text
0. Read canonical docs and audit repo
1. Inventory shared preview/export semantics
2. Define parity contract
3. Generate deterministic fixture media
4. Create isolated fixture projects
5. Add normalized semantic snapshots
6. Add real preview capture
7. Add production export capture
8. Add image normalization/diff metrics
9. Establish evidence-based tolerances
10. Run baseline suite
11. Classify mismatches
12. Fix semantic drift one issue at a time
13. Add regression test for every fix
14. Run combined fixture set
15. Run negative-control test
16. Run Phase 6 regressions
17. Run full validation
18. Write POC 2 report
19. Update canonical docs/status
```

Do not start by loosening visual tolerance.

Do not start with pixel diff before semantic snapshot tracing.

---

## 27. Milestone Gates

### Gate 7A — Semantic pipeline understood

Pass when every shared property has one documented normalized meaning and both consumers are known.

### Gate 7B — Fixtures deterministic

Pass when core timing/geometry cases can be reproduced.

### Gate 7C — Both real render paths capturable

Pass when preview and export can be sampled at the same canonical time.

### Gate 7D — Harness detects drift

Pass when a controlled mismatch fails.

### Gate 7E — Shared semantics pass

Pass when all required fixtures meet semantic and visual criteria.

### Gate 7F — Regression clean

Pass when Phase 6 and repository validation remain green.

---

## 28. Required Final Report from Codex

At completion provide:

### Decision

```text
PHASE 7: PASS
```

or

```text
PHASE 7: BLOCKED
```

### Semantic results

Example:

```text
source time: PASS
position: PASS
scale: PASS
rotation: PASS
crop: PASS
opacity: PASS
z-order: PASS
caption timing: PASS
```

Do not report unsupported future features as failures.

### Fixture table

```text
Fixture   Semantic   Visual   Notes
P7-F01    PASS       PASS     baseline
P7-F02    PASS       PASS     timeline offset
...
```

### Tolerance rationale

Document:

- image normalization;
- metrics;
- thresholds;
- expected WebView/FFmpeg differences.

### Bugs fixed

For each:

```text
symptom
root cause
semantic affected
fix
regression test
```

### Validation

Report actual commands and results for:

- Rust tests;
- frontend tests;
- typecheck;
- clippy/lint;
- build;
- parity suite;
- runtime smoke;
- Phase 6 regression.

### Files changed

List important files and purpose.

### Remaining limitations

Acceptable examples if evidenced:

- small text anti-aliasing differences;
- small decoder/scaler color delta.

Do not hide unresolved semantic drift as a "limitation".

---

## 29. Documentation Rules

### PROJECT_STATUS.md

Update only the current snapshot:

- Phase 7 status;
- POC 2 result;
- validation status;
- blocker if any;
- next canonical phase.

Do not turn it into a long changelog.

### ARCHITECTURE.md

Update only durable architectural facts, for example:

- canonical transform order;
- coordinate system;
- crop semantics;
- preview/export parity contract.

### DEVELOPMENT_WORKFLOW.md

Update only if a parity command becomes a supported routine workflow.

---

## 30. Debugging Workflow

When a visual test fails:

```text
1. inspect semantic snapshot
2. if semantic data differs -> fix semantic pipeline
3. if semantics match -> inspect backend adapter mapping
4. if adapters match -> inspect raster/color/scaling behavior
5. classify expected renderer variance vs real defect
6. change tolerance only with documented evidence
```

Never loosen thresholds as the first response.

---

## 31. Definition of Done

Phase 7 is complete when:

> A deterministic project state at canonical time T can be sampled through the real production preview path and the real production export path, compared automatically, and shown to preserve the same currently supported composition semantics with bounded and documented raster differences.

The resulting test infrastructure must be capable of detecting future preview/export semantic drift.

---

## 32. Core Principle

Do not optimize Phase 7 for pixel-perfect screenshots.

Optimize it for **durable semantic correctness**.

The lasting invariant should be:

> Any future change to composition semantics must not silently make preview and export disagree.
