# plan_phase9.md

> Execution plan for **TinixCut Phase 9 — Editing UX Completion**.
>
> Known incomplete areas from the current handoff:
> - Audio waveform UX
> - Basic keyframe editing UI / interpolation integration
> - Caption/transcription integration boundary only where required by canonical `PLAN.md`
>
> Canonical precedence:
> `AGENTS.md` → `PLAN.md` → `ARCHITECTURE.md` → `DEVELOPMENT_WORKFLOW.md` → `PROJECT_STATUS.md` → `UX_UI.md`.
>
> If this file conflicts with a canonical repository document, the canonical document wins.

---

## 1. Objective

Complete the first incomplete work in canonical Phase 9 without reopening completed Phase 6/7 architecture.

Primary goal:

```text
Existing domain/jobs/composition primitives
            |
            +------------------+
            |                  |
            v                  v
       Waveform UX        Keyframe UX
            |                  |
            +--------+---------+
                     |
                     v
              Command Service
                     |
                     v
            Canonical Project Model
```

Phase 9 should expose existing editing capabilities through safe, deterministic UI.

---

## 2. Mandatory Kickoff Reconciliation

Before coding, read:

1. `AGENTS.md`
2. the entire Phase 9 section in `PLAN.md`
3. relevant `ARCHITECTURE.md`
4. `DEVELOPMENT_WORKFLOW.md`
5. `PROJECT_STATUS.md`
6. `UX_UI.md`
7. `RESEARCH.md` only if evidence/limitations are relevant

Then record:

```text
Phase 9 canonical items:
9.1:
9.2:
9.3:
9.4:
9.5:

Already complete:
Partially complete:
Not started:
Optional/deferred:
```

Do not assume the earlier handoff is perfectly current.

If `PLAN.md` contains required Phase 9 work not described here, include it.
If this plan includes something the canonical plan marks optional, do not promote it to a blocker.

---

## 3. Known Current State

### Waveform
- Job infrastructure already has waveform-related foundations.
- Timeline currently lacks actual waveform drawing on clips.

### Keyframes
- Domain/core already has basic linear keyframe interpolation foundations.
- Inspector/timeline do not yet provide complete keyframe authoring UX.

### Captions / STT
- Manual captions already exist.
- Caption export exists.
- Earlier plan wording indicates transcription may be conditional.
- Do not add Whisper/cloud STT unless the canonical Phase 9 explicitly requires a provider implementation.

---

## 4. Non-Goals

Unless the current `PLAN.md` explicitly requires otherwise, do not implement:

- Whisper/STT provider integration
- MCP server
- Bezier curve editor
- advanced easing editor
- masks/tracking
- advanced color grading
- new codecs
- HDR
- 4K60 requirement
- installer/signing
- macOS/Linux
- cloud rendering
- plugin SDK
- a second project/timeline model
- a second mutation path

Do not redesign Phase 6/7 preview/export architecture.

---

## 5. Architectural Invariants

### Command Service
All authoritative edits go through the existing Command Service.

Keyframe operations must not mutate project state directly from React.

High-frequency pointer drag state may be transient and committed once on pointer-up.

### Canonical time
Persist keyframe timing with the existing integer/rational time model.

Do not persist floating-point seconds as authoritative time.

### Waveform cache
Waveform data is derived/disposable:

```text
media asset
  -> bounded waveform job
  -> derived cache
  -> timeline visualization
```

Deleting cache must not damage the project.

### Preview/export parity
Keyframe evaluation must feed the same normalized composition semantics verified by Phase 7.

Do not implement UI-only interpolation that export cannot reproduce.

---

# Workstream A — Audio Waveform UX

## A0. Audit existing implementation

Inspect:

- JobManager waveform job kind
- FFmpeg/media abstraction
- cache conventions
- asset identity
- timeline clip components
- missing/relink logic

Answer:

```text
Does waveform extraction already execute?
Output format?
Cache path/identity?
Cancellation?
Duplicate-job handling?
Relink invalidation?
```

Do not create a second waveform subsystem.

---

## A1. Waveform data contract

Prefer a compact peak representation such as:

```text
WaveformData {
  asset_id,
  source_duration,
  bucket_rate,
  channels_or_mixdown,
  peaks: [min,max,...]
}
```

Exact schema should follow current repository conventions.

Requirements:

- deterministic
- compact
- independent of current UI pixel width
- disposable/regenerable
- versionable if cache schema changes

Do not ship raw PCM to React for timeline drawing.

---

## A2. Waveform extraction

Use the existing controlled FFmpeg/media abstraction.

Required behavior:

- bounded background job
- cancellable where appropriate
- duplicate work avoided
- safe failure on missing/corrupt media
- no project corruption
- no unrestricted shell commands from frontend

Follow canonical policy for channel mixing/normalization if specified.

---

## A3. Cache identity

Reuse the existing cache identity/fingerprint mechanism.

Waveform cache must invalidate when source meaning changes.

Potential identity inputs may include repository-standard:

- stable asset ID
- source fingerprint
- source modification metadata
- cache schema version

Do not create an incompatible parallel cache convention.

---

## A4. Backend/UI API

Expose only timeline-ready state/data.

Conceptual response:

```text
ready(data)
pending(job_state)
unavailable(reason)
```

Avoid retransmitting large waveform arrays on every render.

---

## A5. Timeline rendering

Draw waveform inside the relevant clip body.

Requirements:

- correct timeline duration mapping
- does not block clip drag/trim/select
- clear selected/missing states
- pending waveform has a stable fallback
- timeline remains readable

Waveform is visual information only in this phase.

---

## A6. Trim-aware mapping

Critical rule:

```text
source waveform
  -> source in/out range
  -> visible clip timeline range
```

A trimmed clip must show only the correct source region.

Test:

- untrimmed
- head trim
- tail trim
- both trims
- moved clip

Moving a clip must not change its source waveform shape.

---

## A7. Zoom behavior

Timeline zoom must not trigger new waveform extraction.

Preferred behavior:

```text
derived waveform data
    -> UI resampling/aggregation
    -> current zoom rendering
```

Do not build a DAW-scale waveform engine unless canonical scope requires it.

---

## A8. Performance guardrails

Test:

- many visible clips
- scroll
- zoom in/out
- cached project reload
- background waveform extraction

Avoid:

- full PCM in React state
- thousands of unnecessary DOM nodes
- extraction per zoom
- blocking UI on waveform jobs

Use Canvas/SVG/compact DOM according to current project patterns and measured behavior.

---

## A9. Failure states

Handle:

```text
missing media
no audio stream
corrupt audio
job failed
cancelled job
cache missing
```

Timeline editing must continue working.

---

# Workstream B — Basic Keyframe Editing

## B0. Audit keyframe domain

Inspect actual structures/functions for:

- keyframe representation
- animated-property representation
- interpolation
- serialization
- preview/export evaluation

Document:

```text
supported properties
time domain
value types
before-first behavior
between behavior
after-last behavior
duplicate-time policy
```

Do not add a second keyframe model.

---

## B1. Lock supported properties

Only expose properties with stable existing semantics.

Likely examples may include:

- position X/Y
- scale
- rotation
- opacity

But verify against the current model and Phase 9 plan.

Do not automatically animate crop/captions/effects unless required.

---

## B2. Command semantics

All canonical operations must be commands.

Required capabilities may map to existing/generic commands:

```text
AddKeyframe
UpdateKeyframe
MoveKeyframe
DeleteKeyframe
```

Commands must preserve:

- validation
- revision checks
- deterministic ordering
- history
- undo/redo

Reject or canonically resolve:

- unsupported property
- invalid time
- invalid value
- duplicate time
- missing clip

React must not own conflict semantics.

---

## B3. Undo / redo

Test complete round trips:

```text
add -> undo -> redo
move -> undo -> redo
value change -> undo -> redo
delete -> undo -> redo
```

Verify save/load after operations.

---

## B4. Inspector controls

Add minimal keyframe controls beside supported properties.

Expected concepts:

```text
previous keyframe
add/remove current keyframe
next keyframe
value editor
```

At current playhead:

- indicate whether a keyframe exists
- add when absent
- update keyed value when editing at a keyframe
- remove with canonical command

Do not add curve graph UI.

---

## B5. Timeline markers

Display keyframe markers on selected/expanded clip or repository-approved lane.

Requirements:

- canonical time → pixel mapping
- selected/hover state
- remains usable at dense spacing
- clip move semantics correct
- timeline zoom semantics correct

---

## B6. Drag behavior

Use transient UI state during drag.

Recommended flow:

```text
pointer down
  -> capture original canonical state

pointer move
  -> transient marker position

pointer up
  -> one authoritative command
```

Do not commit project state on every mousemove.

Reuse existing snapping rules if applicable.

---

## B7. Keyframe time domain

Verify whether keyframes are stored as:

- clip-local time
- project time
- another existing canonical domain

Do not mix source-media time with project/keyframe time implicitly.

Add regression test:

> moving a clip on the timeline preserves its animation relative to the clip unless canonical semantics explicitly say otherwise.

---

## B8. Interpolation evaluation

Reuse the existing core interpolation.

Authoritative flow:

```text
canonical project time
  -> keyframe evaluation
  -> normalized composition
  -> preview
  -> export
```

Do not duplicate authoritative interpolation in React.

---

## B9. Boundary behavior

Test:

```text
before first keyframe
exactly first
midpoint
exactly next
after last
```

Use existing hold/default semantics.

Do not invent extrapolation if the domain already defines behavior.

---

## B10. Duplicate-time policy

Use exactly one deterministic repository-defined policy:

- reject
- replace
- update existing

Add tests.

---

# Workstream C — Animated Preview / Export Parity

Phase 7 validated static composition.

Phase 9 must extend the invariant to supported animated properties.

For each supported property test:

```text
T0 before first
T1 first
Tmid midpoint
T2 second
T3 after last
```

Verify:

1. domain-evaluated value
2. normalized composition value
3. preview semantics
4. export semantics

Do not weaken existing Phase 7 tolerance rules.

Semantic equality is mandatory.

---

# Workstream D — Conditional Caption / Transcription Boundary

Read exact `PLAN.md` 9.5 wording first.

## If transcription is conditional

Do not implement Whisper/provider merely to check a box.

Only implement/verify the required boundary, conceptually:

```text
Transcription request
   -> bounded Job Manager
   -> provider adapter
   -> transcript result
   -> Command Service
   -> captions in canonical Project
```

Rules:

- provider output cannot mutate Project directly
- caption creation/update goes through commands
- job remains bounded/cancellable
- provider/model choice is not arbitrarily hard-coded

Do not create speculative dead abstractions if even the boundary is not required.

## If real STT is explicitly mandatory

Stop before choosing a provider.

Report that provider/model choice needs canonical confirmation/research.

The known project handoff treats AI/provider choice as deferred.

---

# UI / UX Rules

Follow `UX_UI.md`.

Preserve:

- compact Windows desktop density
- timeline as primary workspace
- keyboard/mouse efficiency
- readable clip labels
- trim handles
- selection states
- missing-media state

Waveform should visually sit behind clip content.

Keyframe markers should have clear:

```text
normal
hover
selected
active-at-playhead
```

states.

Do not introduce a disconnected visual language.

---

# Test Matrix — Waveform

## Extraction
- [ ] video with audio
- [ ] no-audio video
- [ ] audio-only media if supported
- [ ] missing media
- [ ] corrupt media
- [ ] cancelled job

## Cache
- [ ] first generation
- [ ] cache hit
- [ ] invalidation
- [ ] relink
- [ ] project reload

## Timeline
- [ ] untrimmed clip
- [ ] head trim
- [ ] tail trim
- [ ] both trims
- [ ] moved clip
- [ ] zoom in/out
- [ ] horizontal scroll
- [ ] many visible clips

## Performance
- [ ] no unbounded jobs
- [ ] zoom does not re-extract
- [ ] waveform does not block timeline interaction

---

# Test Matrix — Keyframes

## Commands
- [ ] add
- [ ] update
- [ ] move
- [ ] delete
- [ ] invalid property
- [ ] invalid time
- [ ] duplicate time

## History
- [ ] add undo/redo
- [ ] move undo/redo
- [ ] value undo/redo
- [ ] delete undo/redo

## Evaluation
- [ ] before first
- [ ] first
- [ ] midpoint
- [ ] next
- [ ] after last

## UI
- [ ] inspector add/remove
- [ ] previous/next navigation if in scope
- [ ] timeline marker positioning
- [ ] drag
- [ ] zoom
- [ ] clip move

## Persistence
- [ ] save/load
- [ ] schema migration if changed
- [ ] recovery retains keyframes

---

# Phase 7 Regression Protection

Rerun existing parity coverage for:

- source time
- position
- scale
- rotation
- crop
- opacity
- z-order
- captions
- half-open boundaries

Then add animated-property parity cases.

Do not change existing parity thresholds merely because keyframes were added.

---

# Schema / Migration

If project schema changes:

1. follow existing schema-version policy
2. add migration
3. preserve older project files
4. test load/save round trip
5. test recovery behavior where applicable

Avoid schema churn if keyframe structures already serialize.

---

# Performance Guardrails

## Waveform
Do not:
- send raw PCM to frontend
- extract per zoom
- spawn one job per render
- grow unbounded cache/work queues

## Keyframes
Do not:
- save to disk per pointermove
- issue authoritative command per pixel dragged
- recalculate unrelated project state without need
- render excessive DOM markers unnecessarily

Measure before introducing complex optimization.

---

# Milestone Sequence

```text
M0  Reconcile canonical Phase 9 scope
M1  Audit waveform/keyframe foundations
M2  Complete waveform backend/data contract
M3  Complete waveform cache/job lifecycle
M4  Implement timeline waveform UX
M5  Validate trim/zoom/performance
M6  Lock supported keyframe properties
M7  Complete keyframe commands/history
M8  Implement Inspector keyframe controls
M9  Implement timeline keyframe markers/navigation
M10 Implement drag/commit behavior
M11 Extend preview/export parity to animation
M12 Resolve conditional 9.5 transcription boundary
M13 Validate persistence/migration
M14 Full regression + runtime UX smoke
M15 Update docs/status
```

---

# Gates

## Gate 9A — Scope reconciled
Exact 9.1–9.5 scope and current status are known.

## Gate 9B — Waveform backend valid
Extraction, cache, bounded jobs and failure states are correct.

## Gate 9C — Waveform UX valid
Trim/zoom/move behavior is correct and responsive.

## Gate 9D — Keyframe command model valid
Edits use commands; validation/history/persistence work.

## Gate 9E — Keyframe UX valid
Supported keyframes can be added, selected, moved, edited and removed.

## Gate 9F — Animation parity valid
Preview/export consume equivalent evaluated animation semantics.

## Gate 9G — 9.5 resolved correctly
Either:
- implemented as required, or
- confirmed optional without forced provider integration.

## Gate 9H — Full regression clean
All required tests/build/runtime checks pass.

---

# Acceptance Criteria — Waveform

- [ ] derived waveform extraction works
- [ ] bounded/cancellable job semantics preserved
- [ ] cache is disposable
- [ ] no canonical project dependency on cache
- [ ] waveform renders in timeline
- [ ] trims map to correct source region
- [ ] moving clip preserves waveform source shape
- [ ] zoom does not re-extract
- [ ] no-audio/missing media safe
- [ ] pending extraction does not block editing
- [ ] tests pass

---

# Acceptance Criteria — Keyframes

- [ ] supported properties explicitly defined
- [ ] canonical time representation used
- [ ] add works
- [ ] update works
- [ ] move works
- [ ] delete works
- [ ] duplicate-time policy deterministic
- [ ] invalid edits rejected
- [ ] Command Service is authoritative
- [ ] undo/redo works
- [ ] save/load works
- [ ] timeline markers work
- [ ] drag uses transient state then coherent commit
- [ ] Inspector controls reflect state
- [ ] preview interpolation correct
- [ ] export interpolation equivalent
- [ ] Phase 7 static parity remains green

---

# Acceptance Criteria — Transcription Boundary

Only if Phase 9 includes this item:

- [ ] exact 9.5 wording was checked
- [ ] optional wording was not turned into mandatory Whisper/STT
- [ ] provider cannot mutate project directly
- [ ] transcript-to-caption changes use commands
- [ ] jobs remain bounded/cancellable
- [ ] provider/model is not arbitrarily selected

---

# Runtime Smoke Test

Perform a real app smoke test:

```text
1. open project with audio/video
2. observe waveform pending -> ready
3. trim clip and inspect waveform region
4. zoom timeline
5. select clip
6. add first keyframe
7. move playhead
8. add second keyframe
9. verify interpolated preview
10. drag a keyframe
11. undo
12. redo
13. save
14. reload
15. export short segment
16. verify animated result
```

A green build alone is not enough.

---

# Validation

Use only repository-standard commands from `DEVELOPMENT_WORKFLOW.md`.

Expected categories may include:

```text
cargo test
cargo clippy -- -D warnings
pnpm test
pnpm run build
pnpm run test:all
```

Verify actual commands in the repo.

Run targeted tests after each milestone, then full regression before completion.

---

# Stop / Escalation Conditions

Stop before creating a parallel architecture if:

1. waveform requires persisted cache paths in canonical project state
2. JobManager cannot safely represent waveform work
3. existing keyframe model cannot represent required Phase 9 semantics
4. preview/export cannot share keyframe evaluation
5. migration would risk project data
6. 9.5 requires an unresolved provider/product decision
7. React would need direct canonical mutation
8. performance would require a major timeline rewrite outside scope

Report:

```text
Phase: 9
Milestone:
Blocker:
Evidence:
Root cause:
Attempted fixes:
Impact:
Architecture assumption affected:
Available options:
```

---

# Documentation Updates

## PROJECT_STATUS.md
Update current snapshot at phase completion/blocker/transition.

## ARCHITECTURE.md
Update only durable decisions such as:
- canonical keyframe time domain
- interpolation semantics
- waveform cache contract
- transcription boundary

## DEVELOPMENT_WORKFLOW.md
Update only if routine commands changed.

## UX_UI.md
Update only if durable interaction rules were established.

---

# Required Final Codex Report

End with exactly one decision:

```text
PHASE 9: PASS
```

or

```text
PHASE 9: BLOCKED
```

Include:

## Scope reconciliation

```text
9.1:
9.2:
9.3:
9.4:
9.5:
```

with status for each.

## Waveform
- extraction/data contract
- cache/job behavior
- UI behavior
- test results

## Keyframes
- supported properties
- commands
- interpolation
- Inspector UX
- timeline UX
- undo/redo
- persistence

## Animation parity

Example:

```text
Property     First   Mid   Second   Export parity
Position X   PASS    PASS  PASS     PASS
Opacity      PASS    PASS  PASS     PASS
```

## Transcription boundary

State one of:

```text
not required
boundary implemented
provider implementation explicitly required
```

Do not report optional STT as an incomplete mandatory feature.

## Validation
List actual commands/results.

## Files changed
Important files and purpose.

## Remaining limitations
Only real limitations, not deferred features.

---

# Definition of Done

Phase 9 is complete when the exact canonical scope has been reconciled and all required incomplete work is finished, especially:

> Waveform UX behaves as derived bounded media visualization, and basic keyframe editing uses the same canonical Command Service, project model, composition evaluation, preview and export semantics as the rest of TinixCut.

Phase 9 must not introduce a second editor architecture.

---

# Core Principle

Waveform and keyframe UI are not isolated frontend features.

They must preserve the project invariant:

> Human editing and future automation use the same canonical project model and the same editing primitives.
