# plan_phase6.md

> Implementation plan for completing **Phase 6 — Real Preview Backend POC**.
>
> This document is an execution plan for the coding agent. It is derived from the current project handoff, the reported implementation status, and the existing architectural constraints.  
> If this file conflicts with `PLAN.md`, `ARCHITECTURE.md`, `AGENTS.md`, or `PROJECT_STATUS.md`, the canonical repository document wins.

---

## 1. Objective

Complete Phase 6 by replacing the current mock-only preview behavior with a **real decoded video preview path** that is sufficient to validate the preview architecture.

The goal is **not** to build a full production-grade playback engine in this phase.

The goal is to prove, with real media and measurements, that the chosen preview approach can:

1. decode actual video frames;
2. seek to a timeline time and present the correct frame;
3. consume the same normalized composition semantics used by the editor/export path;
4. support basic continuous playback;
5. survive common media/runtime failures without corrupting project state;
6. remain behind the existing preview/media abstractions;
7. produce enough evidence to make a durable preview-backend decision.

Phase 6 is complete only when the Preview Backend POC gate can be closed with evidence.

---

## 2. Why Phase 6 Must Be Completed Before Later Feature Work

The current implementation reportedly has:

- a `PreviewBackend` abstraction in Rust;
- composition/layout logic;
- a React `PreviewCanvas`;
- transforms such as position, scale, rotation, opacity, captions and timecode;
- but only a **mock viewport**, not real decoded video pixels.

That is not sufficient to validate the central Phase 6 question:

> Can the preview approach provide acceptable responsiveness for target timeline/media workloads?

Do **not** start Whisper/STT, MCP transport, waveform UI, keyframe editing UI, advanced export profiles, cross-platform work, or deferred editor features until this gate has been addressed.

---

## 3. Canonical Constraints

The coding agent must preserve the existing project invariants.

### 3.1 Project state

- The Project Domain Model remains the canonical source of truth.
- Do not introduce a second timeline/project model for preview.
- Renderer-specific runtime state must not be persisted into the canonical project model.

### 3.2 Command semantics

- All authoritative project mutations continue through the Command Service.
- Preview playback state may be transient.
- Do not create a preview-specific mutation/reducer path.

### 3.3 Time model

- Use the existing canonical integer/rational timeline representation.
- Do not introduce arbitrary floating-point seconds as authoritative project time.

Floating-point values may be used internally by a media/runtime API only at the boundary, after explicit conversion from canonical time.

### 3.4 FFmpeg / media boundary

- FFmpeg/ffprobe must remain behind the controlled media abstraction.
- Frontend code must not construct arbitrary shell commands.
- Do not allow React components to invoke unrestricted FFmpeg commands.

### 3.5 Preview/export relationship

- Preview and export may use different backends.
- They must consume the same normalized composition semantics.
- Do not duplicate transform/crop/opacity/timing semantics in ad hoc frontend code if they already exist in the normalized composition layer.

### 3.6 Background work

- Expensive preview preparation must use bounded work.
- Jobs/requests that are obsolete must be cancellable or supersedable where appropriate.
- Derived preview data/cache is disposable and never canonical state.

---

## 4. Explicit Non-Goals

The following are **out of scope** for this Phase 6 completion unless one of them is strictly necessary to pass the preview POC.

Do not implement:

- Whisper / speech-to-text;
- MCP server transport;
- waveform timeline UI;
- keyframe editor UI;
- Bezier graph editor;
- masks;
- tracking;
- advanced color grading;
- 4K60 as a launch requirement;
- H.265 / AV1 / ProRes / VP9 export expansion;
- macOS/Linux ports;
- installer/signing work;
- cloud rendering;
- distributed workers;
- Redis or external job infrastructure;
- plugin SDK/marketplace;
- a separate preview project format;
- a second mutation path;
- arbitrary architectural rewrites unrelated to preview.

If the existing architecture is proven insufficient, stop at the decision point and document the evidence instead of creating a parallel implementation.

---

## 5. Phase Entry Checklist

Before coding, inspect the current repository and record the actual state.

Read in this order:

1. `AGENTS.md`
2. `PLAN.md` — Phase 6 and all preview/parity acceptance criteria
3. relevant `ARCHITECTURE.md` preview/media/composition sections
4. `DEVELOPMENT_WORKFLOW.md`
5. `PROJECT_STATUS.md`
6. relevant UI rules in `UX_UI.md`
7. relevant POC evidence in `RESEARCH.md` if needed

Then inspect at minimum:

- `src/components/PreviewCanvas.tsx`
- Rust preview module(s), including the `PreviewBackend` trait
- normalized composition structures
- media probing/path resolution layer
- Tauri commands/events used by preview
- job manager implementation
- project time conversion utilities
- missing-media/relink handling
- export composition builder
- current tests around preview/composition/media

Do not assume the handoff report is perfectly current. The repository state is authoritative.

---

## 6. Required Phase Deliverables

Phase 6 must produce all of the following:

1. Real decoded frame presentation in the preview.
2. Timeline seek -> correct video frame.
3. Basic continuous playback.
4. Composition transforms applied to real decoded media.
5. Safe handling of missing/corrupt/unsupported media.
6. Bounded/supersedable preview work.
7. Preview performance instrumentation.
8. A documented Preview Backend POC result.
9. Tests appropriate to the selected approach.
10. Updated `PROJECT_STATUS.md`.
11. Updated `ARCHITECTURE.md` only if a durable architecture decision has been made.
12. Updated `DEVELOPMENT_WORKFLOW.md` if new runtime/test commands are introduced.

---

# 7. Milestone 0 — Audit the Existing Preview Pipeline

## Goal

Understand exactly what already exists before changing architecture.

## Tasks

Document the current path:

```text
Project State
  -> Composition normalization
  -> PreviewBackend
  -> Tauri/native boundary
  -> PreviewCanvas
```

Identify:

- where active clips at time `T` are resolved;
- where transforms are computed;
- whether preview and export already share normalized semantics;
- how media source paths are resolved;
- how offline/missing media is represented;
- which preview APIs are mock-only;
- which parts already handle real media;
- which runtime state currently lives in React vs Rust;
- whether any existing cache/job system can be reused.

## Output

Add a short implementation note in the Phase 6 worklog or POC document containing:

```text
Current preview input contract:
Current preview output contract:
Current backend implementation(s):
Mock-only components:
Reusable media/composition code:
Known gaps:
```

## Exit Criteria

The agent can point to the exact code path responsible for:

- timeline-time resolution;
- composition generation;
- frame acquisition;
- frame presentation.

Do not continue if the current architecture cannot be clearly traced.

---

# 8. Milestone 1 — Define the Preview POC Contract

## Goal

Make the POC measurable before implementing the backend.

## Required POC Questions

The implementation must answer:

1. Can the backend seek to real frames with acceptable latency?
2. Can it play continuously without pathological frame backlog?
3. Can it present transformed/composited media using normalized composition semantics?
4. Can obsolete frame requests be dropped/superseded?
5. Does memory remain bounded over repeated seek/play/pause cycles?
6. What codec/container limitations exist?
7. What hardware acceleration behavior is available?
8. What fallback is required?
9. What preview/export semantic differences remain?

## Test Media

Create or reuse a **small committed or documented fixture set**.

The fixture set should be minimal but representative.

Prefer fixtures covering:

- one common H.264 MP4;
- at least one non-zero clip in-point;
- different source resolutions/aspect ratios;
- one file that can exercise missing/corrupt handling.

Do not add large media files to Git unless the repository conventions explicitly allow it.

If fixtures are external/generated, document the exact generation procedure.

## Metrics

Instrument at minimum:

- seek request timestamp;
- frame-ready timestamp;
- frame-presented timestamp where measurable;
- seek latency;
- decode/frame acquisition latency;
- dropped/superseded request count;
- playback late-frame count;
- current queue depth;
- peak/steady memory observation for the POC;
- CPU observation for the POC.

Exact production telemetry infrastructure is not required.

---

# 9. Milestone 2 — Select and Prototype Candidate Preview Backends

## Goal

Evaluate at least the viable backend already implied by the codebase and, when necessary, one realistic alternative.

Do not permanently lock the architecture before evidence exists.

## Candidate A — WebView / Browser Media Decode

Possible shape:

```text
Rust / Project Model
  -> normalized preview composition
  -> source URL / safe media bridge
  -> WebView media element or browser decode path
  -> PreviewCanvas/compositor
```

Evaluate:

- codec/container support in target Windows WebView2;
- seek accuracy;
- hardware decode behavior;
- frame access/presentation;
- media source URL/path safety;
- composition integration;
- whether multiple layers are feasible.

## Candidate B — Native / FFmpeg-backed Decode

Possible shape:

```text
Rust media layer
  -> decoder
  -> frame/surface representation
  -> preview presentation bridge
  -> PreviewCanvas/native surface
```

Evaluate:

- decoder control;
- codec consistency;
- hardware decode/fallback;
- seek behavior;
- frame transport cost;
- integration complexity.

## Important Anti-Pattern

Do not implement the long-term path as:

```text
FFmpeg/native decoded RGBA frame
  -> base64
  -> JSON
  -> Tauri IPC
  -> React
```

for continuous full-resolution playback.

It may be acceptable only as a tiny diagnostic experiment, never as the selected architecture without evidence that the cost is acceptable.

Avoid any design whose benchmark mostly measures serialization/copy overhead rather than decoder/presentation capability.

## Decision Rule

Select the smallest backend that satisfies Phase 6 acceptance criteria without creating architectural debt.

Record:

- selected backend;
- rejected/secondary candidate;
- evidence;
- limitations;
- fallback/preparation strategy;
- decision consequences.

---

# 10. Milestone 3 — Real Frame Seek

## Goal

Before continuous playback, implement deterministic single-frame preview at the current playhead.

## Required Data Flow

```text
canonical timeline time T
  -> resolve active clip(s)
  -> map project time to source media time
  -> obtain real decoded frame
  -> apply normalized composition
  -> present frame
```

## Functional Requirements

At minimum:

- load a project with a real video clip;
- move the playhead to a known time;
- display actual video pixels;
- seek repeatedly to different positions;
- handle clip in-point correctly;
- handle timeline offsets correctly;
- handle end-of-clip correctly;
- handle an inactive/gap region;
- avoid committing project state during scrub-only movement unless a real command is required by existing editor semantics.

## Correctness Tests

Add tests for time mapping where practical:

```text
project_time -> clip_local_time -> source_time
```

Cover:

- clip at timeline zero;
- clip with timeline offset;
- trimmed clip;
- seek before clip;
- seek after clip;
- exact boundaries.

## Exit Criteria

Real decoded media is visible in PreviewCanvas/native preview at arbitrary supported playhead positions.

Do not start playback work until frame seek correctness is reliable.

---

# 11. Milestone 4 — Apply Existing Composition Semantics to Real Frames

## Goal

Ensure the real preview is not a separate rendering interpretation.

Apply the existing normalized preview/composition semantics for supported Phase 6 properties.

At minimum, verify whichever of these are already part of the editor model:

- position X/Y;
- scale;
- rotation;
- opacity;
- layer/z-order;
- clip timing/in/out;
- project canvas/aspect behavior;
- caption/text overlay if already included in the normalized composition path.

Do not add new editor features just to enrich the POC.

## Required Rule

When export and preview both support a property, they must derive behavior from the same normalized semantic representation.

Backend-specific implementation code may differ.

Semantic meaning must not.

## Test Approach

Use deterministic fixtures where possible:

```text
same project state
  -> normalized composition
  -> preview interpretation
  -> export interpretation
```

Assert normalized values or resulting geometry before relying only on visual inspection.

---

# 12. Milestone 5 — Basic Continuous Playback

## Goal

Add a minimal playback loop after single-frame seek works.

## Required Controls

At minimum:

- play;
- pause;
- seek while paused;
- seek after pause;
- stop/end-of-range behavior.

Use existing UI controls where they exist.

Do not redesign playback UI unless necessary.

## Runtime State

Playback runtime state may be transient:

```text
playing
current playback clock
pending frame request
last presented frame
dropped/superseded requests
```

Do not persist this into the canonical project model.

## Scheduling Requirements

The implementation must avoid unbounded work.

Example failure to prevent:

```text
request T=1.00
request T=1.03
request T=1.06
request T=1.09
...
decoder is still serving all obsolete requests
```

Prefer latest-useful-frame behavior during scrubbing/playback.

Depending on the chosen backend:

- cancel obsolete requests;
- supersede them;
- coalesce them;
- drop late frames;
- bound queues.

## Exit Criteria

A representative fixture can play continuously without:

- runaway memory;
- ever-growing request queues;
- application lockup;
- authoritative project state mutation on every frame.

---

# 13. Milestone 6 — Preview Jobs, Cache and Cancellation

## Goal

Integrate preview preparation into the existing bounded Job Manager when heavy preparation is required.

Possible jobs:

- proxy preparation;
- preview index generation;
- optimized decode asset;
- frame cache;
- other derived media preparation.

Only add jobs that the selected preview backend actually requires.

## Requirements

- all derived data is disposable;
- canonical project files do not depend on cache paths;
- jobs have stable identity where appropriate;
- duplicate work is avoided;
- obsolete work can be cancelled/superseded where appropriate;
- job failure does not corrupt project state;
- background work status can be surfaced using existing application conventions.

## Do Not

- introduce Redis;
- create a server-side queue;
- create distributed workers;
- persist runtime decoder handles into the project model.

---

# 14. Milestone 7 — Missing Media and Failure Handling

## Goal

Real preview must preserve existing data-safety semantics.

Test:

- media file missing;
- media file moved;
- corrupt media;
- unsupported media;
- decoder failure;
- preparation job failure;
- preview backend initialization failure.

## Required Behavior

- do not crash the editor;
- do not silently replace the asset;
- retain the canonical asset identity;
- expose existing offline/missing state;
- preserve relink flow;
- allow project save/load even when media is unavailable;
- show a stable placeholder/error preview where appropriate.

## Rule

Preview readiness must not be interpreted as proof that export will succeed.

Likewise, export success must not be treated as proof that preview supports the same codec path.

---

# 15. Milestone 8 — Preview/Export Semantic Parity Validation

## Goal

Use the now-real preview to strengthen the preview/export parity evidence.

This is not necessarily a full separate project phase if `PLAN.md` places the parity POC elsewhere; follow the canonical phase ordering.

For the supported Phase 6 property set, compare preview against exported output.

Suggested fixtures:

1. plain clip;
2. trimmed clip;
3. positioned/scaled clip;
4. rotated clip;
5. opacity change;
6. overlapping layers if already supported;
7. caption/text if already supported.

## Validation

Prefer a combination of:

- normalized semantic assertions;
- geometry/timing tests;
- targeted exported-frame comparison where practical;
- manual visual verification only as supplemental evidence.

Do not attempt exhaustive pixel-perfect parity if the architecture does not promise identical rasterization.

Document tolerated differences.

---

# 16. Test Matrix

The coding agent must add/update tests matching actual risk.

## 16.1 Domain/time tests

Validate:

- project-time to source-time mapping;
- clip boundaries;
- trim mapping;
- rational/integer time conversions;
- no regression to canonical time invariants.

## 16.2 Preview backend tests

Where feasible:

- valid frame request;
- invalid time request;
- missing media;
- backend error;
- cancellation/supersession;
- repeated seek;
- backend shutdown/restart.

## 16.3 Job tests

If jobs are added:

- bounded concurrency;
- cancellation;
- supersession;
- failure;
- cleanup;
- repeated identical request behavior.

## 16.4 UI/integration tests

Verify:

- preview presents a real frame;
- play/pause state;
- seeking changes presented frame;
- missing media state;
- no mock-only rectangle path is mistaken for actual preview readiness.

## 16.5 Regression validation

Run repository-standard:

- targeted tests;
- full relevant unit/integration tests;
- typecheck;
- lint;
- build;
- runtime smoke test.

Use the exact commands already documented by the project.

Do not invent replacement commands unless the repository requires a workflow update.

---

# 17. Performance / POC Evaluation

The POC must produce concrete observations.

At minimum document:

```text
Test machine:
Windows version:
CPU:
GPU:
RAM:
WebView2/runtime version if relevant:
FFmpeg build/version if relevant:

Fixture:
codec/container:
resolution:
frame rate:
duration:

Seek latency:
Playback behavior:
Dropped/late frames:
CPU observation:
Memory observation:
Queue/backlog behavior:
Hardware acceleration behavior:
Known limitations:
```

Do not create fake pass thresholds if `PLAN.md` does not define them.

If no explicit threshold exists, report the measured values and make the backend decision based on the actual target workload described by the canonical project documents.

---

# 18. Acceptance Criteria

Phase 6 can be marked complete only if all applicable items below are satisfied.

## Functional

- [ ] Preview displays real decoded video pixels.
- [ ] Seeking to supported timeline positions displays the expected frame.
- [ ] Trim/in-point mapping is correct.
- [ ] Timeline offset mapping is correct.
- [ ] Basic play/pause works.
- [ ] Repeated seek/play/pause does not create unbounded backlog.
- [ ] Supported transforms/composition properties work on real media.
- [ ] Missing media does not crash the application.
- [ ] Corrupt/unsupported media fails safely.

## Architecture

- [ ] Canonical Project Domain Model remains the source of truth.
- [ ] No second authoritative preview timeline model exists.
- [ ] Project mutations still use the Command Service.
- [ ] Preview/runtime state remains transient where appropriate.
- [ ] Canonical integer/rational time rules remain intact.
- [ ] FFmpeg/decoder remains behind controlled abstractions.
- [ ] Frontend does not construct unrestricted shell commands.
- [ ] Derived preview cache is disposable.
- [ ] No unbounded preview worker/request queue exists.
- [ ] Preview consumes normalized composition semantics shared with export.

## POC Evidence

- [ ] Real-media fixture(s) were used.
- [ ] Seek latency was measured.
- [ ] Playback behavior was measured.
- [ ] Queue/backlog behavior was observed.
- [ ] CPU/memory behavior was observed.
- [ ] Codec/runtime limitations were documented.
- [ ] Selected backend and rationale were documented.
- [ ] Rejected/secondary approach and reason were documented if evaluated.
- [ ] Remaining known limitations were documented.

## Validation

- [ ] Relevant tests pass.
- [ ] Typecheck passes.
- [ ] Lint passes.
- [ ] Build passes.
- [ ] Runtime smoke test passes.
- [ ] Existing editor/export functionality has no known regression caused by Phase 6.

## Documentation

- [ ] `PROJECT_STATUS.md` updated.
- [ ] POC result recorded in the appropriate canonical document.
- [ ] `ARCHITECTURE.md` updated if a durable backend decision was made.
- [ ] `DEVELOPMENT_WORKFLOW.md` updated if commands/workflow changed.

---

# 19. Stop / Escalation Conditions

Stop implementation and report a blocker instead of creating an architectural bypass when any of the following occurs:

1. the selected backend cannot meet the required workload;
2. required media transport would require an unsafe/unbounded IPC design;
3. the existing normalized composition model cannot represent required preview semantics;
4. PreviewBackend abstraction is proven fundamentally incompatible with the required design;
5. a required Tauri/WebView/native capability is unavailable on the supported Windows target;
6. hardware/codec licensing/distribution assumptions are disproven;
7. the only apparent solution requires a second project model or second mutation path;
8. the change would create data-loss or persistence risk;
9. the POC requires a product decision that `PLAN.md` intentionally left open.

When blocked, report:

```text
Phase:
Blocker:
Evidence:
Root cause:
Attempted approaches:
Impact:
Available options:
Recommended architecture/plan decision to review:
```

Do not silently expand scope.

---

# 20. Implementation Order

Follow this order unless repository evidence requires a smaller adjustment:

```text
0. Repository audit
1. Define measurable POC contract
2. Evaluate/select preview backend approach
3. Real single-frame seek
4. Apply normalized composition
5. Basic continuous playback
6. Bounded jobs/cache/cancellation if required
7. Failure/missing-media handling
8. Preview/export parity validation
9. Performance measurement
10. Tests + self-review
11. Documentation/status update
```

Do not begin Milestone 5 until Milestone 3 is correct.

Do not declare the phase complete before Milestone 9 produces evidence.

---

# 21. Suggested Coding-Agent Working Procedure

For each milestone:

1. inspect the relevant existing code;
2. state the smallest coherent change;
3. implement only that change;
4. run targeted validation;
5. fix regressions caused by the current work;
6. self-review for architecture violations;
7. record evidence;
8. move to the next milestone without waiting for confirmation unless a real blocker exists.

Do not rewrite unrelated modules for cleanliness during the POC.

Prefer small, reviewable commits/checkpoints.

---

# 22. Required Final Phase Report

When Phase 6 is complete, the coding agent should provide a report containing:

## Summary

- backend selected;
- what now works;
- what remains intentionally unsupported.

## Architecture

- final preview data flow;
- composition relationship;
- runtime state ownership;
- job/cache strategy;
- failure path.

## POC Results

- fixture set;
- seek measurements;
- playback measurements;
- CPU/memory observations;
- queue/backpressure behavior;
- hardware acceleration/fallback notes;
- codec/container limitations.

## Validation

- tests executed;
- typecheck result;
- lint result;
- build result;
- runtime smoke result.

## Files Changed

List the important files and why they changed.

## Remaining Limitations

List only real known limitations.

Do not convert optional future features into Phase 6 blockers.

## Decision

Explicitly state one of:

```text
PHASE 6: PASS
```

or

```text
PHASE 6: BLOCKED
```

If blocked, include the Stop / Escalation report.

---

# 23. Next Work After Phase 6

Only after Phase 6 passes:

1. complete/strengthen the planned Preview/Export Parity gate at its canonical plan position;
2. continue the first incomplete canonical phase;
3. likely product/UI work such as waveform/keyframe UX only when the plan reaches it;
4. treat STT/Whisper as optional unless canonical requirements change;
5. treat MCP as an additional automation transport, not a replacement command engine;
6. perform installer/release work at the planned release phase.

The next phase must still be selected from the canonical `PLAN.md` and current `PROJECT_STATUS.md`, not from feature preference.

---

## Core Principle

> Build the smallest real preview pipeline that proves the architecture, preserves canonical project semantics, and can become the foundation for production preview without introducing a second editor architecture.
