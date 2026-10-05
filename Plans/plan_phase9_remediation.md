# plan_phase9_remediation.md

> Remediation plan for **TinixCut Phase 9 — Bounded Creator Feature Expansion**
>
> Current review classification: **INCOMPLETE**
>
> Purpose: close the specific Phase 9 acceptance-evidence gaps identified by review without expanding product scope or beginning Phase 10.
>
> Canonical precedence:
> `AGENTS.md` → `PLAN.md` → `ARCHITECTURE.md` → `DEVELOPMENT_WORKFLOW.md` → `PROJECT_STATUS.md` → `UX_UI.md`.
>
> If this document conflicts with a canonical repository document, the canonical document wins.

---

## 1. Remediation Objective

Bring Phase 9 from:

```text
INCOMPLETE
```

to one of:

```text
COMPLETE
```

or:

```text
COMPLETE WITH DEFERRED ITEMS
```

by closing only the review-identified acceptance gaps:

1. **Simple transitions (9.3)** need full production-supported lifecycle evidence.
2. **Keyframe Volume** needs parity coverage if Volume remains production-supported.
3. **Keyframe save/reopen** needs explicit round-trip validation.
4. **Waveform extraction** must prove that heavy work uses the bounded/cancellable Job Manager path.
5. **Manual QA evidence** must cover waveform, keyframe and transition UX where automated tests do not exercise UI behavior.
6. `PLAN.md` / `PROJECT_STATUS.md` must remain reconciled with the true incomplete state until remediation passes.

Do not start Phase 10 work during this remediation.

---

## 2. Canonical Acceptance Target

Phase 9 acceptance requires every production-supported feature to have:

```text
project schema semantics
command operations
undo/redo
save/reopen tests
preview behavior
export behavior
parity coverage appropriate to feature risk
```

The remediation is complete only when each claimed Phase 9 production feature has evidence for all applicable items.

---

## 3. Current Review Classification

### Status

```text
Phase 9: INCOMPLETE
```

### Already healthy

The previous implementation report indicates the repository is generally healthy:

- Rust test suite green
- Rust lint clean
- frontend tests green
- production frontend build green
- waveform UX substantially implemented
- keyframe command/history/persistence foundation substantially implemented
- keyframe parity already exists for several visual properties

These green checks must be preserved.

### Remaining blocking gaps

```text
9.3 Simple Transitions lifecycle evidence
Keyframe Volume parity
Explicit keyframe save/reopen round trip
Waveform bounded JobManager execution proof
Manual QA evidence
```

---

# 4. Workstream A — Simple Transitions Completion

## A0. Audit Current Transition Implementation

Inspect:

- transition domain types
- transition schema serialization
- transition command registration
- history/undo implementation
- CompositionPlan transition normalization
- PreviewBackend transition consumption
- FFmpeg export transition generation
- parity tests
- UI controls, if production-supported

Determine exact current production subset.

Expected currently reported types:

```text
FadeIn
FadeOut
CrossFade
```

Do not assume all three are production-supported merely because enum/domain definitions exist.

Produce:

```text
Transition type:
Schema: YES/NO
Commands: YES/NO
Undo/redo: YES/NO
Save/reopen: YES/NO
Preview: YES/NO
Export: YES/NO
Parity: YES/NO
UI: YES/NO
```

---

## A1. Lock the Production Transition Subset

Choose the smallest subset already intended by `PLAN.md` and current implementation.

Do not add new transitions.

Allowed outcomes:

### Outcome 1 — Full current subset remains supported

Then every type claimed as production-supported must meet the entire Phase 9 lifecycle.

### Outcome 2 — Some transition types are prototypes only

Then explicitly remove/defer them from the production-supported Phase 9 scope in `PLAN.md` / project status.

Do not claim a transition as production-supported without full acceptance evidence.

---

## A2. Project Schema Semantics

For each supported transition, define canonical data.

At minimum verify:

- stable transition identity or ownership
- transition type
- duration
- placement/relationship to clips
- canonical time representation
- validation rules

Rules must be independent from FFmpeg syntax and WebView/CSS details.

Test:

```text
serialize
deserialize
semantic equality
```

---

## A3. Command Operations

All authoritative transition edits must use Command Service operations.

Required operations depend on existing design, but conceptually must cover:

```text
add/set transition
update transition duration/type
remove transition
```

Do not mutate transition state directly from React.

Validate:

- target exists
- supported transition type
- legal duration
- legal clip relationship
- timeline boundary constraints
- stale revision handling
- deterministic error result

---

## A4. Undo / Redo

Add explicit tests:

```text
add transition -> undo -> redo
update transition -> undo -> redo
remove transition -> undo -> redo
```

Verify equivalent canonical state after each round trip.

Do not use UI state as evidence for history correctness.

---

## A5. Save / Reopen

Add a real persistence test:

```text
create project
add supported transition
save canonical project
reopen project
compare transition semantics
```

Test stable IDs and canonical times.

If schema migration is involved, test it.

---

## A6. Preview Behavior

For each supported transition:

- verify transition appears at the correct interval
- verify exact start/end boundary semantics
- verify duration
- verify behavior at midpoint
- verify no transition effect outside active interval

Use canonical sample times:

```text
T0 = before transition
T1 = transition start
Tmid = midpoint
T2 = transition end - one frame
T3 = transition end
```

Use half-open timeline interval semantics where applicable.

---

## A7. Export Behavior

Use the production export path.

For each supported transition:

- render the same fixture
- confirm expected activation interval
- confirm duration
- confirm final output does not silently omit or approximate unsupported semantics

Do not create a parity-only export implementation.

---

## A8. Transition Parity

Add deterministic preview/export parity coverage.

Minimum:

```text
FadeIn
FadeOut
CrossFade
```

only if all remain production-supported.

At sampled canonical times compare:

- normalized transition semantic values
- layer activation
- opacity/blend progression or other transition-specific semantic value
- preview vs export output within documented tolerance

If `CrossFade` requires two clips, fixture must verify both layers.

Add at least one negative-control assertion or reuse the existing parity harness sensitivity test.

---

# 5. Workstream B — Keyframe Volume Completion

## B0. Decide Whether Volume Is Production-Supported

The current report claims Volume support.

Either:

### Keep Volume production-supported

Then add complete parity validation.

or:

### Defer/remove Volume from current supported keyframe set

Then explicitly reconcile docs and capability/UI claims.

Do not leave Volume advertised as supported without evidence.

---

## B1. Volume Semantic Evaluation

If Volume remains supported, verify:

```text
keyframe domain value
    ->
audio gain normalized composition
    ->
preview audio behavior
    ->
export audio behavior
```

Test at:

```text
before first
first keyframe
midpoint
second keyframe
after last
```

Use deterministic gain values.

Example:

```text
1.0 -> 0.5
```

Do not use subjective listening as the only validation.

---

## B2. Volume Export / Preview Parity

Use a deterministic audio fixture.

Measure actual gain/amplitude behavior.

Possible assertions:

- normalized evaluated gain exact
- FFmpeg audio gain expression matches evaluated semantics
- preview audio path consumes equivalent evaluated gain
- output RMS/peak ratio falls within documented tolerance where practical

If preview audio does not currently support animated volume, Volume cannot remain production-supported without an explicit approved scope change.

---

## B3. Volume History/Persistence

Confirm Volume keyframes already inherit the common keyframe command/history/persistence path.

If not, add:

```text
add/edit/move/delete volume keyframe
undo/redo
save/reopen
```

tests.

---

# 6. Workstream C — Explicit Keyframe Save/Reopen Validation

## C1. Add Round-Trip Fixture

Create a project fixture containing multiple supported keyframes.

Include at least:

```text
Position X
Scale
Rotation
Opacity
Volume if supported
```

Save through the actual persistence layer.

Reopen through the actual load path.

Assert:

- keyframe count
- property identity
- canonical times
- values
- interpolation type
- stable clip/keyframe IDs where applicable
- equivalent evaluated values at sample times

Do not test only JSON serialization helpers if the canonical project persistence layer exists.

---

## C2. Recovery Compatibility

If Phase 9 changed schema or keyframe persistence behavior, verify recovery/autosave path does not lose keyframes.

This is required only if the relevant persistence layer was touched by Phase 9.

Do not broaden into a full Phase 11 crash suite.

---

# 7. Workstream D — Waveform Job Architecture Remediation

## D0. Trace Actual Execution Path

Inspect the real path from frontend request to extraction.

Document:

```text
Timeline/Frontend
    ->
Tauri command/API
    ->
?
    ->
JobManager
    ->
FFmpeg media service
    ->
waveform cache
```

The key question is:

> Does waveform extraction actually execute as a bounded JobManager job, or does `media_get_waveform` synchronously/directly invoke FFmpeg extraction?

Do not infer from enum definitions alone.

---

## D1. If Already Using JobManager

Add evidence/tests that prove:

- waveform job kind is submitted through JobManager
- per-class concurrency limit exists
- cancellation semantics exist
- dedupe key exists
- failure state is represented
- cleanup/stale-result policy is defined
- UI does not block waiting synchronously for extraction

Add a focused integration test where practical.

Document the path.

No architecture rewrite is needed.

---

## D2. If Waveform Bypasses JobManager

Correct it.

Target:

```text
media_get_waveform
   ->
lookup cache
   ->
if ready: return data
if missing:
   enqueue/dedupe Waveform job
   ->
return pending/job state
```

The JobManager executes:

```text
Waveform Job
   ->
FFmpegService waveform extraction
   ->
validated derived cache write
   ->
completed state
```

The UI can poll/subscribe/use existing job-state mechanisms.

---

## D3. Bounded Work Tests

Add tests for:

```text
repeated same waveform request -> dedupe
multiple waveform jobs -> concurrency bound respected
cancel -> safe cancelled state
failure -> safe failed state
cache hit -> no extraction job
stale result -> does not overwrite newer source identity
```

Exact tests should fit current JobManager API.

---

## D4. Relink / Cache Invalidation

Verify waveform derived data does not incorrectly survive a relink/source identity change.

The project itself must remain valid if waveform cache is deleted.

---

# 8. Workstream E — Manual QA Evidence

Automated tests do not fully prove interactive editor behavior.

Record a short manual QA matrix.

## E1. Waveform QA

Verify in running app:

```text
import media with audio
waveform pending -> ready
timeline remains interactive
trim head
trim tail
move clip
zoom in/out
scroll
reopen project
missing/no-audio media state
```

Record PASS/FAIL and notable limitations.

---

## E2. Keyframe QA

Verify:

```text
select clip
add first keyframe
move playhead
add second keyframe
observe interpolation
drag keyframe marker
edit Inspector value
previous/next navigation if supported
undo
redo
save
reopen
export
```

Verify all supported production keyframe properties.

---

## E3. Transition QA

For every production-supported transition:

```text
create transition
edit duration/type if supported
preview
undo/redo
save/reopen
export
remove transition
```

Record any renderer tolerance difference separately from semantic defects.

---

# 9. PLAN.md Reconciliation During Remediation

Until remediation passes, Phase 9 must remain open.

Add/retain a concise Phase 9 execution-status note equivalent to:

```text
Status: IN PROGRESS — completion review pending remediation
```

Record remaining acceptance gaps.

Do not mark Phase 9 complete yet.

Do not begin Phase 10 planning.

When remediation passes, update the status to:

```text
COMPLETE
```

or:

```text
COMPLETE WITH DEFERRED ITEMS
```

depending only on remaining non-blocking deferred items.

Advanced Bezier/easing and multi-channel/spectral waveform may remain deferred if they are outside current bounded scope.

---

# 10. Architecture Reconciliation

Update `ARCHITECTURE.md` only if remediation establishes a new durable rule.

Potential durable items:

- exact transition subset and semantics
- transition timing/overlap semantics
- keyframe Volume production support status
- waveform JobManager execution contract

Do not add routine test implementation details.

---

# 11. Required Test Matrix

## Transitions

- [ ] schema serialization
- [ ] command add/set
- [ ] command update
- [ ] command remove
- [ ] invalid duration/type validation
- [ ] undo/redo
- [ ] save/reopen
- [ ] preview timing
- [ ] export timing
- [ ] preview/export parity
- [ ] boundary samples

## Keyframes

- [ ] supported visual property parity remains green
- [ ] Volume parity if supported
- [ ] save/reopen round trip
- [ ] undo/redo remains green
- [ ] animation parity remains green

## Waveform

- [ ] JobManager execution path
- [ ] concurrency bounded
- [ ] dedupe
- [ ] cancellation
- [ ] failure
- [ ] cache hit
- [ ] invalidation/relink where applicable
- [ ] UI pending/ready behavior

## Regression

Run project-standard:

```text
cargo test
cargo clippy -- -D warnings
pnpm test
pnpm run build
pnpm run test:all
```

Use actual commands from `DEVELOPMENT_WORKFLOW.md`.

Do not weaken existing tests or parity tolerances.

---

# 12. Remediation Order

Execute in this order:

```text
R0  Reconcile exact current supported capability claims
R1  Audit Simple Transition lifecycle
R2  Implement missing transition commands/history/persistence
R3  Implement transition preview/export behavior if missing
R4  Add transition parity fixtures/tests
R5  Decide Volume support status
R6  Add Volume parity or explicitly defer/remove support
R7  Add explicit keyframe save/reopen round-trip test
R8  Trace waveform execution path
R9  Route waveform through JobManager if currently bypassed
R10 Add bounded/dedupe/cancel/failure waveform tests
R11 Execute manual QA matrix
R12 Run full Phase 7/8/9 regression
R13 Update PROJECT_STATUS.md / PLAN.md / ARCHITECTURE.md as required
R14 Produce remediation completion report
```

Do not begin unrelated refactors.

---

# 13. Remediation Acceptance Criteria

Phase 9 may be re-submitted for completion review only when:

## Transitions

- [ ] production transition subset explicitly defined
- [ ] schema semantics exist
- [ ] command operations exist
- [ ] undo/redo validated
- [ ] save/reopen validated
- [ ] preview behavior validated
- [ ] export behavior validated
- [ ] parity coverage exists
- [ ] unsupported transition types are explicitly deferred rather than falsely claimed

## Keyframes

- [ ] every production-supported property has parity coverage appropriate to risk
- [ ] Volume parity passes, or Volume is explicitly removed/deferred from supported set
- [ ] save/reopen round-trip test passes
- [ ] undo/redo remains green
- [ ] static Phase 7 parity remains green

## Waveform

- [ ] extraction path demonstrably uses bounded JobManager semantics
- [ ] concurrency/dedupe/cancellation/failure policy exists
- [ ] no synchronous heavy extraction blocks normal frontend command handling
- [ ] cache remains disposable
- [ ] relink/cache identity behavior safe

## QA

- [ ] waveform manual QA recorded
- [ ] keyframe manual QA recorded
- [ ] transition manual QA recorded

## Repository health

- [ ] Rust tests pass
- [ ] clippy/lint passes
- [ ] frontend tests pass
- [ ] production build passes
- [ ] full integration suite passes
- [ ] Phase 7 parity regressions pass

## Documentation

- [ ] `PROJECT_STATUS.md` reflects actual status
- [ ] `PLAN.md` is reconciled
- [ ] `ARCHITECTURE.md` updated if durable semantics changed
- [ ] deferred items explicitly recorded

---

# 14. Stop / Escalation Conditions

Stop and report instead of expanding scope if:

1. transition parity requires a fundamentally different compositor model;
2. CrossFade cannot be represented by the current normalized composition semantics;
3. Volume preview/export paths cannot share one semantic value;
4. waveform JobManager integration exposes a larger scheduler architecture defect;
5. transition schema changes require unsafe migration;
6. fixes would require a second mutation path;
7. satisfying transition acceptance requires adding new product capabilities not currently approved.

Report:

```text
Phase: 9 remediation
Workstream:
Blocker:
Evidence:
Root cause:
Acceptance criterion affected:
Attempted fix:
Impact:
Available options:
```

---

# 15. Required Codex Remediation Report

The final report must begin with:

```text
PHASE 9 REMEDIATION: PASS
```

or:

```text
PHASE 9 REMEDIATION: BLOCKED
```

Do not state `PHASE 9: COMPLETE` as the sole authority; ChatGPT review still performs the completion decision.

Include:

## 15.1 Transition lifecycle matrix

```text
Feature       Schema Cmd Undo Save Preview Export Parity
FadeIn        PASS   PASS PASS PASS PASS    PASS   PASS
FadeOut       ...
CrossFade     ...
```

Only include types still claimed as production-supported.

## 15.2 Keyframe matrix

```text
Property     Command History Save/Reopen Preview Export Parity
Position X   PASS    PASS    PASS        PASS    PASS   PASS
Scale        ...
Rotation     ...
Opacity      ...
Volume       ...
```

## 15.3 Waveform job evidence

Document exact path:

```text
frontend
-> command
-> JobManager
-> FFmpegService
-> derived cache
-> UI
```

Include:

- concurrency policy
- dedupe
- cancellation
- failure
- cache behavior

## 15.4 Manual QA

Record:

```text
Waveform: PASS/FAIL
Keyframes: PASS/FAIL
Transitions: PASS/FAIL
```

with tested actions.

## 15.5 Validation results

List actual command results.

## 15.6 Deviations / deferred work

Explicitly record legitimate deferred items.

---

# 16. Phase Completion Decision After Remediation

After Codex submits the remediation report, ChatGPT must review again against `PLAN.md`.

Possible final outcomes:

```text
COMPLETE
```

All material Phase 9 acceptance criteria are satisfied.

```text
COMPLETE WITH DEFERRED ITEMS
```

All required Phase 9 acceptance criteria are satisfied and only explicitly non-blocking future work remains.

```text
INCOMPLETE
```

One or more production-supported features still lack required lifecycle/validation evidence.

```text
BLOCKED
```

A real external/architectural dependency prevents completion.

Only after acceptance and project-state reconciliation may Phase 10 be selected/planned.

---

# 17. Definition of Done

This remediation is complete when every feature TinixCut claims as production-supported within Phase 9 can demonstrate the full lifecycle required by the canonical plan:

> canonical schema → command mutation → undo/redo → save/reopen → preview → export → parity/validation.

For derived waveform work, the equivalent lifecycle is:

> bounded JobManager execution → cancellation/dedupe/failure → disposable cache → responsive UI.

Anything less remains Phase 9 `INCOMPLETE`.
