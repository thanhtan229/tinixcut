# Phase 16 — Timeline Editing Productivity

## Status

**APPROVED FOR IMPLEMENTATION — explicitly approved by user**

Governance note:
- ChatGPT authored the plan after Phase-16 research completion.
- User explicitly approved it with `Duyệt Phase 16`.
- Approval occurred before any Phase-16 production implementation.

---

## 1. Objective

Deliver a bounded creator-productivity pass for TinixCut's timeline editor without changing project Schema v3 or preview/export rendering semantics.

Phase 16 focuses on high-frequency timeline interactions that are currently missing or partial:

1. ephemeral Ctrl-based multi-selection with one primary selection;
2. deterministic UI-only snapping;
3. safe atomic multi-clip horizontal movement through a dedicated `ClipGroupMove` command;
4. bounded same-track ripple delete through `ClipRippleDelete`;
5. a centralized Windows-first timeline shortcut layer;
6. split-at-playhead workflow;
7. normal non-ripple trim-to-playhead shortcuts;
8. atomic multi-selection delete;
9. primary-only duplicate shortcut;
10. timeline zoom and snap-toggle shortcuts.

The phase must preserve:

- Project Schema v3;
- the single authoritative `CommandService`;
- ADR-001 idempotency and transaction semantics;
- ADR-003 transition invariants;
- existing preview/export semantics;
- CLI / JSONL / MCP transport equivalence;
- high-frequency pointer interactions as frontend-local transient state.

No renderer/compositor/media feature expansion belongs in Phase 16.

---

## 2. Authoritative Inputs

Implementation must follow the latest reconciled state:

- `PLAN.md`
- `PROJECT_STATUS.md`
- `RESEARCH.md`
- `ARCHITECTURE.md`
- `ADR_001_DURABLE_IDEMPOTENCY_TOPOLOGY_AND_FINGERPRINT.md`
- `ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md`
- `Plans/plan_phase15.md`
- `src-tauri/tests/phase16_research_poc.rs`
- accepted Phase-15 source-control checkpoint:
  - `48fe7b0597e925f7ab801ddab2e28af8911f3ac9`

The Phase-16 research contract TP-01 through TP-12 is authoritative for product semantics.

Source-of-truth order:

1. repository code = actual implementation;
2. accepted ADRs = durable architecture;
3. `RESEARCH.md` TP-01..TP-12 = Phase-16 semantic decisions;
4. this approved phase plan = implementation scope and acceptance criteria;
5. `PLAN.md` / `PROJECT_STATUS.md` = durable phase status.

If actual code invalidates TP-01..TP-12, stop and report instead of silently broadening the phase.

---

## 3. Pre-Implementation Source-Control Gate

The accepted Phases 0–15 state is checkpointed at:

`48fe7b0 — chore: checkpoint accepted phases 0-15`

After explicit user approval of this plan, but **before any Phase-16 production-code edit**, Codex must create a second clean planning/research checkpoint.

### 3.1 Required contents

The checkpoint must contain only the already-approved/reconciled Phase-16 planning inputs:

- `RESEARCH.md` Phase-16 reconciliation;
- `PLAN.md` Phase-16 research-gate milestone state;
- `PROJECT_STATUS.md` Phase-16 research-gate state;
- `src-tauri/tests/phase16_research_poc.rs`;
- `Plans/plan_phase16.md` with status:
  - `APPROVED FOR IMPLEMENTATION — explicitly approved by user`.

It must not contain Phase-16 production implementation.

Suggested commit message:

`docs: approve phase 16 timeline productivity plan`

### 3.2 Baseline verification

Before production implementation starts:

- `git status --short` must be clean after the planning checkpoint;
- run `cargo test`;
- confirm the accepted Phase-0..15 regression baseline remains intact;
- confirm the 12 Phase-16 research POC tests pass;
- run frontend tests once to establish the implementation baseline.

Expected known baseline before new Phase-16 acceptance tests:

- accepted Phase-0..15 Rust tests: 155;
- Phase-16 research POC: 12;
- expected Rust total after the research POC becomes committed: 167, unless repository test discovery produces a transparently reconciled count;
- frontend Vitest baseline: 14.

If counts differ, reconcile before changing production code.

---

## 4. Scope

### 4.1 In scope

#### Selection
- replace single `selectedItemId` model with ephemeral multi-selection;
- `selectedItemIds`;
- `primarySelectedItemId`;
- normal click replace;
- Ctrl+Click toggle;
- empty timeline click clear;
- selection reconciliation after canonical project updates.

#### Snapping
- UI-only snapping helper;
- exact 8 CSS pixel threshold;
- targets:
  - timeline zero;
  - current playhead;
  - start/end edges of unselected clips;
- snapping for clip move;
- snapping for group move;
- snapping for trim head/tail;
- primary dragged clip as group snap anchor;
- Alt temporary bypass;
- ephemeral snap enabled/disabled state;
- deterministic tie-breaking.

#### Group move
- new `CommandOperation::ClipGroupMove`;
- one common semantic `RationalTime` delta;
- cross-track selection allowed;
- each member retains its original track;
- simultaneous final-state validation;
- internal transition preservation;
- external transition reconciliation;
- dry-run/diff/history/idempotency/automation parity.

#### Ripple delete
- new `CommandOperation::ClipRippleDelete`;
- exactly one target item;
- same-track downstream shift only;
- no ripple insert;
- no ripple trim;
- no cross-track shift;
- no new transition synthesis;
- transition cleanup/preservation per TP-06/TP-07.

#### Shortcuts
- centralized frontend timeline shortcut hook;
- input/editable focus suppression;
- Windows-first keymap;
- play/pause;
- split;
- duplicate;
- delete selected;
- ripple delete primary;
- trim head/tail to playhead;
- zoom;
- snap toggle.

#### Existing-command workflow improvements
- split at playhead;
- normal non-ripple trim to playhead;
- atomic multi-selection delete;
- primary-only duplicate.

#### Automation
- update supported operation capability list for new commands;
- generic CLI/JSONL/MCP envelopes continue to use the same commands;
- no transport-specific editing semantics.

#### Verification
- command-engine tests;
- Phase-16 integration acceptance tests;
- frontend interaction/helper tests;
- regression tests;
- manual timeline QA;
- full build/typecheck/lint gates.

### 4.2 Explicitly out of scope

Do NOT add:

- Shift+Click range selection;
- marquee / rubber-band selection;
- multi-clip duplication;
- ripple insert;
- ripple trim;
- Q/W ripple-trim behavior;
- magnetic/FCP-style timeline;
- cross-track ripple;
- track targeting/arming;
- persistent timeline markers;
- persistent selection state;
- project-owned snapping preferences;
- new project settings schema;
- keyframe UX expansion;
- stable keyframe IDs;
- audio crossfade;
- waveform expansion;
- spectral waveform;
- new video transition kinds;
- transition easing;
- text/caption expansion;
- speech-to-text;
- preview compositor changes;
- FFmpeg export graph changes;
- Schema v4;
- new ADR unless implementation reveals an actual architecture contradiction.

---

## 5. Normative Phase-16 Contract

### P16-TP-01 — Ephemeral selection
Selection exists only in frontend memory and is never serialized into `.tcproj`.

### P16-TP-02 — Primary selection
When selection is non-empty, one item is primary and drives Inspector/single-item actions/group snap anchoring.

### P16-TP-03 — Save does not clear selection
Normal project save does not clear selection solely because selection is ephemeral.

### P16-TP-04 — Selection reconciliation
Selection is cleared/reconciled on project close/open/reload or when selected IDs no longer exist in canonical state.

### P16-TP-05 — Ctrl-only multi-selection
Normal click replaces selection; Ctrl+Click toggles; Shift range and marquee selection are out of scope.

### P16-TP-06 — Snapping is UI-only
Snapping never changes project schema or creates a special domain command.

### P16-TP-07 — Exact 8px snap threshold
A candidate target is eligible only when its actual timeline pixel distance is <= 8 CSS px.

### P16-TP-08 — Exact target commit
A snapped operation commits the exact target-derived semantic `RationalTime`, not an approximate float-derived time.

### P16-TP-09 — Selected items are not self-targets
Selected clips do not contribute snap targets during their own move.

### P16-TP-10 — Group anchor snapping
Group drag evaluates snap anchors on the primary/actively dragged clip and applies one resulting semantic delta to the whole group.

### P16-TP-11 — Alt bypass
Holding Alt suppresses snapping for the active pointer interaction.

### P16-TP-12 — `ClipGroupMove` exclusivity
Multi-clip movement must not be implemented as batched ordinary `ClipMove` operations.

### P16-TP-13 — One common group delta
`ClipGroupMove` applies one semantic `RationalTime` delta to every member.

### P16-TP-14 — Track/source invariance
Group move changes timeline position only; member tracks and source ranges remain unchanged.

### P16-TP-15 — Simultaneous validation
Group-move final positions are staged as one simultaneous state before overlap/domain validation.

### P16-TP-16 — Group negative-boundary rejection
If any member's final semantic timeline start is negative, reject the full operation.

### P16-TP-17 — Group external collision rejection
If any member's final placement overlaps an unselected item under existing same-track rules, reject the full operation.

### P16-TP-18 — Internal transition preservation
A cut transition whose two participants both move with the same group delta remains intact if ADR-003 invariants remain valid.

### P16-TP-19 — External transition reconciliation
A cut transition between a moved and unmoved participant is reconciled/removed through existing transition rules when adjacency is broken.

### P16-TP-20 — `ClipRippleDelete` is single-item/single-track
Ripple delete accepts one target item and shifts only downstream items on that target's track.

### P16-TP-21 — Ripple shift interval
Every surviving same-track item whose semantic start is at or after the deleted item's semantic end shifts left by exactly the deleted duration.

### P16-TP-22 — Ripple preserves downstream spacing
Relative offsets/gaps among downstream items remain unchanged.

### P16-TP-23 — Ripple preserves source ranges
Ripple delete changes timeline placement only; source ranges remain unchanged.

### P16-TP-24 — No synthesized cut transition
Ripple delete never automatically creates a transition between newly adjacent survivors.

### P16-TP-25 — Downstream transitions move with participants
A transition whose participants both shift remains intact when ADR-003 invariants stay satisfied.

### P16-TP-26 — Direct transitions on deleted item are removed
Incoming/outgoing transitions directly dependent on the deleted item are removed.

### P16-TP-27 — Multi-delete uses one envelope
Deleting multiple selected clips uses existing `ClipDelete` operations inside one atomic `CommandEnvelope`.

### P16-TP-28 — Duplicate is primary-only
Phase 16 does not attempt multi-clip duplicate.

### P16-TP-29 — Split is deterministic
Split shortcut targets primary intersecting clip first; otherwise lowest canonical track index containing playhead; otherwise no-op.

### P16-TP-30 — Trim shortcuts are non-ripple
Head/tail trim-to-playhead uses existing `ClipTrim`; downstream clips do not shift.

### P16-TP-31 — Keyboard input suppression
Timeline shortcuts do not fire while user is typing/editing in input, textarea or contenteditable controls.

### P16-TP-32 — Schema v3 retained
No project-schema migration is allowed in Phase 16.

### P16-TP-33 — One mutation authority
New operations execute only through `CommandService`.

### P16-TP-34 — Automation inheritance
New normal domain operations flow through generic CLI/JSONL/MCP envelopes; only capability discovery requires explicit operation-list updates.

### P16-TP-35 — RationalTime semantic comparisons
Mixed-rate movement/ripple logic must use semantic rescaling/comparison helpers, never raw cross-rate value equality/order assumptions.

---

## 6. Frontend Selection Model

### 6.1 State

Replace the current single-selection model with an equivalent of:

```ts
selectedItemIds: Set<string>
primarySelectedItemId: string | null
```

Do not persist either field.

A helper should centralize reconciliation rather than duplicating selection rules across `App.tsx`, `Timeline.tsx`, and `Inspector.tsx`.

### 6.2 Normal click

Clicking a clip without Ctrl:

- selection becomes exactly that item;
- primary becomes that item.

Clicking empty timeline canvas:

- selection becomes empty;
- primary becomes `null`.

### 6.3 Ctrl+Click

Ctrl+Click unselected item:

- add item;
- make it primary.

Ctrl+Click selected item:

- remove it;
- if it was primary, choose the most recently added remaining selected item where current Set insertion order permits;
- if none remain, primary becomes `null`.

Do not add Shift semantics.

### 6.4 Drag initiation

Pointer-down on an unselected clip without Ctrl:

- replace selection with that clip;
- begin single/group drag using it as primary anchor.

Pointer-down on a selected clip:

- retain current selection;
- set dragged clip as primary anchor;
- begin group drag for the full selected set.

Ctrl+Click should toggle selection and must not accidentally begin a drag.

### 6.5 Canonical refresh reconciliation

After a successful canonical mutation/project refresh:

- intersect selected IDs with current canonical item IDs;
- if primary no longer exists, choose an existing selected fallback;
- if no selected IDs remain, set primary null.

Normal Save must preserve selection.

Project open/reopen/reload may clear selection.

### 6.6 Inspector

When multiple clips are selected:

- Inspector shows/edit-controls for the primary selected item only;
- add a small multi-selection indicator/count;
- do not implement bulk property editing in Phase 16.

---

## 7. Snapping Engine

### 7.1 Pure helper boundary

Implement snapping as a pure frontend helper/module, not inline ad-hoc pointer arithmetic.

Equivalent input concepts:

```ts
type SnapTargetKind = "playhead" | "zero" | "clipEdge";

interface SnapTarget {
  time: RationalTime;
  kind: SnapTargetKind;
  sourceItemId?: string;
  stableOrder: number;
}

interface SnapCandidate {
  anchorTime: RationalTime;
  anchorKind: "start" | "end";
}

interface SnapResult {
  snapped: boolean;
  snappedAnchorTime: RationalTime;
  delta: RationalTime;
  target?: SnapTarget;
}
```

Exact naming may follow existing frontend conventions.

### 7.2 Target generation

Generate:

- timeline zero;
- current playhead;
- start edge of each unselected item;
- end edge of each unselected item.

Do not add transition-specific duplicate targets where the same cut is already represented by item edges.

### 7.3 Eligibility

Use display geometry only to decide snap eligibility:

```text
distancePx = abs(targetSeconds - candidateAnchorSeconds) * zoomPxPerSecond
eligible iff distancePx <= 8
```

Do not force a minimum one-frame threshold.

The committed snapped time must come from the target's canonical/semantic time representation.

### 7.4 Candidate anchors

For clip move:

- evaluate dragged clip start;
- evaluate dragged clip end;
- choose best eligible result across both.

For group move:

- evaluate start/end anchors of the primary dragged clip;
- selected non-primary clips do not contribute anchor candidates;
- apply one resulting delta to all group members.

For trim head:

- evaluate only proposed new head.

For trim tail:

- evaluate only proposed new tail.

Playhead acts as a snap target, not a Phase-16 snapped subject.

### 7.5 Tie-breaking

If multiple eligible targets are equal-distance:

1. smallest pixel distance;
2. target kind priority:
   - Playhead
   - Timeline Zero
   - Clip Edge
3. earlier semantic target time;
4. stable target order;
5. stable source item ID ordering if still tied.

### 7.6 Bypass/state

- Alt held during active drag/trim bypasses snapping.
- `S` toggles snap enabled state.
- snap state is in-memory only.
- default state: enabled.
- no project preference field.

### 7.7 High-frequency invariant

Pointer move:

- updates transient UI positions only;
- must not execute canonical Tauri/Rust commands;
- snap computation remains frontend-local.

Pointer up:

- emits exactly one canonical move/trim command/transaction.

---

## 8. `ClipGroupMove` Command

### 8.1 Operation shape

Add one normal command variant equivalent to:

```rust
ClipGroupMove {
    item_ids: Vec<String>,
    delta: RationalTime,
}
```

Update corresponding TypeScript command type.

### 8.2 Validation

Validate before mutating staged state:

- item list non-empty;
- IDs are unique;
- every item exists;
- every owning track permits editing under existing lock/edit rules;
- delta is interpreted semantically and rescaled per item timeline rate;
- every final start is >= semantic zero;
- track IDs remain unchanged;
- source ranges remain unchanged;
- final placement of moved members against all unselected same-track items satisfies existing overlap rules.

Do not validate moved members sequentially against each other's old positions.

### 8.3 Staging

Compute the complete target state first.

Apply all member timeline starts to the staged project before full project validation.

Do not call the existing single-item move path repeatedly if that path performs per-item transition reconciliation or intermediate validation.

Reuse lower-level validation helpers only when they preserve simultaneous semantics.

### 8.4 Transitions

For every transition touching any moved member:

- if both participants are in the moved set:
  - preserve transition;
  - verify final semantic adjacency and existing handle/media invariants;
- if only one participant moves:
  - use existing dependent-transition reconciliation;
  - remove when adjacency is broken;
- never synthesize a replacement transition.

### 8.5 Diff / receipt

Dry-run should expose movement for every affected clip through existing diff semantics.

Receipt/affected-entity reporting should include all moved items and any transition entity removed by reconciliation where current receipt conventions support it.

### 8.6 History

The command is one logical operation:

- one atomic apply;
- one project revision transition;
- one undo step;
- one redo step;
- one ADR-001 operation fingerprint.

### 8.7 Automation

Update `AutomationAdapter::supported_operations()` with `ClipGroupMove`.

No CLI/JSONL/MCP-specific command implementation.

---

## 9. `ClipRippleDelete` Command

### 9.1 Operation shape

Add one command variant equivalent to:

```rust
ClipRippleDelete {
    item_id: String,
}
```

Update corresponding TypeScript command type.

### 9.2 Resolve target

Locate:

- target item;
- target track;
- semantic target start;
- semantic target end;
- semantic target duration.

Honor existing track lock/editability rules.

### 9.3 Direct deletion

Remove:

- target item;
- transitions where target is left or right participant.

Do not create a new transition between newly adjacent survivors.

### 9.4 Downstream set

On target track only, downstream items are survivors whose semantic timeline start is:

`>= deleted_end`

Use semantic RationalTime comparison/rescaling.

Do not use raw `value` comparison across rates.

### 9.5 Shift

Each downstream item moves left by exactly the deleted duration rescaled to that item's timeline start rate.

Preserve:

- source range;
- duration;
- track;
- relative downstream spacing.

Other tracks are unchanged.

### 9.6 Downstream transitions

If both participants of an existing downstream transition shift:

- keep transition IDs/kind/duration unchanged;
- verify final ADR-003 invariants.

If a transition is directly dependent on deleted target:

- remove it.

No new transition is synthesized.

### 9.7 Validation/history

The full ripple is staged and validated atomically.

Required:

- rollback on any failure;
- one project revision transition;
- one undo step restores exact previous project;
- one redo step restores exact ripple result.

### 9.8 Diff / automation

Dry-run should report:

- deleted target;
- each downstream moved item;
- transition removals where represented by existing diff structures.

Update supported operation capabilities.

CLI/JSONL/MCP inherit the new operation generically.

---

## 10. Multi-Selection Delete

Use existing `ClipDelete` operations inside one existing atomic `CommandEnvelope`.

Do not add `ClipGroupDelete`.

Requirements:

- one user action;
- one envelope/transaction;
- selected item IDs are deterministic and de-duplicated;
- operation order must remain safe per research POC;
- transitions clean up through existing `ClipDelete` behavior;
- one undo/redo logical transaction;
- after success, remove deleted IDs from selection and reconcile primary selection.

If implementation discovers a case not covered by the research POC where order causes semantic divergence, stop and report rather than introducing a hidden group-delete command.

---

## 11. Duplicate

Phase-16 duplicate shortcut acts on `primarySelectedItemId` only.

Use existing `ClipDuplicate`.

Do not duplicate every selected item.

If multiple items are selected:

- `Ctrl+D` duplicates primary item only;
- existing destination/placement semantics remain unchanged;
- new duplicate becomes canonical according to existing command behavior;
- do not invent group-relative duplicate placement.

---

## 12. Split-at-Playhead

### 12.1 Shortcut

Use:

`Ctrl+K`

### 12.2 Target resolution

At shortcut time:

1. if primary selected item semantically contains playhead in its strict interior:
   - split primary;
2. otherwise inspect tracks in canonical order:
   - choose first item whose strict interior contains playhead;
3. if none:
   - clean no-op.

Do not split at item start/end boundary.

Do not split multiple selected items at once.

### 12.3 Command

Use existing `ClipSplit`.

Generate right-item ID through existing ID-generation conventions.

No new split command.

---

## 13. Normal Trim-to-Playhead

Use existing `ClipTrim`.

Shortcuts:

- `[` → trim head to playhead;
- `]` → trim tail to playhead.

Target:

- primary selected item only.

Preconditions:

- primary exists;
- playhead produces a valid non-zero resulting clip duration under existing trim semantics.

This is **not ripple trim**:

- downstream clips do not move;
- no Q/W semantics;
- existing transition reconciliation rules apply through `ClipTrim`.

Invalid trim shortcut should fail safely/no-op according to current UI command-error conventions; it must never partially edit the project.

---

## 14. Centralized Shortcut Framework

### 14.1 Implementation boundary

Introduce one small reusable frontend hook/helper, repository naming permitting:

`src/hooks/useTimelineShortcuts.ts`

or equivalent.

Do not add a third-party shortcut library.

### 14.2 Keymap

Phase-16 approved keymap:

| Action | Shortcut |
|---|---|
| Play/Pause | `Space` |
| Split at Playhead | `Ctrl+K` |
| Duplicate Primary | `Ctrl+D` |
| Delete Selected | `Delete` / `Backspace` |
| Ripple Delete Primary | `Shift+Delete` |
| Trim Head to Playhead | `[` |
| Trim Tail to Playhead | `]` |
| Zoom In | `=` / `+` |
| Zoom Out | `-` |
| Toggle Snapping | `S` |

Do not add Q/W ripple trim.

### 14.3 Focus suppression

Suppress timeline shortcuts when:

- active element is `<input>`;
- `<textarea>`;
- `contenteditable`;
- another existing editable control requires text entry.

`Escape` behavior may preserve existing editor behavior but should not grow into a new focus-management feature.

### 14.4 Existing Space handler

Remove or route the existing separate Space keydown handler through the centralized shortcut layer so one physical key press cannot toggle playback twice.

### 14.5 Browser/WebView defaults

Call `preventDefault()` only for recognized editor shortcuts when the editor owns the action.

Do not globally swallow unrelated shortcuts.

---

## 15. Timeline Drag Integration

### 15.1 Single selection

Existing single-item drag should continue to work.

At pointer move:

- calculate unsnapped semantic target;
- evaluate snapping if enabled and Alt not held;
- update transient preview only.

At pointer up:

- emit one existing `ClipMove` if one item selected.

### 15.2 Multi-selection

When dragged item belongs to a multi-selection:

- compute common delta from primary anchor;
- apply snap-adjusted delta;
- show transient positions for every selected item;
- do not move items between tracks;
- emit one `ClipGroupMove` on pointer up.

### 15.3 Drag rejection preview

Frontend may visually clamp/prevent obvious negative starts, but Rust remains authoritative.

Do not attempt to reproduce complete overlap/transition validation in frontend.

A Rust rejection must restore transient UI to canonical positions cleanly.

---

## 16. Expected Files / Modules

Expected affected areas include, but are not limited to:

### Rust
- `src-tauri/src/modules/command/mod.rs`
  - new command variants;
  - validation/execution/diff/history;
  - tests.
- `src-tauri/src/modules/automation/mod.rs`
  - supported operation names.
- potentially domain time/helper modules only if a reusable semantic-time helper is genuinely missing.

### Frontend
- `src/App.tsx`
  - multi-selection ownership;
  - command orchestration;
  - selection reconciliation.
- `src/components/Timeline.tsx`
  - selection rendering;
  - group drag;
  - snapping;
  - pointer interaction.
- `src/components/Inspector.tsx`
  - primary selection + multi-selection indicator.
- `src/services/editorApi.ts`
  - only if command helper typing requires update.
- `src/types/command.ts`
  - new command variants.
- optional focused helpers:
  - `src/hooks/useTimelineShortcuts.ts`
  - `src/utils/timelineSnapping.ts`

### Tests
- keep `src-tauri/tests/phase16_research_poc.rs` as research evidence;
- add Phase-16 production acceptance suite, preferably:
  - `src-tauri/tests/phase16_timeline_productivity_acceptance.rs`
- extend frontend Vitest coverage in existing test files or focused helper tests.

Do not modify preview/export/media modules unless required solely to fix a regression caused by Phase 16; such a requirement should normally trigger a stop/report.

---

## 17. Error and Edge-Case Handling

### Selection
- deleting primary item reconciles to remaining selection or null;
- external project reload clears/reconciles invalid IDs;
- no selected ID may crash Inspector or Timeline.

### Group move
Test/reject:
- empty `item_ids`;
- duplicate IDs;
- missing item;
- locked track;
- negative resulting start;
- collision with unselected item;
- mixed tracks;
- mixed RationalTime rates;
- member participating in internal transition;
- member participating in external transition;
- stale revision;
- idempotent replay;
- undo/redo;
- semantic zero delta according to existing command no-op policy.

### Ripple delete
Test:
- first item;
- middle item;
- last item;
- target on locked track;
- downstream gap;
- multiple downstream clips;
- mixed rates;
- target with incoming transition;
- target with outgoing transition;
- target with both dependency directions where topology permits;
- downstream transition;
- other tracks;
- stale revision;
- idempotent replay;
- undo/redo;
- project-duration shrink/no-shrink cases.

### Snapping
Test:
- exactly 8px eligible;
- >8px ineligible;
- high zoom where one frame >8px;
- tie playhead vs edge;
- tie zero vs edge;
- stable two-edge tie;
- Alt bypass;
- disabled snapping;
- selected-self target exclusion;
- group anchor only;
- trim head/tail.

### Shortcuts
Test:
- recognized actions;
- ignored while typing;
- modifier combinations;
- no double Space handling;
- no-op split/trim when invalid;
- default browser behavior prevented only when action executes/is owned.

---

## 18. Implementation Slices

### Slice A — Planning checkpoint + clean baseline

Deliver:

- durable approved `Plans/plan_phase16.md`;
- research/planning checkpoint commit;
- clean working tree before production code;
- baseline Rust/frontend tests.

Gate A:

- no Phase-16 production code in the planning commit;
- `git status --short` clean;
- accepted Phase-0..15 tests intact;
- 12/12 research POC green.

### Slice B — Command semantics

Deliver:

- `ClipGroupMove`;
- `ClipRippleDelete`;
- Rust/TS command serialization;
- supported operations update;
- diff/history/idempotency integration;
- focused command tests.

Gate B:

- group move is simultaneous and transition-safe;
- ripple delete matches TP-06/07;
- no Schema v4;
- dry-run/undo/redo/replay pass.

### Slice C — Selection model

Deliver:

- `selectedItemIds`;
- `primarySelectedItemId`;
- normal click/Ctrl toggle/empty clear;
- canonical refresh reconciliation;
- Inspector primary-selection behavior;
- selected styling for all selected clips.

Gate C:

- Save preserves selection;
- project open/reload clears/reconciles;
- no Shift/marquee;
- no persisted selection data.

### Slice D — Snapping + drag integration

Deliver:

- pure snapping helper;
- exact 8px threshold;
- deterministic tie-breaking;
- Alt bypass;
- snap toggle;
- single move snap;
- group move snap;
- trim snap;
- transient multi-item drag preview;
- one command on pointer-up.

Gate D:

- high-frequency pointer move produces zero canonical mutations;
- selected clips are not self-targets;
- high-zoom threshold remains exactly 8px;
- group commit is one `ClipGroupMove`.

### Slice E — Shortcut workflows

Deliver:

- centralized shortcut hook;
- focus suppression;
- centralized Space;
- split;
- duplicate primary;
- delete selected;
- ripple delete primary;
- trim head/tail;
- zoom;
- snap toggle.

Gate E:

- no Q/W ripple behavior;
- input typing unaffected;
- no duplicate playback toggles;
- split fallback deterministic.

### Slice F — Automation/regression integration

Deliver:

- supported operations capability entries;
- generic automation dry-run/apply tests for new commands;
- MCP/CLI/JSONL generic compatibility evidence;
- full Phase-16 integration suite;
- manual creator workflow QA.

Gate F:

- no transport-specific command logic;
- no renderer/export behavior changed;
- Schema v3 unchanged;
- all quality gates green.

---

## 19. Acceptance Criteria

### Governance / baseline

- **P16-AC-01** — Accepted Phase-0..15 checkpoint `48fe7b0` remains intact.
- **P16-AC-02** — Before production implementation, approved research/docs/POC/plan are committed as a clean planning checkpoint.
- **P16-AC-03** — No production Phase-16 code is included in the planning checkpoint.
- **P16-AC-04** — Pre-implementation test baseline is reconciled before production edits.

### Selection

- **P16-AC-05** — Normal click selects exactly one clip and makes it primary.
- **P16-AC-06** — Ctrl+Click toggles clip membership without Shift/range semantics.
- **P16-AC-07** — Empty timeline click clears selection.
- **P16-AC-08** — Multiple selected clips render visibly selected while Inspector uses primary item only.
- **P16-AC-09** — Normal Save does not clear selection.
- **P16-AC-10** — Project open/reload and canonical item deletion reconcile invalid selected IDs safely.
- **P16-AC-11** — Selection/snap state does not enter `.tcproj`.

### Snapping

- **P16-AC-12** — Snapping targets timeline zero, playhead, and unselected clip start/end edges.
- **P16-AC-13** — Snap eligibility is exactly <=8 CSS px; no minimum one-frame expansion exists.
- **P16-AC-14** — The snapped commit uses exact target semantic time.
- **P16-AC-15** — Tie-breaking is deterministic.
- **P16-AC-16** — Alt bypass disables snapping only for the active interaction.
- **P16-AC-17** — `S` toggles ephemeral snapping state.
- **P16-AC-18** — Selected clips do not snap to their own edges.
- **P16-AC-19** — Group move snaps using primary/dragged clip anchors and applies one common delta.
- **P16-AC-20** — Pointer-move snapping/dragging performs no canonical Rust mutation until commit.

### Group move

- **P16-AC-21** — `ClipGroupMove { item_ids, delta }` exists as a normal `CommandOperation`.
- **P16-AC-22** — Group move rejects empty/duplicate/missing item IDs and locked/invalid members according to existing rules.
- **P16-AC-23** — All members retain track/source range and preserve relative timeline offsets.
- **P16-AC-24** — Mixed-rate delta application uses semantic RationalTime rescaling.
- **P16-AC-25** — Any negative final start rejects the full operation.
- **P16-AC-26** — Collision with an unselected same-track item rejects the full operation.
- **P16-AC-27** — Adjacent selected items can move left/right without intermediate false-overlap failures.
- **P16-AC-28** — Internal CrossDissolve between moved members survives a valid group move.
- **P16-AC-29** — External moved/unmoved transitions reconcile under existing ADR-003 rules.
- **P16-AC-30** — Group move is one atomic operation, one revision transition, one undo/redo step and is idempotent under ADR-001.
- **P16-AC-31** — UI never implements group move as sorted/batched ordinary `ClipMove`s.

### Ripple delete

- **P16-AC-32** — `ClipRippleDelete { item_id }` exists as a normal command operation.
- **P16-AC-33** — Ripple delete affects only the target's track.
- **P16-AC-34** — Same-track items starting at/after deleted end shift left by exactly deleted duration.
- **P16-AC-35** — Downstream source ranges and relative spacing/gaps remain unchanged.
- **P16-AC-36** — Other tracks remain unchanged.
- **P16-AC-37** — Transitions directly dependent on the deleted item are removed.
- **P16-AC-38** — Downstream transitions whose participants both shift remain valid/preserved.
- **P16-AC-39** — No new transition is synthesized across the exposed cut.
- **P16-AC-40** — First/middle/last item, gaps, mixed rates and project-duration cases are covered.
- **P16-AC-41** — Ripple delete is atomic, dry-runnable, undoable/redoable and ADR-001 replay-safe.

### Existing-command workflows / shortcuts

- **P16-AC-42** — Multi-delete uses ordinary `ClipDelete`s in one atomic envelope and is one logical undo/redo action.
- **P16-AC-43** — `Ctrl+D` duplicates only the primary selected clip.
- **P16-AC-44** — `Ctrl+K` splits primary intersecting clip, otherwise lowest canonical track intersecting playhead, otherwise no-op.
- **P16-AC-45** — `[` and `]` perform normal non-ripple head/tail trim-to-playhead on primary selection using `ClipTrim`.
- **P16-AC-46** — `Delete`/`Backspace`, `Shift+Delete`, Space, zoom and snapping shortcuts follow the approved keymap.
- **P16-AC-47** — Timeline shortcuts are suppressed while typing/editing in input, textarea or contenteditable controls.
- **P16-AC-48** — Existing Space play/pause handling is centralized so one press cannot toggle twice.

### Architecture / automation / regression

- **P16-AC-49** — Project Schema remains v3 with no Phase-16 persisted selection/snapping fields.
- **P16-AC-50** — Preview/composition/export semantics are unchanged by Phase 16.
- **P16-AC-51** — New commands execute only through `CommandService`; no UI reducer bypass exists.
- **P16-AC-52** — `AutomationAdapter::supported_operations()` exposes both new operations.
- **P16-AC-53** — CLI/JSONL/MCP generic dry-run/apply paths can carry the new operations without transport-specific edit logic.
- **P16-AC-54** — Existing Phase-13/14/15 regression suites remain green.
- **P16-AC-55** — Full Rust tests, clippy, fmt, cargo build, frontend tests, TypeScript typecheck and production build pass.
- **P16-AC-56** — Manual Windows timeline QA confirms selection, snapping, group move, ripple delete and shortcuts operate coherently in the packaged/development desktop UI as applicable.

---

## 20. Required Test Matrix

### Research regression
- retain and run all 12 `phase16_research_poc` tests;
- do not weaken them to make production implementation pass.

### `ClipGroupMove`
Test at minimum:

- two adjacent same-track clips move right;
- two adjacent same-track clips move left;
- three adjacent move both directions;
- gap-separated selection;
- cross-track group;
- collision with unselected clip;
- negative boundary;
- locked member;
- duplicate ID;
- missing ID;
- mixed rates;
- internal CrossDissolve preserved;
- external transition removed;
- stale revision;
- exact replay;
- idempotency collision;
- dry-run leaves project unchanged;
- undo;
- redo;
- transaction rollback.

### `ClipRippleDelete`
Test:

- first clip;
- middle clip;
- last clip;
- downstream gaps;
- several downstream clips;
- other tracks untouched;
- target incoming transition;
- target outgoing transition;
- downstream transition preserved;
- no transition synthesis;
- mixed rates;
- locked target;
- stale revision;
- replay;
- dry-run;
- undo/redo;
- duration shrink when target track determines project end;
- no duration shrink when another track remains longer.

### Multi-delete
Test:

- adjacent selected clips;
- reversed ordering;
- cross-track selection;
- clips participating in transitions;
- one-envelope atomic rollback if a delete is invalid.

### Selection
Frontend tests:

- normal click;
- Ctrl add;
- Ctrl remove;
- primary fallback;
- empty canvas clear;
- selected styling;
- Inspector primary;
- Save preserves;
- project replacement clears/reconciles;
- selected item deletion reconciles.

### Snapping
Pure helper/frontend tests:

- 0 px;
- exactly 8 px;
- 8.01+ px;
- high zoom one-frame >8px;
- zero target;
- playhead target;
- edge target;
- tie priorities;
- stable deterministic tie;
- Alt bypass;
- disabled state;
- selected-self exclusion;
- single move start/end anchor;
- group anchor;
- trim head;
- trim tail.

### Shortcuts
Frontend tests:

- each approved keybinding;
- input suppression;
- textarea suppression;
- contenteditable suppression;
- Ctrl modifier behavior;
- Shift+Delete vs Delete;
- no Q/W ripple trim;
- Space fires once;
- split fallback;
- invalid split no-op;
- invalid trim safe behavior.

### Automation
At minimum:

- capabilities include `ClipGroupMove` and `ClipRippleDelete`;
- AutomationAdapter dry-run/apply;
- one JSONL or CLI generic-envelope smoke for each new operation;
- one MCP generic apply/dry-run smoke proving no transport-specific mapping is required.

### Regression
Run all existing:

- Phase-13 transition tests;
- Phase-14 keyframe/export tests;
- Phase-15 MCP acceptance;
- export automation POC;
- frontend existing suite.

---

## 21. Manual QA Checklist

Perform desktop UI QA on Windows:

1. select one clip;
2. Ctrl-select clips on same track;
3. Ctrl-select clips across tracks;
4. deselect primary and verify fallback;
5. drag selected group left/right;
6. drag group containing a CrossDissolve pair;
7. attempt invalid group collision and verify visual rollback;
8. snap single move to playhead;
9. snap group anchor to another clip edge;
10. trim with snapping;
11. hold Alt and confirm snapping bypass;
12. toggle snapping with S;
13. save and confirm selection remains;
14. Ctrl+K split primary;
15. Ctrl+K fallback split on lowest track;
16. trim head/tail with brackets;
17. multi-delete;
18. Shift+Delete ripple delete;
19. Ctrl+D primary-only duplicate;
20. type in text/numeric inputs and verify shortcuts do not fire;
21. Space playback works exactly once;
22. undo/redo group move;
23. undo/redo ripple delete.

Record any platform-specific key conflicts.

---

## 22. Performance and Interaction Guardrails

Phase 16 must not turn pointer gestures into high-frequency native mutations.

Required:

- pointer move is transient frontend state only;
- snap target generation should be bounded by visible/current project items and avoid unnecessary allocations where practical;
- one pointer-up = one canonical move/group-move/trim transaction;
- do not call `CommandService` on every pointer movement;
- do not build a persistent spatial index unless profiling demonstrates necessity;
- no project cache/schema addition for selection/snapping;
- no renderer changes for drag feedback.

A simple O(n) snap-target scan is acceptable for the current editor scale unless measured interaction performance fails existing UX expectations.

If performance is unexpectedly poor on representative projects, report measured evidence rather than introducing a new indexing architecture inside Phase 16.

---

## 23. Architecture / Scope Stop Conditions

Stop and report to ChatGPT if any of the following becomes necessary:

1. Schema v4 or project migration;
2. persisted selection or snapping state;
3. renderer/compositor/export graph changes;
4. a new concurrency architecture;
5. an ADR-level change to CommandService semantics;
6. group move cannot preserve valid internal transitions without redesigning ADR-003;
7. ripple delete requires cross-track or global timeline ripple semantics;
8. ripple delete requires automatic transition synthesis;
9. group move requires track migration;
10. multi-delete research result proves false in production implementation;
11. transport-specific MCP/CLI edit logic is required;
12. a third-party shortcut framework is needed;
13. stable keyframe IDs or keyframe-domain changes become necessary;
14. source-control baseline cannot remain clean/isolate Phase-16 implementation.

Do not weaken TP-01..TP-12 or acceptance criteria to bypass a stop condition.

---

## 24. Deferred / Future Work

Explicitly retain for future phases:

- Shift range selection;
- marquee selection;
- multi-clip duplicate;
- ripple insert;
- ripple trim;
- magnetic timeline;
- cross-track ripple;
- track targeting;
- markers;
- keyframe graph/curve UX;
- stable keyframe IDs;
- audio crossfade;
- multi-channel waveform;
- spectral waveform;
- transition library expansion;
- transition easing;
- richer text/caption styling/workflows;
- Schema v4 features.

Per-path or distributed MCP work remains unrelated Phase-15 deferred platform work and must not be pulled into Phase 16.

---

## 25. Completion Report Requirements

Codex final Phase-16 report must include:

1. pre-implementation planning checkpoint SHA;
2. files changed after that checkpoint;
3. confirmation production implementation began only after explicit plan approval;
4. exact new `CommandOperation` shapes;
5. `ClipGroupMove` simultaneous-validation implementation;
6. transition-preservation/reconciliation evidence;
7. `ClipRippleDelete` downstream-set implementation;
8. RationalTime semantic-comparison evidence;
9. selection state implementation;
10. snapping helper/algorithm;
11. proof exact 8px threshold;
12. group-drag frontend behavior;
13. shortcut map implementation;
14. focus suppression;
15. split fallback behavior;
16. trim behavior;
17. multi-delete behavior;
18. primary-only duplicate behavior;
19. supported operations capability changes;
20. CLI/JSONL/MCP inheritance evidence;
21. proof Schema v3 unchanged;
22. proof preview/export modules/semantics unchanged;
23. Phase-16 research POC result;
24. Phase-16 acceptance test count/results;
25. full Rust test count/results;
26. frontend test count/results;
27. clippy result;
28. fmt result;
29. cargo build result;
30. TypeScript typecheck result;
31. frontend production build result;
32. manual Windows QA result;
33. deviations from plan;
34. known issues;
35. deferred items;
36. architecture implications;
37. proposed classification exactly one:
   - COMPLETE
   - COMPLETE WITH DEFERRED ITEMS
   - INCOMPLETE
   - BLOCKED

Do NOT update `PLAN.md` or `PROJECT_STATUS.md` to Phase 16 COMPLETE before ChatGPT completion review.

Do NOT select or start Phase 17.

---

## 26. Phase Lifecycle

Required lifecycle:

```text
Research COMPLETE
→ durable research reconciliation
→ ChatGPT phase plan
→ explicit user approval
→ planning/research checkpoint
→ Codex implementation
→ Codex completion report
→ ChatGPT independent review
→ completion decision
→ durable project-state reconciliation
→ next milestone decision
```

No Phase-17 planning until Phase 16 completes this lifecycle.

---

## 27. Approval

Current state:

**APPROVED FOR IMPLEMENTATION — explicitly approved by user**

Governance note:
- ChatGPT authored the plan after Phase-16 research completion.
- User explicitly approved it with `Duyệt Phase 16`.
- Approval occurred before any Phase-16 production implementation.
