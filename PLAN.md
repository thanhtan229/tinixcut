# PLAN.md — Automation-First Desktop Video Editor

## 1. Plan Metadata

**Project:** Automation-First CapCut-like Desktop Video Editor  
**Plan status:** READY FOR IMPLEMENTATION  
**Plan date:** 2026-10-03  
**Inputs:** `RESEARCH.md`, `ARCHITECTURE.md`  
**Primary target:** Windows 10/11 desktop first  
**Delivery strategy:** Vertical increments behind architecture gates  

This plan converts the approved research and architecture into an implementation sequence. It is intentionally dependency-driven: foundational project semantics, persistence, command safety, and media boundaries must be proven before broad editor feature work.

---

## 2. Implementation Objective

Deliver a reliable desktop video editor that can:

1. create/open/save an editable project;
2. import local media;
3. place and manipulate media on a multi-track timeline;
4. preview the supported edit semantics responsively;
5. export deterministic video;
6. recover safely from common failures;
7. expose the same edit semantics to future automation through one command engine.

The project should become useful incrementally, but no milestone may violate the architectural invariants defined in `ARCHITECTURE.md`.

---

## 3. Scope Strategy

### 3.1 Initial functional scope

Prioritize:

- project lifecycle
- media import/probe
- multi-track timeline
- video/image/audio clips
- move / trim / split / delete / duplicate
- track/layer ordering
- position / scale / rotation
- crop / opacity
- basic audio gain/fade/mute
- text/captions
- simple transition
- basic linear keyframe semantics only after preview/export parity is proven
- preview/scrubbing
- deterministic H.264/AAC MP4 export
- save/reopen/recovery/relink
- automation-ready command API

### 3.2 Explicitly defer

Do not expand initial implementation into:

- full CapCut parity
- advanced effects library
- advanced masks/rotoscoping
- motion tracking
- advanced speed curves
- professional HDR/color grading
- multicam
- cloud collaboration
- cloud render
- public plugin SDK
- arbitrary native plugins
- provider-specific AI workflow
- browser/PWA parity

These are future workstreams and must not delay a reliable core.

---

## 4. Working Acceptance Baseline

Until the user changes these product assumptions, plan against:

- Windows 10/11 first
- SDR/Rec.709-style workflow
- H.264/AAC MP4 initial required export
- 1080p30 minimum representative playback/export workload
- 4K source import allowed with proxy-assisted workflow
- no guaranteed real-time 4K multi-layer playback in first release
- local-first project and media
- no required cloud connection for manual editing
- no external queue/database server

These assumptions are planning defaults, not permanent product commitments.

---

## 5. Delivery Rules

### 5.1 Architecture before convenience

If implementation conflicts with `ARCHITECTURE.md`, stop and update the architecture decision rather than bypassing the boundary.

### 5.2 Every phase must leave the repository healthy

Before a phase is considered complete:

- build passes;
- relevant automated tests pass;
- new failure paths are covered where practical;
- no known project-corruption path remains open;
- documentation/status is updated.

### 5.3 Do not accumulate hidden architecture debt

Do not implement a temporary second mutation path, temporary float-time persistence, direct UI FFmpeg calls, or direct project-file editing “just for MVP.” These temporary shortcuts touch permanent compatibility boundaries and are disallowed.

### 5.4 Feature flags are acceptable; duplicated semantics are not

A feature may remain disabled or experimental while backend work matures. It must not be implemented with a throwaway project model that later requires migration.

---

## 6. Milestone Overview

```text
Phase 0  Repository / engineering baseline
   ↓
Phase 1  Domain model + canonical time + command engine
   ↓       POC 3 gate
Phase 2  Persistence + history + recovery + relink foundation
   ↓       POC 4 gate
Phase 3  Tauri desktop shell + basic editor UI
   ↓
Phase 4  Media foundation + local job manager + FFmpeg
   ↓
Phase 5  Core timeline editing vertical slice
   ↓
Phase 6  Preview backend POC and decision
   ↓       POC 1 gate
Phase 7  Composition plan + preview/export parity slice
   ↓       POC 2 gate
Phase 8  Export/render foundation
   ↓
Phase 9  Text/audio/transitions/keyframes within bounded scope
   ↓
Phase 10 Automation surface: inspect/diff/apply
   ↓
Phase 11 Reliability/security/performance hardening
   ↓       POC 5 gate
Phase 12 Packaging + release qualification (Complete with deferred items)
   ↓
Phase 13 Transition Compositor & CrossDissolve (Complete)
   ↓
Phase 14 Advanced Keyframe Interpolation (Complete)
   ↓
Phase 15 MCP Automation Adapter (Complete)
   ↓
Phase 16 Timeline Editing Productivity (Complete with Deferred Items)
   ↓
Phase 17 Performance Requalification & Release Foundation (Complete with Deferred Items)
```

POC gates are architectural validation, not optional polish.

---

# Phase 0 — Repository and Engineering Baseline

## Objective

Create the minimum healthy project structure and engineering workflow needed to implement the architecture without prematurely building editor features.

## Workstreams

### 0.1 Repository baseline

Establish:

- Tauri 2.12.x stable baseline
- React + TypeScript frontend
- Rust core workspace/module structure
- deterministic package/tool versions
- development/build commands
- test commands
- formatting/linting

### 0.2 Documentation baseline

Repository root should contain and maintain:

- `RESEARCH.md`
- `ARCHITECTURE.md`
- `PLAN.md`
- `PROJECT_STATUS.md`
- project-wide `AGENTS.md` if used by coding agents

Do not create feature-specific research files unless new research is actually needed.

### 0.3 CI/local validation baseline

Define one standard validation path that can later run:

- TypeScript checks
- frontend tests
- Rust formatting/lint/tests
- build

## Deliverables

- bootable empty desktop shell
- documented build/test workflow
- architecture-aligned module skeleton only

## Acceptance / Exit Criteria

- desktop app launches on target development Windows machine;
- frontend/native communication can execute one harmless typed health request;
- no unrestricted shell/filesystem capability is exposed to frontend;
- baseline build and test commands succeed.

## Do Not Do Yet

- timeline feature implementation
- FFmpeg integration beyond optional environment investigation
- automation transport
- preview implementation

---

# Phase 1 — Project Domain, Canonical Time, and Command Engine

## Objective

Build the product's permanent semantic core before editor UI features accumulate.

## Dependencies

- Phase 0 complete.

## Workstreams

### 1.1 Choose canonical time representation

Decision Gate A from `ARCHITECTURE.md`.

Compare only the two approved forms:

- integer ticks + project timebase;
- rational `{value, rate}`.

Required validation cases:

- 23.976 / 24 / 25 / 29.97 / 30 / 50 / 59.94 / 60 conversions;
- frame stepping;
- range arithmetic;
- trim/split boundary preservation;
- serialization round trip.

Record the chosen representation in `ARCHITECTURE.md` if needed.

### 1.2 Project schema v1 foundation

Define only foundational concepts:

- project identity
- schema version
- revision
- project settings
- assets
- timeline
- tracks
- items/clips
- source range
- timeline placement
- transform/crop/opacity placeholders where needed by domain

Avoid overdesigning unsupported effects.

### 1.3 Stable IDs

Implement stable identity rules for:

- project
- assets
- tracks
- items/clips
- transactions/operations

### 1.4 Core invariants

Implement domain validation for:

- positive/valid durations
- valid ranges
- valid references
- legal timeline placement according to current policy
- schema integrity

### 1.5 Command registry/service

Initial operations should be sufficient for POC 3:

- create track
- insert/create clip fixture
- move
- trim
- split
- delete
- duplicate

### 1.6 Transaction/revision/idempotency semantics

Implement:

- `baseRevision`
- transaction boundary
- `operationId`
- duplicate retry handling
- revision increment after successful authoritative change
- no revision increment on dry-run/failure/no-op according to explicit policy

### 1.7 Dry-run/diff

Use the same registry/validators as real apply.

The first diff may be structural rather than beautiful UI output, but it must be semantically trustworthy.

### 1.8 History compatibility

Define command result/delta semantics needed by undo/redo. Full UI history integration may land in Phase 2, but Phase 1 operations must not make undo impossible.

## POC 3 — Command Engine / Automation Safety

Perform the POC exactly at this phase rather than after UI implementation.

Minimum experiment:

- in-memory fixture
- two thin callers representing UI and automation
- move/trim/split/delete
- multi-operation transaction
- stale `baseRevision`
- duplicate `operationId`
- dry-run
- undo/redo behavior or reversible model prototype
- serialize/reopen semantic comparison where available

## Phase Acceptance

POC 3 passes only if:

- UI-like and automation-like callers produce equivalent canonical state;
- stale revision is deterministically rejected;
- duplicate operation ID does not mutate twice;
- failed atomic transaction changes nothing;
- dry-run writes nothing;
- command semantics are independent of React/UI state;
- domain unit/property tests pass.

## Gate

**Do not build substantial timeline editing UI until this phase passes.**

---

# Phase 2 — Persistence, History, Recovery, and Media Relink Foundation

## Objective

Prove that editorial work survives saves, crashes, cache loss, and missing source files.

## Dependencies

- Phase 1 and POC 3 pass.

## Workstreams

### 2.1 Canonical serialization

Implement deterministic project serialization for schema v1.

Test:

- save/reopen semantic equivalence
- stable identity
- time representation preservation

### 2.2 Atomic save

Implement safe temporary write → commit/replace behavior.

Failure must leave a previous valid project readable.

### 2.3 Recovery/autosave model

Define:

- autosave cadence/policy
- recovery location
- newer-recovery detection
- restore/discard workflow contract

Autosave is not the canonical user save.

### 2.4 History / undo-redo

Implement logical history entries based on committed commands/transactions.

Tests must cover:

- edit → undo → redo;
- transaction undo;
- no history spam from transient pointer events;
- persistence interaction policy.

### 2.5 Asset identity model

Store enough information to support relink:

- path
- name
- size
- probe identity placeholders
- optional fast fingerprint strategy

Do not require a full content hash for every normal import unless POC/data shows it is necessary.

### 2.6 Offline media state

Project must remain openable when media is absent.

Clips remain in timeline with offline status.

### 2.7 Schema migration harness

Implement migration framework even if only v1 exists.

A test migration fixture should prove the mechanism works before users create long-lived projects.

## POC 4 — Save / Recovery / Media Relink

Minimum experiment:

- save project;
- interrupt simulated save;
- reopen previous valid state;
- delete cache location;
- move source media;
- reopen offline;
- relink source;
- verify timeline semantic state unchanged.

## Phase Acceptance

- interrupted save does not destroy last valid project;
- cache deletion does not invalidate project;
- moved media results in offline state, not deleted timeline items;
- relink restores media association;
- save/reopen preserves canonical state;
- migration harness has automated tests;
- undo/redo round trips pass.

## Gate

**Project data safety is release-blocking. Do not accept “will fix later” corruption paths.**

---

# Phase 3 — Desktop Shell and Basic Editor UI

## Objective

Create the interactive editor shell without duplicating domain semantics in the frontend.

## Dependencies

- Phase 2 pass.

## Workstreams

### 3.1 Application layout

Initial surfaces:

- project/media panel
- preview/canvas region
- timeline
- inspector
- job/status area

Visual polish is secondary to correct interaction architecture.

### 3.2 Project lifecycle UI

Support:

- new project
- open project
- save
- save as
- recovery prompt when applicable

### 3.3 Timeline presentation

Implement visualization for:

- tracks
- clips/items
- playhead
- selection
- zoom/scroll

### 3.4 Transient gesture architecture

Implement drag/trim interaction pattern:

- local transient UI state during pointer movement;
- one authoritative command on commit;
- rollback/reconciliation if command fails.

Do not commit every pointer move to Rust/history/persistence.

### 3.5 Inspector binding

Inspector reads canonical state and sends typed domain commands when the user commits changes.

## Deliverable

A basic editor capable of displaying and manipulating fixture/project data through the Command Service.

## Acceptance

- no direct frontend project-file mutation;
- drag/trim does not generate command/history spam;
- command failure reconciles UI state;
- reopening the project shows committed state;
- basic keyboard/selection behavior does not alter durable project schema unnecessarily.

---

# Phase 4 — Media Foundation, Cache, Job Manager, and FFmpeg Service

## Objective

Introduce real local media safely without making FFmpeg or cache layout part of the project format.

## Dependencies

- Phase 3 shell available.
- Persistence foundation already reliable.

## Workstreams

### 4.1 FFmpeg/ffprobe service boundary

Create typed native interfaces for:

- probe
- thumbnail extraction
- proxy creation
- audio/waveform-supporting extraction

Trusted Rust code constructs process arguments.

### 4.2 Initial media import

On import:

- register source asset;
- schedule probe;
- update metadata;
- preserve offline/error states explicitly.

### 4.3 Local Job Manager

Implement initial job classes:

- probe
- thumbnail
- proxy
- waveform

For each job define:

- priority
- max concurrency
- cancellation
- dedupe key
- stale-result rules
- progress
- cleanup

### 4.4 Cache structure

Implement rebuildable cache with typed cache identity.

The project file must not require cache paths to remain valid across machines.

### 4.5 Media fixture corpus

Add approved fixture assets covering normal and failure cases.

## Acceptance

- importing supported media does not block UI;
- repeated probe/proxy requests dedupe appropriately;
- cancelling jobs leaves project valid;
- deleting cache triggers regeneration;
- corrupt/unsupported media produces explicit error state;
- raw user/agent strings cannot inject sidecar shell commands.

---

# Phase 5 — Core Timeline Editing Vertical Slice

## Objective

Turn the architecture into a genuinely usable basic NLE workflow before solving every advanced feature.

## Dependencies

- Phase 4 pass.

## Workstreams

### 5.1 Real clip insertion

Support placing video/image/audio assets on appropriate tracks using canonical ranges.

### 5.2 Core operations

Complete UI + domain integration for:

- move
- trim
- split
- delete
- duplicate
- track ordering/layering

### 5.3 Transform basics

Add canonical semantics for:

- position
- scale
- rotation
- crop
- opacity

Do not bind semantics to preview implementation details.

### 5.4 Audio basics

Add:

- mute/enable
- gain
- simple fades if semantics are clear

### 5.5 Undo/redo UX

Validate logical history behavior under real UI gestures.

## Acceptance

A user can:

1. create project;
2. import media;
3. add multiple clips/tracks;
4. move/trim/split/delete/duplicate;
5. change basic transforms;
6. save;
7. reopen with equivalent timeline state;
8. undo/redo normal edits.

Preview may still be limited at this stage; correctness of editable state is the priority.

---

# Phase 6 — Preview Backend POC and Architecture Decision

## Objective

Measure rather than guess the initial continuous preview architecture.

## Dependencies

- real media foundation;
- core transform semantics;
- representative timeline state.

## POC 1 — Preview Backend Decision

Compare against the same canonical project/composition semantics:

### Candidate A

WebView/proxy-assisted playback.

### Candidate B

Minimal native decoded continuous preview surface or equivalent native path.

### Representative workload

At minimum:

- 1080p30 H.264;
- 4K source using downscaled/proxy preview;
- 2–3 stacked video/image layers;
- transform/crop/opacity;
- text test placeholder if available;
- two audio sources;
- repeated seeks;
- rapid scrubbing;
- 30–60 seconds continuous playback.

### Measure

- dropped frames;
- seek-to-visible-frame P50/P95;
- UI input latency;
- CPU/GPU;
- memory growth;
- A/V drift;
- stale/pending work under rapid seeks.

### Decision rule

Adopt the **simplest** backend that passes explicit target thresholds on minimum target hardware.

Do not choose native complexity merely because it is theoretically faster.

## Deliverables

- POC report/results;
- selected `PreviewBackend` implementation direction;
- architecture update if the POC changes assumptions.

## Gate

Do not build advanced preview-specific effects before this decision.

---

# Phase 7 — Normalized Composition Plan and Preview/Export Parity Slice

## Objective

Prove that one semantic composition model can drive both interactive preview and deterministic export for the supported feature subset.

## Dependencies

- Phase 6 preview choice.

## Workstreams

### 7.1 Composition plan v1

Normalize:

- timeline/source time;
- layer order;
- position;
- scale;
- rotation;
- crop;
- opacity;
- audio timing/gain;
- deterministic asset/font references where relevant.

### 7.2 Preview consumption

Selected PreviewBackend consumes the plan or a deterministic projection of it.

### 7.3 Export prototype consumption

FFmpeg-backed path consumes the same semantic plan without making FFmpeg syntax canonical.

## POC 2 — Preview / Export Semantic Parity

Minimum fixed test case:

- transform;
- crop;
- opacity;
- layers;
- one transition prototype;
- text with bundled deterministic font;
- one linear keyframe prototype;
- audio gain/fade.

Compare fixed timeline timestamps/frames.

## Pass Criteria

- exact canonical timing boundaries;
- geometry within declared pixel tolerance;
- crop/layer order match;
- keyframe values match sampled canonical times;
- text behavior within declared layout tolerance;
- audio timing within defined tolerance.

## Decision

If parity cannot be bounded without extensive backend-specific exceptions, reassess the compositor architecture before adding more effects.

---

# Phase 8 — Export / Render Foundation

## Objective

Deliver a safe, deterministic export workflow for the supported semantic subset.

## Dependencies

- Phase 7 parity gate pass.

## Workstreams

### 8.1 Export snapshot

Export starts from a specific project revision/snapshot.

Later editor changes must not silently change the in-flight render.

### 8.2 Output profile

Initial required profile:

- MP4
- H.264
- AAC
- supported project resolution/FPS subset

### 8.3 Safe output promotion

- write temporary/incomplete file;
- only promote after successful completion;
- preserve prior valid output on failure where possible.

### 8.4 Job integration

Export is a non-supersedable explicit job with:

- progress
- cancellation
- completed/failed/cancelled state
- cleanup

### 8.5 Determinism tests

For controlled fixtures, repeat export and verify semantic consistency and expected metadata/duration.

## Acceptance

- supported project exports successfully;
- cancel works safely;
- render crash does not corrupt project;
- failed export does not leave misleading final output;
- unsupported effects/codecs fail explicitly;
- preview/export parity tests stay within approved tolerance.

---

# Phase 9 — Bounded Creator Feature Expansion

## Objective

Add the minimum creator-facing features needed for a convincing CapCut-like MVP without weakening render parity.

## Dependencies

- export foundation stable.

## Workstreams

### 9.1 Text / captions

Define deterministic:

- font resolution;
- position/size;
- line/layout semantics;
- timing.

If imported fonts are supported, apply security/resource limits.

### 9.2 Audio improvements

Add only approved semantics such as:

- fades;
- simple clip-level volume automation;
- waveform UX.

### 9.3 Simple transitions

Start with a deliberately small transition set whose preview/export semantics are testable.

### 9.4 Basic keyframes

Only after interpolation is formally defined.

Initial scope should prefer linear interpolation for a small property set.

### 9.5 Captions/transcription integration boundary

If speech-to-text is added, treat transcription as a job/provider adapter that produces normal editable caption objects.

Do not let provider-specific schemas become project truth.

## Acceptance

Status: **COMPLETE WITH DEFERRED ITEMS**

Each production-supported feature has:

- project schema semantics;
- command operations;
- undo/redo;
- save/reopen tests;
- preview behavior;
- export behavior;
- parity coverage appropriate to feature risk.

### Accepted Deferred Items (Phase 9 Reconciliation)
- adjacent-cut / overlap-based `CrossDissolve` (historical Phase 9 deferral; resumed and completed in Phase 13 via ADR-003, Schema v2 cut-owned topology, and premultiplied RGBA dual-backend compositing)
- non-linear Bezier/custom easing keyframes (historical Phase 9 deferral; resumed and assigned to Phase 14 via ADR-004)
- multi-channel / spectral waveform visualization (deferred)
- audio crossfade (deferred; Phase 13 remains video-only)

### Phase 9 Retrospective Note (Do Not Reopen)
*Later-discovered correctness defect: full export did not consume dynamic linear keyframes. Remediation assigned to Phase 14 as P14-PREREQ-01.*

---

# Phase 10 — Automation Surface

## Objective

Expose the editor's existing command semantics safely to headless/agent workflows without creating new editing logic.

## Dependencies

- command engine proven;
- core editing set stable;
- persistence/revisions reliable.

## Workstreams

### 10.1 Inspect API

Provide structured reads for:

- project summary;
- tracks;
- clips/items;
- assets;
- revision;
- supported operations/capabilities.

### 10.2 Diff / dry-run

Expose the same domain dry-run used internally.

Return:

- validation errors;
- stale revision;
- change summary;
- warnings.

### 10.3 Apply API

Support:

- base revision;
- transaction ID;
- operation IDs;
- atomic transaction mode;
- structured receipts.

### 10.4 Initial transport & session model (Resolved)

Decision: **CLI-first** with **long-lived machine-readable stdio session**; **MCP deferred**.

- **Initial transport: CLI-first**. A standalone CLI entrypoint exposes headless inspection, diffing, execution, and rendering without UI dependencies.
- **Session model: long-lived machine-readable stdio**. Beyond one-shot subcommands, automation supports a long-lived interactive session over stdio (line-delimited JSON on stdin/stdout, reserving stderr strictly for diagnostic logs) to maintain an active in-memory `CommandService` / `JobManager` and avoid repeated process-spawn/file-reload overhead.
- **MCP deferred**. Model Context Protocol (MCP) server support is deferred to a future integration phase; when implemented, MCP will serve as a thin transport adapter over the identical underlying Command Service / Job Manager APIs.
- **Architecture Invariant**: Automation must strictly reuse the existing `CommandService`, `JobManager`, and `save_project_atomic` persistence path without introducing a second project reducer or parallel mutation path.


### 10.5 Render automation

Allow automation to start/query/cancel export through typed job APIs.

### 10.6 Security

Automation receives capability-limited project operations only.

No unrestricted shell or arbitrary filesystem API.

## Automation E2E Acceptance

Status: **COMPLETE** (Verified via unit, integration, protocol, and subprocess tests)

Required scenario:

1. open fixture project;
2. inspect revision;
3. dry-run plan;
4. apply plan;
5. save/reopen;
6. retry same operation ID;
7. confirm no duplicate edit;
8. attempt stale base revision;
9. confirm explicit conflict;
10. export through job API.

### Verification Summary
- **CLI & Machine-Readable Stdio Session**: Implemented in `modules::automation` and `main.rs`, supporting one-shot commands (`inspect`, `diff`, `apply`, `render`) and long-lived JSONL stdio session (`tinixcut session`, `--stdio`).
- **Durable Idempotency (P10-AC-08 & P10-AC-09)**: `command_ledger` persisted in `Project` domain model with deterministic FNV-1a operation fingerprinting. Exact replays return prior receipts (`deduplicated_noop`), 0 operations applied, revision unchanged across save/reopen process boundaries. Reused operation IDs with different payloads are rejected with `IDEMPOTENCY_COLLISION`.
- **Conflict Handling**: Unseen operations with mismatched revisions yield `STALE_REVISION`. Failed transactions leave project and ledger unmutated.
- **Export Automation**: `export.start`, `export.status`, `export.cancel` integrated through existing `JobManager` with snapshot isolation and cancellation flags.
- **Protocol & Subprocess**: Protocol version 1 strictly emits JSON lines to stdout with diagnostic logs directed to stderr. Verified through automated tests and separate real-OS subprocess lifetime execution.

---

# Phase 11 — Reliability, Security, and Performance Hardening

Status: **COMPLETE** (Verified via unit, integration, stress, security, and benchmark suites; ADR-001 v3 implemented)

## Objective

Qualify the product against failure modes rather than only happy-path demos, implementing ADR-001 v3 durable idempotency architecture.

## Dependencies

- all core workflows exist.

## Hardening & Research Gate Inputs

Concluded via ADR-001 REMEDIATED v3 and direct empirical measurements:
- Structured Scoped Operation Identity (`client_id:client_epoch:sequence_number`) as sole durable authority across unbounded horizons.
- Client Registry quota ($C_{\max} = 64$ clients, $\le 3.5 \text{ KB}$) with epoch fencing (`CLIENT_EPOCH_EXPIRED`).
- Active Window bounding ($K=5,000$ FIFO eviction); total document idempotency footprint $\le 1.65 \text{ MB}$ permanently.
- Staged Transaction Isolation (601 ns staging clone, $O(\text{editorial items})$).
- Standard SHA-256 operation fingerprints (`fp_sha256_<64hex>`); conservative rejection of unverified Phase 10 FNV-1a replays (`LEGACY_RECEIPT_UNVERIFIABLE`).

## Workstreams & Verification Evidence

### 11.1 Crash/failure scenarios
- Atomic save interruption points preserve last known-good `.tcproj` without corruption.
- Malformed/corrupt project files fail explicitly without panic (`test_corrupt_project_file_handling`).
- Corrupt media streams fail probe safely (`test_corrupt_media_probe_safe_failure`).
- Derived cache deletion preserves editorial state and idempotency replay guarantees (`test_poc4_cache_deletion_independence`).
- Moved/renamed media goes offline cleanly and relinks without item deletion (`test_poc4_moved_media_and_relink`).

### 11.2 Security review
- Tauri capabilities and native command dispatch remain least-privilege with zero shell execution capability.
- FFmpeg/ffprobe arguments constructed strictly from typed arrays (`Vec<String>`), preventing shell injection (`test_security_injection_resistance`).
- Untrusted project/metadata text treated strictly as data literals.
- SHA-256 cryptographic collision and preimage resistance verified (`test_phase11_sha256_cryptographic_fingerprint_and_collision`).

### 11.3 Performance qualification & ADR-001 v3 Benchmarks
Direct empirical measurements on target platform (Windows 11, NVMe):
- **Staged Transaction Isolation Clone**: < 1 µs (601 ns vs 2.64 ms whole-project clone; $4,394\times$ speedup).
- **Active Window Overhead ($K=5,000$)**: 1.635 MB JSON + 3.5 KB ClientRegistry (well within $\le 2.0 \text{ MB}$ budget).
- **CommandService In-Memory Footprint**: ~3.11 MB (within $\le 5.0 \text{ MB}$ budget).
- **Deduplicated Replay Lookup**: 8.5–22.7 µs ($O(1)$ HashMap lookup; within $\le 50 \ \mu\text{s}$ budget).
- **Dry-run & Composition Latency**: < 5 ms on 200-clip project (`test_long_project_scalability_benchmark`).
- **Autosave / Project Serialization**: ~6.5 ms (release) / ~52 ms (debug) on NVMe.

### 11.4 Queue/backpressure review
- `JobManager` tested under burst load of 200 jobs with deduplication keys (`test_queue_backpressure_and_concurrency_stress`).
- Mass job cancellations cleanly reach terminal `Cancelled` status with zero leaks or orphaned workers.

### 11.5 Long-project behavior
- 20-track, 200-clip benchmark project tested for dry-run, composition compilation, and atomic save/load (`test_long_project_scalability_benchmark`).
- 5,006 operations sustained ledger execution verified with active window pruning and evicted sequence protection (`test_phase11_active_window_bounding_and_past_sequence_protection`).

### 11.6 Regression & Health
- All 70 Rust tests pass (`cargo test`).
- All 8 Vitest tests pass (`pnpm run test`).
- 0 Clippy warnings (`cargo clippy -- -D warnings`).
- TypeScript typecheck & Vite build pass (`pnpm run build`).

---

# Phase 12 — FFmpeg Distribution, Packaging, and Release Qualification

## Objective

Turn the development application into a reproducible installable product with known media/runtime behavior.

## Dependencies

- Phase 11 stable.

## POC 5 — FFmpeg Distribution / Hardware Fallback

Using one explicit build configuration:

- probe agreed fixture corpus;
- create proxies/thumbnails;
- export required format;
- test supported hardware acceleration;
- disable/unavailable GPU and verify software fallback;
- record binary/source/build/license inventory.

## Packaging Workstreams

### 12.1 Pin versions

Pin and record:

- Tauri baseline;
- Rust toolchain policy;
- Node/package manager policy;
- FFmpeg/ffprobe build/version;
- bundled fonts/assets required for deterministic rendering.

### 12.2 Installer

Validate:

- clean install;
- upgrade;
- uninstall behavior;
- cache/recovery/project data policy;
- path/permission behavior on Windows.

### 12.3 Release acceptance suite

Run:

- domain suite;
- migration suite;
- media fixtures;
- automation E2E if included in release;
- preview/export parity;
- recovery/failure suite;
- supported hardware matrix smoke tests.

### 12.4 Licensing inventory

Document dependencies and required notices/source obligations.

Commercial codec/patent review is a separate legal/business requirement where applicable.

## Release Gate

Do not ship if any known issue can reasonably:

- corrupt project state;
- overwrite a valid project/output incorrectly;
- duplicate automation mutations;
- make preview/export materially disagree for a claimed supported feature;
- expose unrestricted shell/filesystem authority;
- create unbounded background work under ordinary use.

### Gate F / POC 5 Outcome: COMPLETE WITH DEFERRED ITEMS (2026-10-04)

Phase 12 Core Implementation, Packaging, and Isolated Workstation Qualification completed with final status **COMPLETE WITH DEFERRED ITEMS**.

- **Phase 12 Final Status**: `COMPLETE WITH DEFERRED ITEMS`
- **Required Qualified Export Path**: `CPU/libx264` software reference remains the only required qualified export path (100% verified across automated tests and isolated workstation environment).
- **Isolated Workstation Qualification (CM-01..CM-05)**: PASSED (5/5).
  - Sanitized PATH with 0 developer/toolchain tools on PATH; repository and build dependencies excluded from execution path.
  - Verified isolated NSIS install, bundled FFmpeg/FFprobe execution, probe, thumbnail, proxy, libx264 export, project save/reopen, upgrade rehearsal, and uninstall safety.
  - *Qualification Boundary*: Reclassified as **Isolated Workstation Qualification**, not true clean-machine/clean-VM qualification. Tests executed on the host Windows environment and do not prove behavior on a fresh Windows installation.
  - Evidence: `tests/evidence/clean_machine_qualification_report.json`.
- **Packaging & Resolution**:
  - `MediaBinaryResolver`: 100% call-site adoption; packaged mode enforces app-owned binaries with zero system PATH fallback.
  - `NSIS Package`: `TinixCut_0.1.0_x64-setup.exe` (55.91 MiB, SHA-256 `2eadaaa1f46a13f389752bec07f2ef27951e694fab26286be1f7709d6e487267`).
- **Explicit Deferred Items**:
  1. **True clean Windows 10/11 VM qualification**: DEFERRED / non-blocking for personal-use v1.
  2. **Hardware acceleration NVENC/QSV/AMF qualification**: DEFERRED / best-effort (NVENC API 13.1 requires NVIDIA driver >= 610.00; host has 595.79; QSV/AMF host hardware absent).
  3. `CPU/libx264` remains the only required qualified export path.

---

# Phase 13 — Transition Compositor & CrossDissolve

## Status: COMPLETE

Governed by **ADR-003** (`ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md`).  
Accepted by ChatGPT following comprehensive automated verification and reconciliation.

### Completed Scope
- **Schema v2 track-level `CutTransition`**: Pairwise adjacent-cut topology on `Track.transitions` with exact `RationalTime` duration and zero timeline duration change ($\Delta T_{\text{project}} = 0$).
- **v1→v2 Lossless Migration**: Preserves existing item-level Fade semantics (`fade_in`, `fade_out`, item-level `FadeIn`/`FadeOut` variants) losslessly; initializes `Track.transitions: Vec::new()`; explicit rejection of malformed legacy v1 placeholders.
- **Cut Transition Commands**: Atomic `create_cut_transition`, `update_cut_transition_duration`, and `remove_cut_transition` operations via authoritative Command Service.
- **Strict Handle Validation/Errors**: Rejection with stable machine-readable error codes `INSUFFICIENT_TRANSITION_HANDLE`, `TRANSITION_HANDLE_UNVERIFIABLE`, and `UNSUPPORTED_TRANSITION_MEDIA`. No auto-clamping, shortening, or freeze-frame synthesis.
- **Dependent Move/Trim/Split/Delete/Duplicate Semantics**: Dependent timeline operations preserve, invalidate, or adjust transitions deterministically.
- **Undo / Redo**: Reversible command lifecycle preserving exact transition identity, parameters, and references.
- **Normalized `TransitionPlan`**: Shared evaluation of source times $S_A(t)$ and $S_B(t)$ with linear normalized progress $p(t) \in [0.0, 1.0)$.
- **Isolated `PreviewTransitionGroup + plus-lighter`**: Isolated DOM stacking context with weighted A/B layers ($1-p, p$) and additive premultiplied blending, composited via `source-over` onto normal stack.
- **Bounded Local Premultiply Export Pipeline**: Normalized RGBA inputs $\to$ premultiply $\to$ bounded `xfade=transition=fade:offset=0` $\to$ unpremultiply $\to$ single contribution joined to overlay chain; suppression of ordinary A/B contributions during transition interval $[T_0, T_1)$.
- **Parity Fixtures & Negative Control**: 12 deterministic fixtures + 2 real-media fixtures passing within Gate E tolerances, with negative control proving drift detection.
- **Minimal UI**: Track transition overlay, inspector controls, and visual indicator for cut transitions.
- **Automation Exposure**: Automation commands `create_cut_transition`, `update_cut_transition`, `remove_cut_transition`, and session capability discovery.

### Acceptance Evidence
- **26 Phase-13 targeted tests passed** (Slices A–G).
- **103 Rust tests passed** (`cargo test` clean).
- **11 Vitest tests passed** (`pnpm run test` clean).
- **Static Analysis & Build**: `cargo clippy -- -D warnings` (0 warnings), `cargo fmt --check` (clean), `tsc --noEmit` / `vite build` (clean), `cargo build` (green).

### Retained Deferred Items
- Audio crossfade (`acrossfade` / equal-power audio transitions).
- Future alignment modes (start-on-cut, end-on-cut).
- Additional transition types (wipes, slides, push, iris, etc.).
- Bezier / custom easing curves for transitions (keyframe easing assigned to Phase 14 via ADR-004).
- Spectral / multi-channel waveform visualization.
- Generalized compositor / shader / plugin framework.
- MCP adapter (remains scheduled after Phase 14).
- Hardware acceleration qualification (NVENC/QSV/AMF best-effort).
- True clean Windows VM qualification (non-blocking for personal-use v1).
- HDR / advanced color management, H.265 / AV1 export, cross-platform release parity.

---

# Phase 14 — Advanced Keyframe Interpolation

## Status: COMPLETE

Governed by **ADR-004** (`ADR_004_CANONICAL_KEYFRAME_EASING_CLIP_LOCAL_LIFECYCLE_AND_SAMPLED_EXPORT_AUTOMATION_SEMANTICS.md`).  
Accepted by ChatGPT and ratified by user post-implementation (`Duyệt Phase 14`).

### Completed Scope
- **Schema v3 production promotion**: Project format promoted to Schema v3 with lossless forward migration from Schema v2 (initializes existing keyframes with `outgoing_easing: Linear`).
- **Domain model & easing types**: `KeyframeEasing` (`Linear`, `EaseIn`, `EaseOut`, `EaseInOut`, `Custom CubicBezier`), `ClipKeyframeEasingSet`, semantic validation, and last-keyframe invariant.
- **Canonical Rust evaluator**: Deterministic 32-iteration bisection cubic Bézier solver in Rust/core with control points bounded in $[0.0, 1.0]$. Exact endpoint handling; zero solver drift between preview and export.
- **Semantic RationalTime**: Keyframe equality and ordering use cross-multiplied canonical rational comparison, preventing tick mismatch across timebases. Preserved upsert command semantics.
- **Clip lifecycle operations**: Split rebase with cut-boundary value synthesis and de Casteljau subdivision; non-destructive head/tail trim with hidden keyframe offset retention.
- **Preview integration**: Shared evaluator powering preview canvas and inspector easing controls. Minimal easing UI delivered.
- **Full video export automation**: Ephemeral `ExportAutomationSchedule` compiling literal per-frame runtime command scripts (`sendcmd`) targeting `scale`, `overlay`, `rotate`, `colorchannelmixer`.
- **Phase-13 CrossDissolve coexistence**: Full animation commands route cleanly through ADR-003 premultiplied `xfade` participant streams without filtergraph conflict.
- **Full audio export volume automation**: Ephemeral sample-accurate mono `f32le` gain envelopes applied via `amultiply`, with 44.1k and 48k profile-aligned sample-rate handling, preflight disk check, and reliable cleanup.
- **Automation surface**: Automation capability exposure for inspecting and applying easing commands.
- **P14-PREREQ-01 Remediation**: Fully resolved; dynamic keyframes (linear and eased) affect full video/audio export, not only preview or single-frame snapshot. Phase 9 is not reopened.
- **Parity and negative controls**: Automated visual and audio parity verified, with negative-control tests detecting frame/timing discrepancies.

### Verification
- **Full `cargo test`**: 129 passed, 0 failed
  - Rust unit tests: 106 passed
  - Export POC tests: 8 passed
  - Phase 14 acceptance tests: 15 passed
- **Frontend tests**: 14 passed (vitest)
- **Clippy**: clean (`cargo clippy -- -D warnings`, 0 warnings)
- **Formatting**: clean (`cargo fmt --check`)
- **Cargo build**: clean (`cargo build`)
- **TypeScript typecheck**: clean (`tsc --noEmit`, 0 errors)
- **Frontend production build**: clean (`vite build`, 0 errors)
- **FFmpeg**: app-owned bundled FFmpeg 9.0.2 verified via `MediaBinaryResolver`

### P14-PREREQ-01 Resolution
- `P14-PREREQ-01` is **RESOLVED**. Full export executes dynamic keyframe automation via `ExportAutomationSchedule` and sample-accurate audio envelopes. Phase 9 remains closed and is not reopened.

### Retained Deferred Items
- Draggable graph/curve editor UI (minimal inspector preset/coordinate UI delivered).
- Motion paths / spatial tangents.
- Velocity/tangent editing.
- Overshoot Bézier curves ($y < 0$ or $y > 1$), physics/springs.
- Step/Hold interpolation mode.
- Shortest-path rotation angle wrapping.
- Transition easing curves (ADR-003 transitions remain linear progress).
- Audio crossfade (`acrossfade`, deferred to future audio phase).
- Spectral / multi-channel waveform visualization.
- MCP adapter (implemented in Phase 15).
- GPU compositor redesign.
- Stable keyframe IDs (keyframes remain addressed by property and semantic offset).


---

# Phase 15 — MCP Automation Adapter

## Status: COMPLETE

**Milestone:** `Phase 15 — MCP Automation Adapter`  
**Governed by:** ADR-005 (`ADR_005_MCP_LOCAL_STDIO_TOOLS_ADAPTER_EXPLICIT_PROJECT_ADDRESSING_AND_DURABLE_IDENTITY.md`)  
**Implementation Plan:** `Plans/plan_phase15.md` (P15-AC-01 through P15-AC-41)  
**Status:** COMPLETE — accepted by ChatGPT after implementation and remediation

### Completed Scope
- `tinixcut mcp` headless entrypoint before GUI startup;
- Local stdio MCP transport with pure stdout protocol frames and stderr diagnostics;
- MCP `2026-07-28` primary protocol target with modern stateless lifecycle;
- Official `rmcp` 3.x crates.io implementation;
- Exact eight-tool surface:
  - `tinixcut_inspect`
  - `tinixcut_capabilities`
  - `tinixcut_dry_run`
  - `tinixcut_apply`
  - `tinixcut_save`
  - `tinixcut_export_start`
  - `tinixcut_export_status`
  - `tinixcut_export_cancel`
- Explicit `projectPath` addressing on all project-scoped tools;
- No hidden project session, global active project, or handle manager;
- Authoritative mutation routing through existing canonical `CommandService`;
- One process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths;
- Durable ADR-001 caller identity passthrough (`client_id`, `client_epoch`, `sequence_number`);
- Atomic apply persistence (`load -> mutate -> save_atomic -> receipt`);
- Structured MCP/domain error boundary (`isError: true` with stable machine-readable codes);
- Export start/status/cancel lifecycle through existing `JobManager`;
- Modern `server/discover` tool and capability discovery;
- SDK-only `2025-11-25` backward compatibility via `rmcp`;
- Real spawned-subprocess stdout purity verified on Windows;
- Graceful stdin EOF shutdown with export child process and temp file cleanup;
- Schema v3 unchanged;
- No second reducer, registry, persistence format, or idempotency system.

### Verification
- Full Rust suite: 155 passed, 0 failed (106 unit, 8 POC, 15 Phase 14 acceptance, 26 Phase 15 acceptance);
- Phase-15 acceptance tests: 26 passed, 0 failed (`src-tauri/tests/phase15_mcp_acceptance.rs`);
- Frontend tests: 14 passed, 0 failed (`vitest run`);
- Clippy: clean (`cargo clippy -- -D warnings`, 0 warnings);
- Fmt: clean (`cargo fmt --check`);
- Cargo build: clean;
- TypeScript typecheck: clean (`tsc --noEmit`);
- Frontend production build: clean (`tsc && vite build`);
- Windows real MCP subprocess qualification: passed (clean stdout, stdin EOF termination, orphan child/temp cleanup).

### Deferred Scope
- Streamable HTTP, OAuth 2.0 / authorization server, remote hosting;
- Resources, Prompts, Tasks extension, MCP Apps, Sampling, Roots, Subscriptions, MRTR;
- Remote render, cloud project storage, multi-user collaboration;
- Direct model/provider integrations, plugin framework;
- Project-handle registry, project caching across MCP calls;
- Per-path locking or caching optimization (deferred; one process-local write gate shared by the MCP server);
- Distributed / cross-process file locking.

### Implementation Characteristics (Non-Blocking)
- Project reload/parse occurs per project-scoped MCP request ($O(\text{project size}) \approx 2 \text{ ms}$);
- Authoritative MCP writes serialize through one process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths (per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists);
- No cross-process locking guarantee was added (governed by ADR-001 revisions and atomic saves);
- Export jobs are process-local and terminate with the MCP process;
- Backward compatibility with 2025-11-25 is SDK-provided only.

---

# Phase 16 — Timeline Editing Productivity

## Status: COMPLETE WITH DEFERRED ITEMS

**Milestone:** `Phase 16 — Timeline Editing Productivity`  
**Governed by:** Phase 16 Research Reconciliation & Approved Plan `Plans/plan_phase16.md`  
**Implementation Plan:** `Plans/plan_phase16.md` (Approved & Implemented)  
**Implementation Status:** COMPLETE WITH DEFERRED ITEMS  
**Schema State:** Schema v3 unchanged  
**Automated Verification:** PASS (198 Rust tests, 37 frontend tests, clean clippy, fmt, build, typecheck)  
**Deferred Acceptance Item:** `P16-AC-56 — Real interactive Windows GUI qualification`  

### Completed Scope
1. Ephemeral Ctrl-based multi-select (`selectedItemIds`);
2. Primary selection (`primarySelectedItemId`);
3. Exact 8 CSS-pixel UI snapping (playhead, zero, clip edges, Alt bypass);
4. Dedicated atomic `ClipGroupMove` command (atomic simultaneous translation, delta-based, collision-checked);
5. Bounded single-track `ClipRippleDelete` command (deletes target, shifts downstream on same track via exact `semantic_ge`);
6. Centralized shortcut framework with focus suppression and Windows-first bindings;
7. Deterministic split-at-playhead workflow (`Ctrl+K`, primary clip or lowest-track fallback);
8. Non-ripple trim-to-playhead shortcuts (`[` and `]`, using existing `ClipTrim`);
9. Atomic multi-delete via one atomic envelope (batched `ClipDelete` operations);
10. Primary-only duplicate shortcut (`Ctrl+D`);
11. Timeline zoom (`=`, `-`) and snapping toggle (`S`) shortcuts;
12. Generic CLI/JSONL/MCP transport inheritance without transport-specific edit logic;
13. RationalTime semantic helper remediation (`semantic_cmp`, `semantic_ge`, etc.) and representation-based `Ord`/`Eq` contract preservation.

### Verification Baseline
- 198 Rust tests passed (107 unit, 8 POC, 15 Phase 14 acceptance, 26 Phase 15 acceptance, 12 Phase 16 research POC, 30 Phase 16 timeline productivity acceptance);
- 37 frontend tests passed across 4 test suites;
- `cargo clippy -- -D warnings`: PASS (0 warnings);
- `cargo fmt -- --check`: PASS;
- `cargo build`: PASS;
- `tsc --noEmit`: PASS;
- `pnpm run build`: PASS.

### Deferred Item: P16-AC-56
- **Identifier:** `P16-AC-56 — Real interactive Windows GUI qualification`
- **Status:** DEFERRED (NOT PASS)
- **Execution State:** Not executed. Both Codex and ChatGPT Work environments reported `BLOCKED — GUI interaction capability unavailable` due to lack of native Windows desktop GUI interaction capability. The user elected not to perform the 23-item manual checklist personally.
- **Guidance:** Automated tests are not considered a substitute for this qualification. This item should be revisited before any release milestone where interactive Windows behavior is release-critical.

### Explicitly Out of Scope / Planned Future Features
- Shift range selection
- Marquee / box drag selection
- Multi-clip duplication
- Ripple insert
- Ripple trim / Q-W keyboard semantics
- Magnetic / FCP-style timeline
- Cross-track ripple editing
- Persistent timeline markers
- Track targeting matrix
- Keyframe UX expansion
- Audio crossfade (`acrossfade`)
- Waveform visualization expansion
- New transition kinds or transition easing
- Text/caption expansion
- Project Schema v4

---

# Phase 17 — Performance Requalification & Release Foundation

## Status: COMPLETE WITH DEFERRED ITEMS

**Milestone:** `Phase 17 — Performance Requalification & Release Foundation`  
**Governing Inputs:** `RESEARCH.md` (Phase 17 Research Gate), `ARCHITECTURE.md`, ADR-001 through ADR-005  
**Research Status:** COMPLETE — transport performance contract recalibrated 2026-10-07
**Implementation Status:** COMPLETE WITH DEFERRED ITEMS — P17-AC-01..78 PASS
**Current Active Implementation Phase:** None; next phase requires roadmap gap analysis
**Schema State:** Schema v3 invariant (no Schema v4)  
**ADR Decision:** ADR-006 NOT REQUIRED (qualification-first milestone; zero production architecture/schema changes)  

### High-Level Scope Ceiling
1. **Deterministic Dual Schema-v3 Fixtures:** Standardization of Fixture A (semantic-rich) and Fixture B (20 tracks [14 video, 4 audio, 2 text], 200 items, 20 CrossDissolves scale).
2. **Opt-in Performance Qualification Harness:** Dedicated release-profile benchmark suite isolated from functional CI under `#[ignore]`.
3. **Host-Isolated Workstation CM-01..CM-07:** Formal verification of bundled self-sufficiency, software fallback, unicode paths, read-only media, atomic save, user-scope install, and uninstall data preservation.
4. **Current Release/Package Synchronization:** Pinned Tauri 2.12 NSIS installer profile (`downloadBootstrapper`, `currentUser`) and synchronization of qualification module (`src-tauri/src/modules/media/qualification.rs`) to Schema v3.
5. **Durable Release Evidence:** Cryptographic SHA-256 hashes, build provenance, environment metadata, and qualification reports.
6. **External Gate Documentation:** Explicit tracking and preservation of `P16-AC-56` (interactive Windows GUI QA) and True Clean VM qualification.

### Explicit Exclusions
- NVENC/QSV/AMF hardware acceleration qualification without suitable multi-vendor hardware environments;
- True clean-VM provisioning and automation;
- Interactive GUI qualification (remains tracked under `P16-AC-56`);
- New creator features or timeline UX expansion;
- Project Schema v4 or domain model persistence migrations;
- Compositor, timeline, or renderer architectural redesign;
- Production code modifications (Phase 17 is strictly qualification-first).

The authoritative acceptance criteria and implementation sequence are in `Plans/plan_phase17.md`. Its P17-AC identifiers are unchanged; P17-AC-31 and P17-AC-34 transport reference-host budgets were calibrated to Fixture B and documented in the 2026-10-07 amendment. W0/W1 remain informational; qualification ceilings and P17-AC-37 remain unchanged.

---

## 7. Cross-Cutting Workstreams

These run across phases rather than as separate late tasks.

### 7.1 Testing

Every domain feature should land with tests in the same change.

Do not postpone all testing until UI completion.

### 7.2 Documentation

Keep `PROJECT_STATUS.md` current with:

- completed phase/workstream;
- current gate;
- known limitations;
- unresolved blockers;
- test/build status.

Update `ARCHITECTURE.md` only for actual architecture decisions, not routine implementation notes.

### 7.3 Schema compatibility

Once real user projects exist, schema change risk increases sharply. Treat migrations as part of each schema-changing feature.

### 7.4 Error codes

As automation becomes public, preserve stable machine-readable error categories.

### 7.5 Performance

Measure representative paths as they are introduced. Avoid waiting until the product is feature-complete to discover architectural bottlenecks.

---

## 8. Dependency Map

```text
Canonical Time ──────────────┐
Project Schema ──────────────┼─> Command Engine ─> History
Stable IDs ──────────────────┘          │
                                       ├─> Persistence / Recovery
                                       ├─> UI Commands
                                       └─> Automation

Persistence / Asset Identity ─> Media Import / Relink

Media Service + Job Manager ─> Proxy / Thumbnail / Waveform
                             └> Preview workload

Project Model + Media ───────> Composition Plan
Composition Plan ────────────> Preview Backend
Composition Plan ────────────> Export Backend
Preview + Export ────────────> Parity Tests

Stable Command Engine ───────> CLI/MCP/Agent Adapter
```

---

## 9. Key Risks and Mitigation in the Plan

### Risk 1 — Preview path chosen too early

**Mitigation:** Phase 6 POC before deep playback investment.

### Risk 2 — Preview and export drift apart

**Mitigation:** Phase 7 normalized composition plan + parity gate before broad effects.

### Risk 3 — Project schema becomes incompatible after UI-first development

**Mitigation:** domain/schema/time foundation precedes deep UI.

### Risk 4 — Automation duplicates/desynchronizes edits

**Mitigation:** Phase 1 revision/idempotency/transaction POC before automation transport.

### Risk 5 — Media/cache failure loses edits

**Mitigation:** Phase 2 recovery/relink POC before media feature expansion.

### Risk 6 — FFmpeg becomes embedded as domain schema

**Mitigation:** typed media/export service + composition plan boundary.

### Risk 7 — Scope explodes into “full CapCut”

**Mitigation:** bounded feature expansion only after core/export parity; deferred feature list stays explicit.

### Risk 8 — Hardware acceleration becomes mandatory

**Mitigation:** hardware path is optimization; release gate requires documented software fallback for mandatory workflow where feasible.

### Risk 9 — Over-engineering distributed infrastructure

**Mitigation:** local bounded job manager only until remote execution becomes an actual requirement.

---

## 10. Definition of Done for a Feature

A feature is not complete merely because it works visually once.

For a project-editing feature, Definition of Done should normally include:

- domain semantics defined;
- stable command operation;
- validation/invariants;
- undo/redo behavior;
- persistence/save-reopen behavior;
- missing/offline media behavior where relevant;
- UI integration;
- preview behavior;
- export behavior if render-affecting;
- parity test if render-affecting and high risk;
- structured errors;
- tests;
- no architecture invariant violation.

For a background job, include:

- bounded concurrency;
- cancellation;
- progress;
- failure state;
- cleanup;
- dedupe/staleness policy;
- testable failure behavior.

---

## 11. Coding-Agent Execution Rules

When a coding agent implements this plan:

1. Read `ARCHITECTURE.md` before starting a new phase or architectural change.
2. Read the relevant current section of `PLAN.md` and `PROJECT_STATUS.md`.
3. Do not implement work from future phases merely because it seems convenient unless it is a minimal dependency required by the current phase.
4. If a task reveals an architectural contradiction, stop implementation of that contradiction and report it; update architecture only after an explicit decision.
5. Prefer small vertical changes with tests over large rewrites.
6. Do not create duplicate domain logic in UI/CLI/MCP.
7. Do not weaken tests to make a phase pass.
8. After completing a work unit, report:
   - what changed;
   - tests/build run;
   - remaining known issues;
   - whether phase exit criteria are satisfied.
9. Before marking a phase complete, perform a focused review against that phase's exit criteria.
10. Do not silently advance through a failed POC gate.

---

## 12. Recommended First Implementation Sequence

The first implementation session should stay narrow:

### Step 1

Establish Phase 0 repository/tooling baseline.

### Step 2

Implement only enough Phase 1 domain model to decide canonical time and represent a tiny in-memory timeline.

### Step 3

Add Command Service operations needed by POC 3.

### Step 4

Run POC 3 before building a polished timeline UI.

### Step 5

If POC 3 passes, continue to persistence/recovery Phase 2.

This ordering deliberately delays exciting UI/media work until the permanent mutation model is proven.

---

## 13. Milestone Acceptance Summary

### Foundation milestone

Phases 0–2 complete.

Must prove:

- safe canonical project model;
- command engine;
- revision/idempotency;
- save/recovery/relink;
- test harness.

### Basic Editor milestone

Phases 3–5 complete.

Must prove:

- real media import;
- usable multi-track core editing;
- save/reopen;
- undo/redo;
- bounded jobs.

### Preview/Render milestone

Phases 6–8 complete.

Must prove:

- measured preview decision;
- shared composition semantics;
- preview/export parity for supported subset;
- safe deterministic export.

### Automation-Ready MVP milestone

Phases 9–10 complete.

Must prove:

- bounded creator feature set;
- inspect/diff/apply through same command engine;
- automation E2E safety.

### Release Candidate milestone

Phases 11–12 complete.

Must prove:

- recovery/failure resilience;
- security boundaries;
- performance qualification;
- reproducible FFmpeg/runtime packaging;
- release suite pass.

---

## 14. Open Decisions to Resolve During Execution

These should be resolved only when their planned gate is reached or product scope requires them:

1. integer ticks vs rational value/rate;
2. WebView/proxy vs native preview;
3. exact text layout/rendering solution;
4. exact transition/keyframe v1 subset;
5. asset fingerprint/hash policy;
6. exact FFmpeg build flags/codecs;
7. hardware acceleration support matrix;
8. whether CLI or MCP is the first automation transport (Resolved: CLI-first in Phase 10; MCP completed in Phase 15 with local stdio transport governed by ADR-005; remote MCP deferred);
9. when macOS/Linux become release targets;
10. whether cloud/AI/plugin architecture receives a later dedicated research phase.

Do not let these unresolved choices block unrelated foundational work.

---

## 15. Plan Completion Criteria

The initial project plan is considered fulfilled when a release candidate demonstrates all of the following:

1. editable local project is canonical source of truth;
2. project opens/saves/reopens safely;
3. missing media is relinkable;
4. cache can be destroyed and rebuilt;
5. multi-track core editing works;
6. preview meets approved performance target on minimum target hardware;
7. export is deterministic for the supported feature set;
8. preview/export parity is within documented tolerances;
9. all authoritative edits use the Command Service;
10. retrying automation operation IDs cannot duplicate edits;
11. stale revisions are rejected;
12. failed atomic transaction leaves state unchanged;
13. jobs are bounded/cancellable according to policy;
14. sidecar crash does not corrupt project;
15. security review finds no unrestricted WebView/agent shell path;
16. required tests/build/package validation pass;
17. FFmpeg/runtime distribution is reproducible and documented.

---

## 16. Final Plan Direction

Implement the editor from the **inside out**, not from the visible UI inward:

```text
Domain semantics
→ safe commands
→ persistence/recovery
→ editor UI
→ media/jobs
→ core editing
→ measured preview
→ shared composition plan
→ deterministic export
→ bounded creator features
→ automation adapters
→ hardening/release
```

This sequence protects the two aspects that will be hardest to retrofit later: **durable editable project semantics** and **safe automation through the same command path as human editing**.
