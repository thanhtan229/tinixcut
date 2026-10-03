# ARCHITECTURE.md — Automation-First Desktop Video Editor

## 1. Document Metadata

**Project:** Automation-First CapCut-like Desktop Video Editor  
**Architecture status:** PROPOSED BASELINE — ready for implementation planning  
**Architecture date:** 2026-10-03  
**Source research:** `RESEARCH.md`  
**Primary target:** Desktop, Windows-first  
**Architecture style:** Desktop-first modular monolith  

This document turns the approved findings in `RESEARCH.md` into an implementation architecture. It defines system boundaries, invariants, data ownership, runtime responsibilities, reliability rules, extension points, and decision gates.

It intentionally does **not** define every source file, class, UI component, or implementation detail. Those decisions belong to implementation work and should remain subordinate to the architectural constraints here.

---

## 2. Product Architecture Goal

Build a desktop video editor with a workflow broadly comparable to a focused subset of CapCut, while making automation a first-class capability from the beginning.

The most important architectural property is:

> Manual editing, keyboard shortcuts, future scripting, CLI, MCP, and AI automation must mutate the same editable project through the same authoritative command service.

Automation must produce normal editable timeline state. It must not be a parallel rendering pipeline, a second reducer, or UI mouse/keyboard simulation.

---

## 3. Architectural Principles

### 3.1 The editable project is the source of truth

The canonical project model is owned by the application. It is not React state, a browser object graph, an FFmpeg filter graph, a Remotion composition, an MCP payload, or an AI-generated document.

### 3.2 One mutation path

All authoritative project edits pass through the Command Service.

```text
UI gesture / shortcut ─────┐
CLI / MCP / Agent ─────────┼──> Command Service ──> Project Domain Model
Future scripting ──────────┘
```

No client is allowed to bypass domain validation for normal edits.

### 3.3 Separate domain semantics from media implementation

The project describes **what the edit means**. Media backends implement **how to preview or render it**.

FFmpeg commands, decoder handles, GPU surfaces, WebView media elements, cache paths, and temporary render graphs are implementation details and must not leak into durable project semantics.

### 3.4 Preview and export may differ technically, not semantically

Preview and export may use different execution backends, but both consume the same normalized composition semantics.

### 3.5 Reliability before feature breadth

A smaller editor that saves safely, reopens correctly, relinks missing media, rejects stale automation, and exports predictably is preferable to a wider feature set with project corruption or preview/export divergence.

### 3.6 Local-first before distributed architecture

The first product is a local desktop application. Redis, remote queues, distributed workers, cloud render orchestration, collaboration services, and always-on backend infrastructure are out of scope until an actual product requirement justifies them.

### 3.7 Heavy work is bounded and cancellable

Media probing, proxies, thumbnails, waveforms, analysis, preview decode work, and export must not create unbounded work queues.

### 3.8 Security boundaries are explicit

Imported files, project files, subtitles, fonts, metadata, agent requests, future plugins, and provider responses are untrusted input.

---

## 4. Working Product Assumptions

These assumptions make the implementation plan actionable. They are **not permanent product commitments** and may be changed through an explicit architecture decision.

### 4.1 Initial platform

- Windows 10/11 first-class target.
- Architecture remains compatible with future macOS/Linux work.
- Cross-platform release parity is not required for the first usable milestone.

### 4.2 Initial output profile

- SDR / Rec.709-style workflow only for v1.
- MP4 with H.264 video + AAC audio as the initial required export path.
- 1080p30 is the minimum performance acceptance workload.
- 4K source import should be supported through proxy/downscaled preview where needed, but real-time 4K multi-layer playback is not an initial architectural promise.
- 60 fps, HDR, advanced color management, AV1/H.265 launch requirements remain explicit future decisions.

### 4.3 Initial editing scope

Architecture must support growth, but the first implementation should focus on:

- multi-track video/image/audio
- move / trim / split / delete / duplicate
- track/layer ordering
- transform: position, scale, rotation
- crop and opacity
- basic audio gain/fade/mute
- text/captions
- simple transitions
- basic linear keyframes only after parity semantics are defined

Advanced masks, tracking, speed curves, compositing modes, effects marketplace, advanced color tools, multicam, collaborative editing, and plugin SDK are deferred.

### 4.4 Automation scope

- Local automation first.
- CLI/MCP may be added after the Command Service stabilizes.
- Built-in AI/provider selection is not part of the core architecture.
- Cloud automation is out of v1.

---

## 5. System Context

```text
┌────────────────────────────────────────────────────────────────────┐
│                         Desktop Application                         │
│                                                                    │
│  ┌────────────────────┐       ┌─────────────────────────────────┐ │
│  │ React / TypeScript │       │ Rust Application Core           │ │
│  │ Editor UI          │──────▶│                                 │ │
│  │                    │       │ Command Service                 │ │
│  │ - Timeline         │       │ Project Domain Model            │ │
│  │ - Canvas controls  │       │ History / Undo-Redo             │ │
│  │ - Inspector        │       │ Persistence / Recovery          │ │
│  │ - Media browser    │       │ Media Services                  │ │
│  │ - Progress/status  │       │ Job Manager                     │ │
│  └────────────────────┘       │ Render Plan Builder             │ │
│             │                 │ Security-sensitive I/O          │ │
│             │                 └──────────────┬──────────────────┘ │
│             │                                │                    │
│             │                         ┌──────▼──────┐             │
│             │                         │ Preview     │             │
│             │                         │ Backend     │             │
│             │                         └─────────────┘             │
│             │                                │                    │
│             │                         ┌──────▼──────┐             │
│             │                         │ Export      │             │
│             │                         │ Backend     │             │
│             │                         └──────┬──────┘             │
│             │                                │                    │
│             │                      ┌─────────▼─────────┐          │
│             │                      │ FFmpeg / ffprobe │          │
│             │                      │ controlled       │          │
│             │                      │ sidecars         │          │
│             │                      └───────────────────┘          │
└─────────────┼──────────────────────────────────────────────────────┘
              │
       future local adapters
              │
      ┌───────▼────────┐
      │ CLI / MCP / AI │
      └────────────────┘
```

---

## 6. Runtime and Technology Baseline

### 6.1 Desktop shell

**Baseline:** Tauri 2.12.x stable line.

Responsibilities:

- application/window lifecycle
- secure UI ↔ native bridge
- file/folder pickers
- controlled native capabilities
- installer/update integration later
- controlled sidecar execution

The frontend must not receive unrestricted shell capability.

### 6.2 UI runtime

**Baseline:** React + TypeScript.

Frontend responsibilities:

- timeline presentation
- canvas overlays and controls
- inspector panels
- media library presentation
- keyboard shortcuts
- transient gesture state
- selection/focus state
- job/progress presentation
- user-facing validation/errors

Frontend non-responsibilities:

- canonical project mutation rules
- project schema migrations
- authoritative revision management
- direct FFmpeg command construction
- unrestricted filesystem/process access
- durable project persistence semantics

### 6.3 Native core

**Baseline:** Rust.

Core responsibilities:

- domain model and invariants
- command registry and execution
- revision/precondition checks
- transactions
- idempotency receipts
- history/undo/redo
- schema validation/migrations
- atomic persistence and recovery
- asset identity/relink logic
- job manager
- media service abstraction
- render-plan generation
- security-sensitive path/process handling

### 6.4 Media tools

**Baseline:** pinned FFmpeg + ffprobe distribution behind application-owned interfaces.

Use initially for:

- probing
- proxy generation
- thumbnails/filmstrips
- waveform-supporting decode/extraction
- transcodes
- audio processing
- deterministic export/encode/mux

Continuous preview implementation remains behind a backend abstraction until POC 1 is completed.

---

## 7. Module Boundaries

The application is a modular monolith. Modules communicate through explicit typed interfaces rather than shared mutable implementation state.

### 7.1 Project Domain

Owns:

- project schema
- timeline structures
- asset references
- canonical time
- edit invariants
- schema version
- project revision

Must have no dependency on React, Tauri UI state, MCP, provider SDKs, or FFmpeg command syntax.

### 7.2 Command Service

Owns:

- operation registration
- request validation
- preconditions
- `baseRevision`
- transaction execution
- idempotency
- revision increment
- structured result/error
- dry-run/diff
- integration with undo/redo

Every authoritative edit must be representable as a command operation.

### 7.3 History / Undo-Redo

Owns reversible authoritative project changes.

History should operate at logical edit boundaries, not pointer-event frequency.

Examples:

- dragging a clip for 700 ms → one committed move history entry
- resizing trim handle continuously → one committed trim history entry
- multi-operation automation transaction → one logical transaction entry unless product semantics explicitly split it

### 7.4 Persistence and Recovery

Owns:

- project serialization
- canonical save file
- atomic save
- autosave/recovery file
- schema migrations
- validation on load
- corruption reporting

Cache and generated media are never required for project correctness.

### 7.5 Asset / Media Catalog

Owns logical media identity and metadata.

Responsibilities:

- source path/URI
- filename and size metadata
- duration/streams/timebase
- optional fingerprint/hash
- online/offline state
- relink matching
- derived cache references

Moving a file must not silently delete timeline clips.

### 7.6 Job Manager

Owns long-running and asynchronous local work.

Examples:

- probe
- fingerprint/hash
- proxy
- thumbnail
- waveform
- analysis/transcription later
- export

Each job class defines:

- concurrency limit
- priority
- cancellation semantics
- dedupe key
- retry policy
- stale-result policy
- progress model
- cleanup behavior

### 7.7 Composition / Render Plan Builder

Transforms canonical project state into backend-independent composition semantics.

It is the contract between editing semantics and preview/export implementation.

### 7.8 Preview Backend

Owns responsive playback/scrubbing implementation.

Backend remains replaceable. Candidate implementations:

- WebView/proxy-assisted preview
- native decoded surface

The domain model must not know which backend is active.

### 7.9 Export Backend

Consumes a normalized render plan and produces deterministic final output.

Initially implemented using FFmpeg where practical.

### 7.10 Automation Adapters

Future CLI/MCP/built-in agent adapters are transports only.

They may:

- inspect project
- request dry-run/diff
- apply typed plans
- start render/export jobs
- read structured results

They may not:

- mutate project files directly
- construct unrestricted shell commands
- bypass the command registry
- maintain a parallel timeline reducer

---

## 8. Canonical Project Model

### 8.1 Top-level concepts

A project must include at least:

```text
Project
├── projectId
├── schemaVersion
├── revision
├── settings
├── assets[]
├── timeline
│   └── tracks[]
│       └── items[]
├── markers / metadata
└── supported project-owned extension data
```

### 8.2 Project settings

Minimum settings:

- width
- height
- canonical timeline timebase
- nominal output FPS
- audio sample-rate/channel policy
- project color mode for supported scope

### 8.3 Asset

Minimum logical fields:

- `assetId`
- media kind
- source locator/path
- original filename
- size
- probe metadata
- duration and stream information
- fingerprint/hash when policy requires
- online/offline status derived at runtime

Generated proxies/thumbnails/waveforms are not canonical media.

### 8.4 Track

Minimum logical fields:

- `trackId`
- kind/type
- ordering
- enabled/muted/locked state as supported
- items

### 8.5 Timeline item / clip

Minimum concepts:

- stable `itemId` / `clipId`
- asset reference or generated-content descriptor
- timeline placement/range
- source range
- transform
- crop
- opacity
- audio properties when applicable
- text/caption properties when applicable
- transitions/effects/keyframes only when formally supported

### 8.6 Stable identifiers

IDs are durable project identity, not array indexes or UI keys.

IDs must remain stable across:

- save/reopen
- automation inspect/apply cycle
- undo/redo when the logical object survives
- cache regeneration

### 8.7 Canonical time

Arbitrary floating-point seconds are prohibited as authoritative timeline storage.

Approved representations:

1. integer ticks with declared project timebase; or
2. rational `{ value, rate }`.

A single representation must be selected during Phase 1 and used consistently.

Seconds, milliseconds, SMPTE-like display, and frame numbers are boundary/display representations only.

### 8.8 Source range vs timeline range

The model must distinguish:

- where a clip appears in the edited timeline
- which range of the source asset it reads

Trim and split operations must preserve this distinction.

### 8.9 Project schema versioning

Every persisted project has a `schemaVersion`.

Rules:

- breaking schema changes require explicit migration
- migrations are deterministic and tested
- old project versions are never silently interpreted under new semantics
- migration failure must not overwrite the original project

---

## 9. Command Service Contract

### 9.1 Conceptual envelope

```json
{
  "schemaVersion": 1,
  "projectId": "project-...",
  "baseRevision": 42,
  "dryRun": false,
  "transactionId": "tx-...",
  "operations": [
    {
      "operationId": "op-...",
      "type": "clip.trim",
      "targetId": "clip-...",
      "parameters": {}
    }
  ]
}
```

Exact wire format may evolve, but the semantics below are architectural requirements.

### 9.2 Required semantics

- strict typed validation
- stable IDs
- explicit project identity
- `baseRevision`
- deterministic precondition checks
- dry-run/diff using the same validation and reducers as apply
- optional atomic multi-operation transaction
- idempotent `operationId`
- stable structured error codes
- revision increments only on successful authoritative change
- failed atomic transaction leaves project unchanged

### 9.3 Revision conflict

A stale `baseRevision` is rejected explicitly.

The system must not silently rebase an automation plan onto newer project state.

The caller must inspect again and regenerate/reconfirm the plan.

### 9.4 Idempotency

If the same `operationId` is retried after an uncertain response, it must not apply the mutation twice.

The service should return or reconstruct the prior receipt when valid.

### 9.5 Dry-run / diff

Dry-run must:

- validate the exact operations that real apply would execute
- check current project/revision
- report invalid targets and unsupported operations
- compute an understandable change summary
- write nothing

Dry-run must not be implemented by a separate approximate validator.

### 9.6 Command granularity

Commands represent meaningful domain edits, not pointer events.

Examples:

- `clip.move`
- `clip.trim`
- `clip.split`
- `clip.delete`
- `clip.duplicate`
- `track.create`
- `item.transform.set`
- `audio.gain.set`
- `text.update`

Naming is illustrative, not final API specification.

---

## 10. UI Interaction Architecture

### 10.1 Transient gesture state

During drag/trim/scrub:

- UI may maintain local transient geometry
- updates may occur at animation-frame cadence
- authoritative project revision must not change for every pointer event
- disk persistence must not happen for every pointer event

On gesture commit:

- UI sends one domain command
- core validates and applies it
- canonical state is returned/published
- one history entry is created

### 10.2 Selection and presentation state

Ephemeral UI state such as:

- selected item
- hovered handle
- open panel
- current zoom
- transient playhead drag

must not be mixed into the durable project schema unless it has real project meaning.

### 10.3 UI state synchronization

The UI consumes authoritative snapshots/events from the core after committed edits.

Optimistic UI is allowed only where rollback/reconciliation is deterministic.

---

## 11. Persistence, Autosave, and Recovery

### 11.1 Canonical save

Project save must be atomic:

1. serialize validated project
2. write temporary file in safe location/context
3. flush/close as supported
4. replace/rename canonical project file
5. preserve previous valid state if commit fails

### 11.2 Autosave / recovery

Recovery state is separate from the user-selected canonical project file.

Goals:

- recover recent work after crash
- never turn a failed autosave into canonical corruption
- allow user to identify/recover a newer recovery snapshot

### 11.3 Cache independence

Deleting the complete cache directory must not invalidate the project.

The application may become temporarily slower while regenerating derivatives, but editorial state remains intact.

### 11.4 Missing media

If source media is unavailable:

- asset becomes offline
- timeline items remain
- timing and edit metadata remain
- user can relink
- relink should verify identity using available metadata/fingerprint policy

### 11.5 Portable project

A future explicit `Collect/Package Project` operation may bundle:

- project metadata
- selected/all source media
- fonts or other required assets if policy permits
- optional proxies

Normal save should not copy all source media by default.

---

## 12. Media and Cache Architecture

### 12.1 Source media

Source media is immutable from the editor's perspective unless the user explicitly invokes a destructive external operation outside normal editing semantics.

### 12.2 Derived cache

Derived cache may include:

- probe result cache
- thumbnails
- filmstrips
- waveforms
- proxies
- analysis artifacts
- temporary render intermediates

Derived artifacts must be rebuildable.

### 12.3 Cache keys

Cache identity should derive from enough source identity plus generation parameters to avoid stale reuse.

Example inputs:

- asset identity/fingerprint
- source modification metadata where trusted
- requested proxy profile
- thumbnail resolution/time
- generator version where output semantics change

### 12.4 Cache invalidation

Cache invalidation must never mutate project semantics.

---

## 13. Job Manager Architecture

### 13.1 Job lifecycle

Canonical states:

```text
queued -> running -> completed
                 -> failed
                 -> cancelled
```

Additional internal states are allowed if needed, but user-facing semantics must remain clear.

### 13.2 Job policy

Each job type must define:

- priority
- max concurrency
- cancellation safety
- retry eligibility
- retry limit/backoff if applicable
- dedupe key
- stale-result behavior
- progress reporting
- temporary-output cleanup

### 13.3 Latest-relevant jobs

Interactive jobs such as seek thumbnail/decode may invalidate older pending work.

Older obsolete results must not overwrite newer requests.

### 13.4 Non-supersedable jobs

Export/render must not be silently replaced by newer work.

Only explicit cancellation stops an export.

### 13.5 No external queue initially

No Redis, RabbitMQ, Kafka, database worker queue, or cloud scheduler in v1.

---

## 14. Composition / Render Plan

### 14.1 Purpose

The normalized composition plan is the semantic bridge between project state and rendering backends.

It should describe evaluated or evaluatable rendering intent without embedding FFmpeg syntax.

### 14.2 Semantics initially required

For the supported MVP set, the plan should normalize:

- timeline timing
- source timing
- track/layer ordering
- position
- scale
- rotation
- crop
- opacity
- text/caption layout parameters
- audio gain/fades
- supported transition semantics
- supported keyframe interpolation semantics

### 14.3 Unsupported feature behavior

A feature unsupported by a backend must produce an explicit capability/error result.

Silent approximation is prohibited for final export.

Preview approximation, if ever allowed for performance, must be explicit and visually indicated/documented by feature policy.

---

## 15. Preview Architecture

### 15.1 Boundary

```text
Project Domain
   -> Composition Plan
      -> PreviewBackend interface
```

### 15.2 Decision deferred to POC

Do not hard-code project semantics around either:

- HTML/WebView media playback; or
- native Rust/FFmpeg decoding.

### 15.3 Required preview behaviors

Regardless of implementation:

- bounded pending work
- cancellation/stale seek rejection
- no unbounded frame buffering
- defined audio clock/authority
- timeline remains responsive during playback
- rapid seek does not accumulate obsolete decode work
- backend failure does not corrupt project state

### 15.4 Preview acceptance gate

POC 1 must decide the initial backend using measurable workload tests before deep playback implementation.

---

## 16. Export Architecture

### 16.1 Deterministic input

Export consumes:

- a specific project revision/snapshot
- a normalized composition plan
- an explicit output profile

Edits made after export starts do not mutate the in-flight render implicitly.

### 16.2 Output safety

Export writes to a temporary/incomplete path.

Only after successful render, encoding, muxing, and close does the application promote the output to the final destination.

Existing valid output must not be destroyed by a failed render unless the user explicitly confirms replacement semantics and the safe replacement step succeeds.

### 16.3 Progress and cancellation

Export must expose:

- progress
- current state
- cancellation
- actionable failure reason

### 16.4 FFmpeg abstraction

FFmpeg CLI/filter syntax is generated by trusted backend code from typed render-plan data.

Raw agent/user text must never become an executable FFmpeg/shell command string.

---

## 17. Preview / Export Parity Contract

For every feature considered production-supported, define canonical semantics and parity tolerance.

Examples:

- exact timeline boundary at canonical tick
- transform geometry within defined pixel tolerance
- deterministic layer order
- crop rectangle semantics
- opacity evaluation
- keyframe interpolation
- font identity and text layout rules
- audio start/end/fade tolerance

A feature must not be considered fully supported until parity tests exist at an appropriate level.

---

## 18. Automation Architecture

### 18.1 Automation is a client, not a second editor

Automation performs:

```text
inspect -> propose plan -> diff/dry-run -> apply -> receipt
```

### 18.2 Initial automation capabilities

When added, the first surface should expose only reviewed domain operations and queries.

Recommended groups:

- project inspect
- track/item inspect
- supported operation schema discovery
- dry-run/diff
- apply transaction
- job start/status/cancel for export

### 18.3 Capability model

Automation must not automatically receive:

- unrestricted filesystem enumeration
- arbitrary shell access
- arbitrary process spawn
- project-file direct write permission

### 18.4 Audit/receipts

Automation apply should return structured receipts including at least:

- transaction/operation IDs
- project revision before/after
- applied/no-op status
- errors/warnings
- changed entity IDs or summary where useful

### 18.5 UI and automation equivalence

If a normal edit can be performed manually and is exposed to automation, both paths must invoke equivalent domain semantics.

---

## 19. Error Model

Errors should be structured, stable, and classifiable.

Required categories include:

- validation error
- stale revision
- entity not found
- unsupported operation
- unsupported media/codec
- offline/missing asset
- permission/path error
- low disk/storage error
- job cancelled
- sidecar/process failure
- render failure
- corrupted project
- schema migration failure
- internal invariant violation

Human-readable details may evolve; machine-readable codes should remain stable when public automation depends on them.

---

## 20. Security Architecture

### 20.1 Trust boundaries

Treat as untrusted:

- media containers
- metadata
- filenames
- subtitles/caption files
- fonts
- project files received from elsewhere
- AI-generated commands
- remote provider payloads
- future plugin packages/data

### 20.2 Frontend capability policy

Use least privilege per Tauri window/WebView.

The editor frontend must not receive unrestricted:

- shell
- process launch
- arbitrary filesystem write
- arbitrary external navigation

### 20.3 Native process execution

All sidecar invocation:

- originates in trusted native code
- uses typed arguments
- enforces allowed executable identity
- validates paths/options
- does not concatenate raw untrusted strings into shell commands

### 20.4 Project safety

Project files must never be able to define arbitrary executable commands.

### 20.5 Future plugins

Preserve architectural seams, but do not build public native plugin execution in v1.

A future plugin system requires a separate security/capability design and should not inherit unrestricted Rust/native access by default.

---

## 21. FFmpeg Distribution and Licensing Boundary

Before public/commercial packaging:

- pin exact FFmpeg version
- pin/document build configuration
- record enabled libraries/codecs
- verify intended application license compatibility
- maintain required notices/source obligations
- test software fallback where hardware acceleration is unavailable
- document unsupported formats clearly

Do not auto-update to arbitrary “latest FFmpeg” without qualification.

---

## 22. Testing Architecture

### 22.1 Test pyramid priority

1. pure domain/unit tests
2. property/invariant tests
3. persistence/migration tests
4. media integration tests
5. render/parity tests
6. UI integration/E2E
7. automation E2E
8. crash/recovery/failure tests

### 22.2 Domain tests

Required coverage includes:

- move/trim/split/delete/duplicate
- time conversions
- source/timeline range rules
- revision conflicts
- idempotency
- transaction rollback
- undo/redo
- serialization round trips
- schema migration round trips

### 22.3 Invariant tests

Examples:

- no negative duration
- valid source range
- duplicate idempotent operation does not duplicate state
- failed transaction preserves project equivalence
- undo of a committed edit restores equivalent canonical state

### 22.4 Media fixture corpus

Maintain a small deterministic corpus covering approved formats plus failure cases:

- CFR and VFR
- common container/codecs in supported matrix
- multiple audio sample rates
- rotated metadata
- missing source
- corrupt/truncated media
- unusual timebases

### 22.5 Automation E2E

At minimum:

1. inspect
2. dry-run
3. apply
4. save/reopen
5. retry same operation ID
6. verify no duplicate mutation
7. apply with stale revision
8. verify conflict
9. export

### 22.6 Recovery tests

At minimum:

- kill during autosave
- kill sidecar during export
- delete cache
- move source media
- low disk
- read-only destination
- malformed project
- unsupported codec
- cancel proxy/export

---

## 23. Performance Architecture

### 23.1 High-frequency operations

The following must avoid synchronous heavy native/disk work per event:

- playhead drag
- clip drag
- trim drag
- canvas transform drag
- zoom/scroll

### 23.2 Bounded work

Decode/proxy/thumbnail tasks must have explicit queue bounds and cancellation/staleness rules.

### 23.3 Proxy strategy

Proxy generation is a performance feature, not a correctness requirement.

The project points to original media; preview may select a proxy representation based on policy.

### 23.4 Hardware acceleration

Hardware acceleration is an optimization with software fallback where required by the support matrix.

Project files must not depend on the GPU/codec implementation available on the machine that created them.

---

## 24. Deployment Model

Initial deployment:

```text
Single desktop app
├── Tauri WebView UI
├── Rust core
├── controlled FFmpeg/ffprobe binaries
├── local project/recovery storage
└── local derived-media cache
```

No required server connection for core editing.

Future optional components such as cloud sync, remote render, provider APIs, telemetry, update service, or collaboration must integrate around the existing project/command boundaries rather than replacing them.

---

## 25. Architecture Decision Gates

### Gate A — Canonical time representation

**When:** Phase 1.  
**Decision:** integer ticks + timebase vs rational `{value, rate}`.  
**Requirement:** deterministic frame/time conversion tests across target frame rates.

### Gate B — Command engine safety

**When:** early Phase 1 POC.  
**Decision:** validate shared UI/headless command path before editor feature growth.  
**Pass condition:** POC 3 criteria from `RESEARCH.md`.

### Gate C — Save/recovery/media identity

**When:** before substantial UI feature accumulation.  
**Pass condition:** POC 4 criteria from `RESEARCH.md`.

### Gate D — Preview backend

**When:** before deep playback implementation.  
**Decision:** WebView/proxy-assisted vs native continuous preview.  
**Pass condition:** POC 1 measured criteria.

### Gate E — Preview/export parity

**When:** immediately after a candidate preview path and basic render plan exist.  
**Pass condition:** POC 2.

### Gate F — FFmpeg distribution/hardware fallback

**When:** before packaging/release qualification.  
**Pass condition:** POC 5.

---

## 26. Architecture Invariants

These rules are considered non-negotiable unless `ARCHITECTURE.md` is explicitly revised with rationale.

1. Canonical project state is application-owned and versioned.
2. UI and automation use one authoritative Command Service.
3. Stable entity IDs are used; array position is not identity.
4. Canonical time is integer/rational, not arbitrary float seconds.
5. Failed atomic mutations leave project state unchanged.
6. Stale automation revision is rejected explicitly.
7. Retried idempotent operation cannot duplicate mutation.
8. High-frequency gesture events do not each become canonical history/persistence/IPC mutations.
9. Cache is rebuildable and not required for project correctness.
10. Missing media does not delete editorial state.
11. Project schema does not embed FFmpeg command/filter syntax.
12. Preview and export share normalized semantics.
13. Unsupported final-render semantics fail explicitly rather than silently approximating.
14. WebView/automation never receives unrestricted shell access.
15. External queue/microservice infrastructure is not introduced without a concrete distributed requirement.
16. A third-party editor repo is not adopted as the product source of truth without a new research/architecture decision.

---

## 27. Explicit Non-Goals for Initial Architecture

The architecture does not require v1 implementation of:

- full CapCut feature parity
- cloud collaboration
- real-time multi-user editing
- cloud render farm
- public plugin marketplace
- arbitrary third-party native plugins
- professional HDR/color-management suite
- multicam
- motion tracking
- advanced masks/rotoscoping
- AI provider lock-in
- Redis or distributed queue
- browser/PWA parity

These may be added later only through existing domain/command/render boundaries or a documented architecture revision.

---

## 28. Architecture Inputs for Implementation

Implementation should start in this dependency order:

```text
Project Model + Time
        ↓
Command Service + Transactions + Revisions
        ↓
Persistence + Recovery + History
        ↓
Tauri/React editor shell
        ↓
Media catalog + Job Manager + FFmpeg service
        ↓
Core timeline operations
        ↓
Composition Plan
        ↓
Preview POC / backend
        ↓
Export backend + parity
        ↓
Automation adapters
        ↓
Hardening / performance / packaging
```

Deep UI feature work must not outrun the domain/command foundation.

---

## 29. Open Architecture Decisions

These are intentionally unresolved and must not be guessed deep inside implementation modules.

1. Exact canonical time representation.
2. Exact preview backend after POC 1.
3. Exact text layout/rendering strategy needed for preview/export parity.
4. Exact initial transition/keyframe subset.
5. Asset fingerprint strength and when full hashing is required.
6. Exact FFmpeg build configuration and hardware acceleration matrix.
7. Whether linked libav/native compositor becomes necessary after preview/render measurements.
8. Cross-platform release schedule.
9. Future plugin sandbox model.
10. Future cloud/AI provider model.

---

## 30. Architecture Change Rule

Any implementation proposal that conflicts with an invariant or changes one of the following must update this document before proceeding:

- project source of truth
- canonical time model
- command mutation path
- persistence semantics
- preview/render semantic contract
- security boundary
- deployment model
- external infrastructure requirement

Implementation convenience alone is not sufficient reason to bypass an architecture boundary.

---

## 31. Architecture Summary

**Architecture:** desktop-first modular monolith.  
**Runtime:** Tauri 2.12.x + React/TypeScript + Rust.  
**Core:** application-owned versioned Project Domain Model + single Command Service.  
**Time:** integer/rational canonical timeline time.  
**Persistence:** atomic save, recovery snapshots, explicit migrations.  
**Media:** external originals + rebuildable derivatives.  
**Jobs:** bounded local Job Manager.  
**Preview:** backend abstraction; final choice gated by POC.  
**Export:** normalized composition plan → deterministic backend, initially FFmpeg-centered.  
**Automation:** inspect → diff/dry-run → transactional apply through the same Command Service.  
**Security:** least privilege; no raw shell/file authority from UI or agent.  
**Scaling:** local-first; no Redis/microservices until a real distributed requirement exists.
