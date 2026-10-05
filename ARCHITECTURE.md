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

Automation adapters are external transports only, operating strictly as clients of the editor rather than a second editor engine.

- **Initial transport**: CLI-first (`tinixcut inspect`, `diff`, `apply`, `render`).
- **Machine session**: Long-lived JSONL stdio session (`tinixcut session` / `--stdio`), maintaining in-memory `CommandService` and `JobManager` to eliminate repeated process startup and project re-parsing overhead.
- **Protocol streams**: Standard output (`stdout`) is strictly reserved for machine-readable protocol frames (JSONL or MCP); standard error (`stderr`) is reserved exclusively for diagnostic and logging output.
- **MCP**: Model Context Protocol (MCP) server support is governed by ADR-005; MCP serves as a thin local stdio transport adapter (`tinixcut mcp`, primary protocol `2026-07-28`, Tools-only) over the identical underlying `CommandService`, `JobManager`, and persistence APIs. Remote HTTP/OAuth is deferred.

They may:

- inspect project state and discover capability schemas
- request dry-run/diff validation
- apply typed transactional command plans
- start, query, and cancel export/render jobs
- read structured receipts and reports

They may not:

- mutate project files directly (all mutations must pass through `CommandService`)
- construct unrestricted shell commands or access arbitrary filesystems
- bypass the command registry or validation rules
- maintain a parallel timeline reducer or divergent mutation path

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
├── client_registry (ClientRegistry tracking monotonic sequence watermarks; max 64 clients; ADR-001)
├── command_ledger[] (active receipt window; K=5,000; ADR-001)
├── markers / metadata
└── supported project-owned extension data
```

The `client_registry` and `command_ledger` represent canonical correctness state owned by the Project domain model and persisted directly into `.tcproj` (structured into a bounded client registry tracking monotonic sequences per client in $O(1)$ space and an active receipt window $K=5,000$, capping total idempotency document overhead to $\le 1.65 \text{ MB}$; see ADR-001). Durable idempotency is guaranteed for Structured Operation Identities (`client_id:client_epoch:sequence_number`), mathematically proving that committed operations can never become fresh, while individual tombstone arrays are eliminated in favor of constant-sized sequence watermarks. It is not disposable or rebuildable cache; clearing derived caches must never delete replay protection. Idempotency receipts survive process lifetimes and canonical save/reopen cycles.

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
- `transitions`: track-level cut-transition collection (`Vec<CutTransition>` in Schema v2)

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
- single-clip fades (`fade_in`, `fade_out`, and item-level `FadeIn`/`FadeOut` variants) for head/tail clip opacity ramps; adjacent-clip transitions are strictly cut-owned and stored on `Track.transitions` (item-level fade semantics are preserved losslessly)
- keyframes only when formally supported

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

A single representation was selected during Phase 1 and is used consistently: `RationalTime` (`{ value: i64, rate: u64 }`).

Seconds, milliseconds, SMPTE-like display, and frame numbers are boundary/display representations only.

#### Representation Equality vs. Semantic Timeline-Time Invariant

A durable architectural invariant discovered and enforced during Phase 13:

1. **Exact Representation Equality**: `RationalTime` derived `PartialEq/Eq` represents exact stored `(value, rate)` representation equality. This is critical for exact round-trip serialization, hashing, deterministic ledger deduplication, and structural diffs.
2. **Semantic Comparison Rule**: Semantic timeline-time equality **MUST NOT** rely on raw `==` / `!=` when different rates can represent the same instant (e.g. `{ value: 100, rate: 1000 }` and `{ value: 3, rate: 30 }` both represent $0.1\,\text{s}$, but raw `PartialEq` evaluates to `false`).
3. **Canonical Semantic Helpers**: Semantic comparison must use canonical domain helpers:
   - `RationalTime::is_same_time` / `semantic_eq` for point-in-time equivalence across different rates;
   - `TimeRange::is_adjacent_to` for rate-independent timeline boundary adjacency;
   - `TimeRange::is_adjacent_to_with_rate` where project-frame tick alignment is required.
4. **Rescaling & Arithmetic**: Timeline duration, ordering, and arithmetic must normalize/rescale to a common rate appropriately rather than compare raw `.value` across mismatched rates.
5. **Preservation of Global Semantics**: Global `PartialEq/Eq` semantics on `RationalTime` remain exact representation equality by design and are NOT altered.

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
- **Schema v2 (ADR-003)**: Introduces track-level `CutTransition` collection (`Track.transitions: Vec<CutTransition>`). Production-supported clip-owned Fade semantics currently on `TimelineItem` (`fade_in`, `fade_out`, and item-level `FadeIn`/`FadeOut` variants) are retained and must NOT be cleared, dropped, or re-homed. Existing v1 item-level Fade behavior survives v1→v2 migration losslessly. Migration initializes `Track.transitions: Vec::new()`. No `CrossDissolve` is invented from v1. Malformed v1 containing unsupported `CrossDissolve` placeholder is rejected explicitly per migration/error policy; never silently activated. `TimelineItem.transition` is not deprecated in any manner that implies field removal in Phase 13 while existing supported Fade variants still depend on it.
- **Schema v3 (ADR-004)**: Introduces keyframe `outgoing_easing: KeyframeEasing` on `Keyframe` (`Linear`, `EaseIn`, `EaseOut`, `EaseInOut`, `CubicBezier`). Schema v3 is implemented and current in production. Promoting to `schema_version = 3` protects projects from silent loss of easing data when opened in older Schema v2 binaries. Forward migration from v2 to v3 is lossless: all v2 fields, `CutTransition` on tracks, item-level Fades, and keyframe values/offsets are preserved identically; every migrated keyframe is initialized with `outgoing_easing = Linear`. Old readers reject v3 with an explicit unsupported schema version error.

### 8.10 Cut-Owned Transition Model (ADR-003)

Transitions between clips adhere to strict domain invariants:

1. **Cut-Owned Topology**: CrossDissolve is owned by the cut boundary $C$ between two immediately adjacent video clips on the same track (`left.timeline_range.end() == right.timeline_range.start == C`), not by an individual clip.
2. **Timeline Duration Invariance**: Adding, modifying, or removing a CutTransition strictly preserves total track and project duration ($\Delta T_{\text{project}} = 0$). Canonical timeline does not persist overlap; clips remain abutted edge-to-edge.
3. **Exact RationalTime Odd-Duration Rule**: Transition duration and cut offsets use exact `RationalTime` (`{ value, rate }`). Rescaling $D$ to project timeline rate gives $D_{\text{ticks}}$. Tick division is exact: $L_{\text{ticks}} = \lfloor D_{\text{ticks}} / 2 \rfloor$, $R_{\text{ticks}} = D_{\text{ticks}} - L_{\text{ticks}}$. The cut window boundaries are $T_0 = C - L$ and $T_1 = C + R$. The transition interval is strictly half-open: $[T_0, T_1)$. Phase 13 CrossDissolve provides centered cut semantics exclusively; no persisted `CutAlignment` field is added in Schema v2 (future alignment extensibility is deferred).
4. **Mandatory Provable Handles & Editorial Extents**:
   - Outgoing left clip: postroll handle $\ge R$ and outgoing editorial extent $\ge L$.
   - Incoming right clip: preroll handle $\ge L$ and incoming editorial extent $\ge R$.
   - Available handles must be verified against source media asset bounds prior to transition creation.
5. **Stable Machine-Readable Error Contract**:
   Transition validation produces stable error codes:
   - `INSUFFICIENT_TRANSITION_HANDLE`: Source duration is known, but available postroll handle $< R$, available preroll handle $< L$, or editorial extent is insufficient.
   - `TRANSITION_HANDLE_UNVERIFIABLE`: Media duration or stream handle availability cannot be proven (e.g. unprobed media, stream duration missing, or asset unavailable).
   - `UNSUPPORTED_TRANSITION_MEDIA`: Participant is an image, synthetic graphic, text, or non-video item (Phase 13 supports video/video cuts exclusively).
   The engine strictly rejects invalid requests and never silently clamps duration, auto-shortens the transition, or synthesizes freeze-frames.

### 8.11 Canonical Keyframe Easing, Addressing & Lifecycle (ADR-004)

Keyframes adhere to strict domain ownership, mathematical precision, and clip lifecycle invariants:

1. **Clip-Local Ownership**: Keyframes belong strictly to `TimelineItem.keyframes`. No stable keyframe IDs are introduced in Phase 14; keyframes are addressed exclusively by `(property, semantic RationalTime offset)`.
2. **Supported Channels**: Exactly 7 properties are keyframe-capable: `PositionX`, `PositionY`, `ScaleX`, `ScaleY`, `Rotation`, `Opacity`, `Volume`. The same canonical scalar evaluator evaluates all channels. Rotation interpolates numerically in degrees without shortest-path angle wrapping. No implicit clamping occurs in the interpolation layer.
3. **Semantic RationalTime Ordering & Equality**: Semantic time comparison MUST evaluate cross-multiplied equality ($v_1 \cdot r_2 == v_2 \cdot r_1$) rather than comparing raw `.offset.value`. Global `RationalTime PartialEq/Eq` remains representation equality. Keyframes with equivalent rational offsets address the same keyframe.
4. **Canonical 32-Iteration Bisection Solver**:
   - A fixed 32-iteration bisection loop in Rust/core solves $x(q) = u$ for parameter $q \in [0, 1]$, then evaluates $e = y(q)$.
   - Control points $(x_1, y_1, x_2, y_2) \in [0.0, 1.0]^4$ are finite `f64`. No overshoot is permitted in Phase 14.
   - Endpoints $u \le 0$ and $u \ge 1$ bypass the solver and return exact keyframe values.
   - No browser CSS, renderer, or FFmpeg solver is canonical truth.
5. **Split Subdivision Semantics**:
   - At split offset $S$, keyframes with $K < S$ stay left; $K > S$ migrate right rebased by $-S$; $K == S$ duplicates onto both fragments (left outgoing Linear, right inherits easing).
   - When a split crosses an interpolation segment ($K_i < S < K_{i+1}$), boundary value $V(S)$ is evaluated via the canonical solver, boundary keys are synthesized, and the easing curve is subdivided via solver parameter $q_S$ and de Casteljau's algorithm, normalizing subcurve control points to $[0, 1]^2$. Presets become `CubicBezier`.
   - Left fragment preserves original `item_id`; right fragment receives newly generated `item_id`.
6. **Trim Semantics & Hidden Keyframes**:
   - Tail trim shortens duration; keyframes beyond new duration persist as hidden state. Extending tail restores them.
   - Head trim by $H$ shifts all keyframe offsets by $-H$. Negative keyframe offsets are valid persisted domain state (hidden preroll keyframes). Extending head shifts offsets back by $+H$, preserving visible animation alignment with source media.
   - Trim and keyframe offset updates execute atomically in a single command transaction. Silent deletion of keyframes during trim is prohibited.

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

### 9.4 Idempotency and Durable Command Ledger (ADR-001)

Idempotency guarantees that retrying an operation or transaction identity never duplicates mutations or state transitions:

```text
inspect -> propose -> dry-run -> apply -> receipt
successful apply
  -> durable command ledger entry (`CommandLedgerEntry`)
  -> canonical save (`save_project_atomic`)
  -> reopen (`load_project`)
  -> exact replay recognized
  -> prior receipt returned (`deduplicated_noop`)
  -> no duplicate mutation / no revision bump
```

- **Project-owned durable ledger**: Successful operations persist entries in the project's canonical `command_ledger`. The ledger is part of the project document (`.tcproj`), not transient transport or session memory, surviving canonical save/reopen across process lifetimes.
- **Cache independence**: The command ledger is canonical correctness state, not disposable/rebuildable cache. Deleting derivative cache does not remove replay protection.
- **Staged transaction isolation**: Pre-commit transaction staging simulates only the editorial timeline state (`tracks`, `assets`, `settings`), isolating the command ledger from expensive transaction cloning and guaranteeing sub-microsecond in-memory staging clone latency (verified at 601 nanoseconds on reference hardware, algorithmic complexity bounded to $O(\text{editorial items})$).
- **Structured operation identity authority**: Durable idempotency across unbounded horizons is governed exclusively by **Structured Scoped Identity** (`client_id:client_epoch:sequence_number`), where sequence numbers are strictly monotonic integers per registered client. By information-theoretic proof, arbitrary unscoped string IDs cannot guarantee durable idempotency across unbounded horizons within bounded storage.
- **Mathematical proof of monotonic sequence correctness**: For any registered client, the committed history is a contiguous sequence prefix $\{1, \dots, W(c)\}$. Storing all historical committed operations requires exactly one integer $W(c)$ (8 bytes). For any incoming sequence $s \le W(c)$, the engine proves with $100\%$ certainty that the operation was already committed, preventing duplicate execution even if `base_revision` is refreshed. Individual operation tombstones are completely eliminated.
- **Client lifecycle & bounded capacity contract (`ClientRegistry`)**:
  - The project maintains a bounded client registry (`max_active_clients = 64`), capping client tracking overhead to $\le 3.5 \text{ KB}$ in `.tcproj`.
  - Clients register or use well-known identifiers (`"gui"`, `"cli"`, `"session"`). When registry capacity is reached, unregistered requests are rejected (`CLIENT_REGISTRY_FULL`).
  - Inactive clients can be retired by advancing `min_accepted_epoch`. Any command presenting an expired epoch is rejected with `CLIENT_EPOCH_EXPIRED` via an $O(1)$ scalar check.
- **Legacy & unscoped operation ID semantics**: Bare string IDs (e.g. legacy Phase 10 `"op_xxx"`, UUIDs) without `client_id:seq` are treated as **Ephemeral Window Operations**. Deduplication is guaranteed strictly within the Active Window ($K = 5,000$). Evicted unscoped IDs are expired; durable long-term idempotency is reserved for structured identities. Automation CLI and stdio tools automatically wrap operations with structured client identities.
- **Direct empirical benchmark validation ($K = 5,000$)**: Direct empirical benchmarks on reference hardware (Windows 11, NVMe) validate the 5-point performance budget: 1.635 MB project file overhead, 3.11 MB `CommandService` RAM, 601 ns staging clone, 8.5–22.7 µs deduplicated lookup, ~6.5 ms autosave write, and ~24.0 ms project load (with disk latencies subject to standard OS scheduling and filesystem sync conditions).
- **Cryptographic fingerprint baseline**: Operation payload semantic equality is established using standard SHA-256 (`fp_sha256_<64hex>`) on canonical serialized payload bytes, providing $\sim 2^{128}$ collision resistance (Birthday bound) and $\sim 2^{256}$ preimage resistance.
- **Conservative legacy FNV migration protocol**: Stored Phase 10 entries (`fp_<16hex>`) are treated as unverified legacy receipts. Because FNV-1a lacks cryptographic collision resistance and raw payloads are not retained, replay attempts against legacy entries are strictly rejected with `LEGACY_RECEIPT_UNVERIFIABLE`. The command is NOT executed as fresh (zero project mutation, no revision increment), eliminating adversarial collision spoofing.
- **Exact replay semantics**: When an operation with an already-committed `operationId` and identical semantic payload fingerprint is submitted within the retention horizon, the Command Service returns or reconstructs the prior `CommandReceipt` with zero operations applied, no revision increment, and no new history entry.
- **Collision rejection**: Submitting an existing `operationId` with a different semantic payload fingerprint is rejected explicitly with `IdempotencyCollision` (`IDEMPOTENCY_COLLISION`).
- **Atomic transaction isolation**: A failed transaction persists no entries to the command ledger, ensuring all-or-nothing ledger integrity.
- **Dry-run isolation**: Dry-run validations do not append to or alter the command ledger.
- **Transport reuse**: Future automation transports (such as MCP) inherit these exact durable idempotency guarantees via the shared `CommandService`.

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

### 14.4 Normalized Two-Input TransitionPlan & Premultiplied Parity Semantics (ADR-003)

For adjacent-cut CrossDissolve transitions:

1. **Two-Input Transition Segment**:
   The half-open transition interval $[T_0, T_1)$ (where $T_0 = C - L, T_1 = C + R$ per the exact RationalTime odd-duration rule) is normalized into an explicit `TransitionPlan` segment referencing:
   - `left_item_id` with evaluated source time $S_A(t) = \text{source\_start}_A + (t - \text{timeline\_start}_A)$
   - `right_item_id` with evaluated source time $S_B(t) = \text{source\_start}_B - (C - t)$
   - Normalized progress $p(t) = (t - T_0) / D \in [0.0, 1.0)$.

2. **Canonical Premultiplied RGBA Blend Invariant**:
   Dual backends must strictly evaluate the blend in premultiplied linear/sRGB space:
   $$C_{\text{blend}}^* = (1 - p) C_A^* + p C_B^*$$
   $$\alpha_{\text{blend}} = (1 - p) \alpha_A + p \alpha_B$$

3. **Prohibition of Ordinary Source-Over Opacity**:
   Standard two-layer `source-over` opacity compositing ($\text{Layer}_B$ over $\text{Layer}_A$) produces an unphysical non-linear alpha drop ($\Delta \alpha = p(1 - p)\alpha_A\alpha_B$, resulting in a $25\%$ transparency dip at $p = 0.5$). Ordinary two-layer `source-over` opacity is strictly forbidden for CrossDissolve across both preview and export engines.

### 14.5 Normalized Keyframe Animation & ExportAutomationSchedule (ADR-004)

Keyframe animation adheres strictly to a decoupled evaluation pattern (implemented in Phase 14):
1. **Canonical Evaluator**: Project keyframes are evaluated in Rust/core at the target timeline frame or sample rate using the implemented 32-iteration bisection cubic Bézier solver.
2. **Derived Schedule**: Rust compiles an ephemeral, in-memory `ExportAutomationSchedule` containing pre-calculated literal property values for video frames and audio sample intervals.
3. **Zero Formula Leakage**: `ExportAutomationSchedule` is a build-time execution artifact, never persisted in project JSON.
4. **Backend Parity**: Preview (DOM / React) and export (FFmpeg) both consume canonical scalar evaluations from the same Rust evaluator, guaranteeing zero curve drift.

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

### 15.5 Preview TransitionGroup Architecture (ADR-003)

In the preview backend (WebView2 / DOM):
- Authoritative preview path is an explicit `PreviewTransitionGroup` container (`<div class="preview-transition-group">`) forming an isolated DOM stacking/compositing group for the transition interval $[T_0, T_1)$.
- Left and right layers render weighted A/B contributions with handle-extended frames, opacity modulated by $1 - p$ and $p$.
- The two layers are composited additively via `mix-blend-mode: plus-lighter` (additive premultiplied mixing) strictly inside the group, completely eliminating the non-linear opacity hole.
- The blended group output is then composited via standard `source-over` onto the normal stack.
- Canvas/offscreen compositor is NOT an authorized alternative Phase 13 implementation.
- Silent fallback to ordinary `source-over` is strictly prohibited.

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

### 16.5 Export Filtergraph Transition Mapping (ADR-003)

In the export engine (FFmpeg filtergraph), the authoritative sequence is:
`normalized full-canvas RGBA A/B`
$\to$ `premultiply`
$\to$ bounded aligned `xfade=transition=fade`
$\to$ `unpremultiply`
$\to$ one transition-result contribution
$\to$ existing overlay chain.

- Adjacent-cut CrossDissolve segments $[T_0, T_1)$ are extracted with their required preroll/postroll handle trims.
- Inside $[T_0, T_1)$, ordinary unblended A and B contributions on the track must be strictly suppressed.
- The resulting unpremultiplied RGBA stream represents the single contribution for that track interval in the existing overlay chain.
- Video-only for Phase 13; audio cut transitions remain exact butt-cuts (audio crossfade is deferred).
- Transition easing curves and spectral waveform enhancements remain deferred; keyframe Bézier interpolation is resolved by ADR-004.

### 16.6 Full Export Keyframe Automation via Literal Commands and Audio Envelopes (ADR-004)

ADR-004 and `ExportAutomationSchedule` are implemented and verified in Phase 14:

1. **Video Runtime Command Dispatch**:
   - Rust samples animated visual properties (`PositionX`, `PositionY`, `ScaleX`, `ScaleY`, `Rotation`, `Opacity`) at each output frame RationalTime grid ($t_n = n / \text{fps}$).
   - Initial values at frame 0 are embedded as static initial filter options.
   - For frames $n \ge 1$, runtime command scripts (`sendcmd`) dispatch literal values at frame midpoints $t_{\text{mid}} = (t_{n-1} + t_n) / 2$.
   - Literal filter mappings:
     - `PositionX`, `PositionY` $\to$ `overlay` filter commands (`x`, `y`).
     - `ScaleX`, `ScaleY` $\to$ `scale` filter commands (`w`, `h`).
     - `Rotation` $\to$ `rotate` filter commands (`a` in radians).
     - `Opacity` $\to$ `colorchannelmixer` filter commands (`aa`).
   - Zero interpolation or Bézier math executes in FFmpeg filter expressions.
2. **Audio Sample-Accurate Volume Envelope**:
   - Volume keyframes are evaluated across the exact audio sample rate grid using the canonical Rust evaluator.
   - The audio sample-rate contract derives from the export profile; currently supported profile rates are 48,000 Hz and 44,100 Hz (arbitrary additional sample rates remain unsupported unless separately planned).
   - Rust streams an ephemeral mono 32-bit floating point PCM (`f32le`) binary gain envelope file in $O(1)$ memory buffer chunks.
   - The filtergraph applies volume modulation via the `amultiply` filter.
   - Preflight disk space verification is required before generation ($\approx 691.2 \text{ MB}$ for 60 min at 48kHz). The file is reliably cleaned up on export completion, failure, or cancellation.
3. **ADR-003 Transition Branch Coexistence**:
   - Runtime animation commands target ordinary clip filter branches outside $[T_0, T_1)$, and target transition participant branches inside $[T_0, T_1)$ prior to the premultiplied `xfade` stage.
   - Preserves ADR-003 adjacent-cut topology, zero persisted overlap, duration invariance, and premultiplied blend invariant $(1-p)A^* + pB^*$. ADR-003 transition compositing remains unchanged.

---

## 17. Preview / Export Parity Contract

For every feature considered production-supported, define canonical semantics and parity tolerance.

### 17.1 Canonical Transform & Geometry Pipeline

Both the preview backend (WebView2) and the export backend (FFmpeg) strictly consume the same normalized composition semantics:

1. **Coordinate System**:
   - Canvas has dimensions $W \times H$.
   - Coordinate origin $(0, 0)$ is the canvas center.
   - Position offset: `(position_x, position_y)` in canvas pixels.
   - Layer center on canvas is at $(\frac{W}{2} + \text{position\_x}, \frac{H}{2} + \text{position\_y})$.

2. **Transform Evaluation Order**:
   $$\text{Source Media} \longrightarrow \text{Crop Insets} \longrightarrow \text{Scale} \longrightarrow \text{Rotate} \longrightarrow \text{Translate/Center} \longrightarrow \text{Opacity}$$
   - **Crop**: Normalized fractional insets from source edges (`crop.left`, `crop.right`, `crop.top`, `crop.bottom`).
   - **Scale**: Multipliers `(scale_x, scale_y)` applied to the cropped media dimensions.
   - **Rotate**: Rotation around layer center by `rotation` degrees.
   - **Position**: Overlay placed such that the layer's center is at $(\frac{W}{2} + \text{pos\_x}, \frac{H}{2} + \text{pos\_y})$. In FFmpeg, this is evaluated as `overlay=x='(W-w)/2+pos_x':y='(H-h)/2+pos_y'`.
   - **Opacity**: Alpha multiplier applied via CSS opacity in preview and `colorchannelmixer=aa=opacity` in export.

3. **Timeline Boundary Semantics**:
   - Timeline intervals are strictly half-open $[start, end)$.
   - An item is active at timeline time $t$ if $start \le t < end$.
   - The export filtergraph strictly enforces this half-open interval using `enable='between(t, start, end - 0.0005)'` so that boundary frames do not overlap adjacent clips.

### 17.2 POC 2 Parity Suite Verification

Preview and export parity is validated across 12 deterministic test fixtures (P7-F01 through P7-F12):
- **Level 1 (Semantic Parity)**: Exact equality of normalized layer snapshots and source timestamps.
- **Level 2 (Geometry/Timing Parity)**: Bounding rectangles match within $< 0.1$px tolerance; layer activation matches exactly.
- **Level 3 (Raster Diff)**: Pixel deltas remain within documented tolerances (MAE $\le 4.0$, Exceeded Ratio $\le 3\%$ for video; MAE $\le 10.0$ for text glyph rasterization).
- **Gate Sensitivity (Negative Control)**: An intentional offset (e.g. $+120$px position shift) is verified to fail the parity gate, ensuring the gate cannot pass trivially.

A feature must not be considered fully supported until parity tests exist at an appropriate level.

---

## 18. Automation Architecture

### 18.1 Automation is a client, not a second editor

Automation is an external client adapter to the editor. It does not contain an independent editing engine, does not manipulate project files directly, and does not maintain a second project reducer.

All mutations route strictly through the authoritative `CommandService`, and all background tasks route through the local `JobManager`.

Automation execution loop:

```text
inspect -> propose plan -> diff/dry-run -> apply -> receipt
successful apply
  -> durable command ledger entry (`CommandLedgerEntry`)
  -> canonical save (`save_project_atomic`)
  -> reopen (`load_project`)
  -> exact replay recognized
  -> no duplicate mutation
```

### 18.2 Transport and Session Model

```text
Manual UI
CLI / JSONL
MCP stdio
      │
      ▼
existing application/automation service seam
      │
CommandService / JobManager / Persistence
```

- **Initial transport: CLI-first**. A standalone CLI entrypoint exposes headless commands (`inspect`, `diff`, `apply`, `render`) for one-shot execution without UI dependencies.
- **Machine session: long-lived JSONL stdio (Phase 10)**. Automation supports a long-lived interactive session (`tinixcut session` / `--stdio`) communicating via line-delimited JSON (JSONL). The session maintains in-memory `CommandService` and `JobManager` instances, avoiding the overhead of re-spawning processes and re-parsing `.tcproj` files for sequential edits.
- **Protocol streams**: Standard output (`stdout`) is strictly reserved for protocol frames (JSONL responses or MCP JSON-RPC frames). Standard error (`stderr`) is strictly reserved for diagnostic logs, warnings, and error traces.
- **MCP local stdio adapter (Phase 15, ADR-005 — Implemented)**:
  - **Implemented Automation Transport**: The MCP local stdio adapter is implemented; `tinixcut mcp` is a supported headless automation entrypoint before GUI startup.
  - **Transport Only**: MCP is strictly an external transport adapter; it introduces no second reducer, no second command registry, no second persistence format, and no project mutation bypass.
  - **Primary Protocol**: MCP `2026-07-28` is the primary protocol target with modern stateless lifecycle (no `initialize` / `notifications/initialized`, stateless protocol core, discovery via `server/discover`, and SDK-only `2025-11-25` fallback).
  - **Tools-Only Primitive Scope**: MCP remains Tools-only, exposing exactly eight (8) authoritative tools (`tinixcut_inspect`, `tinixcut_capabilities`, `tinixcut_dry_run`, `tinixcut_apply`, `tinixcut_save`, `tinixcut_export_start`, `tinixcut_export_status`, `tinixcut_export_cancel`). Resources, Prompts, Tasks extension, MCP Apps, Sampling, Roots, Subscriptions, and MRTR are not in scope.
  - **Explicit Project Addressing**: Explicit `projectPath` addressing is implemented for all project-scoped tools; no hidden MCP project session, global active project, or project-handle manager exists.
  - **Authoritative Apply**: Authoritative mutations (`tinixcut_apply`) route through the canonical `CommandService` and commit durably to disk via atomic persistence (`save_project_atomic`) before returning success. No hidden unsaved state exists across MCP calls.
  - **Process-Local Write Serialization**: One process-local async authoritative-write gate is shared by the MCP server; Phase-15 authoritative writes serialize across all project paths to prevent concurrent read-modify-write races. Per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists.
  - **Durable Identity**: Authoritative `tinixcut_apply` requires caller-supplied `(client_id, client_epoch, sequence_number)` per ADR-001 v3. Server never auto-generates sequences on retry, preventing duplicate edits on lost-response retries.
  - **Export Lifecycle**: Existing `JobManager` backs the export lifecycle (`tinixcut_export_start`, `tinixcut_export_status`, `tinixcut_export_cancel`) with snapshot isolation, process-local `jobId` handles, and graceful cancellation on stdin EOF.
  - **Schema Stability**: Project schema remains Schema v3 unchanged.
  - **Stdout Protocol Purity**: Subprocess stdio transport guarantees 100% stdout protocol cleanliness; all diagnostics, media resolver messages, and panic hooks route to `stderr`.
  - **Deferred Scope**: Remote Streamable HTTP, OAuth 2.0 / authorization servers, bearer-token infrastructure, and remote hosting remain deferred.

### 18.3 Initial automation capabilities

The automation surface exposes only reviewed domain operations and queries:

- **Project inspection (`project.inspect`)**: Project metadata, tracks, items, assets, revision, timebase, duration.
- **Capability discovery (`project.capabilities`)**: Supported operations, transition types, export codecs, protocol version.
- **Dry-run validation (`command.dryRun`)**: Evaluates operations against current revision, returning structured changes/warnings without mutating project state.
- **Transactional apply (`command.apply`)**: Executes atomic command envelopes with base revision enforcement and durable idempotency.
- **Export job lifecycle (`export.start`, `export.status`, `export.cancel`)**: Snapshot-isolated asynchronous render jobs through the shared Job Manager.
- **Session management (`session.open`, `session.close`, `session.shutdown`, `project.save`)**: Atomic lifecycle controls.

### 18.4 Capability model and security boundaries

Automation operates under least privilege and must not receive:

- unrestricted filesystem enumeration or access outside approved project boundaries
- arbitrary shell command construction or execution
- arbitrary native process spawning
- direct project-file write bypasses (all saves pass through atomic persistence validation)

### 18.5 Audit, receipts, and durable idempotency

Automation apply returns structured receipts (`CommandReceipt`) including:

- transaction and operation IDs
- project revision before and after
- applied vs. `deduplicated_noop` status
- errors, warnings, and affected entity IDs
- durable persistence into `Project.command_ledger` preserving replay deduplication across save/reopen cycles

### 18.6 UI and automation equivalence

If an edit can be performed manually in the UI and is exposed to automation, both paths invoke the exact same underlying `CommandService` operations and validate against the exact same domain invariants.

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

## 21. FFmpeg Distribution, Process Isolation, and Packaging Boundary (ADR-002)

TinixCut is designed and scoped for **personal use (non-commercial / non-public distribution)**.

1. **Process Isolation Boundary**:
   - FFmpeg and ffprobe execute strictly as independent OS child processes via `std::process::Command` with typed arguments.
   - Core application logic never statically or dynamically links to `libavcodec`/`libavformat`.
2. **Distribution & Version Pinning**:
   - Pinned distribution: **FFmpeg 9.0.2 Gyan Release Essentials** (`ffmpeg-9.0.2-essentials_build.zip`, x86_64 Windows).
   - Artifact provenance (version, upstream URL, SHA-256 digests) is tracked in `ffmpeg_provenance.json` for technical reproducibility and debugging.
   - Legal/commercial distribution reviews, GPL/LGPL EULA declarations, and patent licensing are explicitly **OUT OF SCOPE** for personal-use builds.
3. **Packaging Mechanism (Tauri `bundle.resources`)**:
   - Packaged builds bundle `ffmpeg.exe` and `ffprobe.exe` via Tauri `bundle.resources: ["resources/bin/*"]`.
   - `MediaBinaryResolver` resolves binaries with strict priority:
     - Packaged production: must resolve app-owned pinned binary from resource directory; fails if missing.
     - System `PATH`: strictly forbidden in packaged production; permitted only in explicit dev/test mode (`cfg!(debug_assertions)` or `TINIX_DEV_ALLOW_SYSTEM_PATH=1`).
4. **WebView2 Strategy**:
   - NSIS Windows installer uses `downloadBootstrapper` (minimal installer size; leverages system-wide Evergreen WebView2 preinstalled on Windows 10/11).
5. **Hardware Fallback Strategy**:
   - Baseline reference: `libx264` software encoding.
   - Hardware acceleration: `h264_nvenc`, `h264_qsv`, `h264_amf`.
   - Automatic fallback: If hardware acceleration fails due to driver/device unavailability, the export automatically falls back to `libx264`. Non-hardware errors fail immediately without retry.

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

### Gate F — FFmpeg distribution/hardware fallback (ADR-002)

**When:** before packaging/release qualification.  
**Pass condition:** POC 5 (Pinned FFmpeg 9.0.2 Gyan Release Essentials bundled via Tauri `bundle.resources`; deterministic resolution via `MediaBinaryResolver` with packaged production isolation; hardware fallback to `libx264` validated; installer generated via NSIS with `downloadBootstrapper`; personal-use build scope).

---

## 26. Architecture Invariants

These rules are considered non-negotiable unless `ARCHITECTURE.md` is explicitly revised with rationale.

1. Canonical project state is application-owned and versioned.
2. UI and automation use one authoritative Command Service.
3. Stable entity IDs are used; array position is not identity.
4. Canonical time is integer/rational (`RationalTime`), not arbitrary float seconds. Derived `PartialEq/Eq` represents exact stored representation equality; semantic timeline-time equality across rates must use canonical semantic helpers (`is_same_time`/`semantic_eq`, `is_adjacent_to`, `is_adjacent_to_with_rate`) and appropriate rate rescaling rather than raw equality or raw `.value` comparison.
5. Failed atomic mutations leave project state unchanged.
6. Stale automation revision is rejected explicitly.
7. Retried idempotent operation cannot duplicate mutation; durable automation idempotency is project-owned (`command_ledger` active window bounded to $K=5,000$ operations plus bounded `client_registry` $C_{\max}=64$; ADR-001) and survives canonical save/reopen across process lifetimes; durable idempotency requires Structured Operation Identity (`client_id:client_epoch:sequence_number`), mathematically proving that committed operations can never become fresh even under refreshed revisions (`OPERATION_ALREADY_COMMITTED`); operation fingerprints use cryptographic digest (SHA-256 baseline with $\sim 2^{128}$ collision resistance and $\sim 2^{256}$ preimage resistance); replay against unverified legacy FNV-1a entries is conservatively rejected (`LEGACY_RECEIPT_UNVERIFIABLE`) without fresh execution; transport-local memory is not the source of truth for committed operation receipts.
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
4. Exact initial transition/keyframe subset (Resolved for Phase 13 CrossDissolve via ADR-003: Video CrossDissolve on adjacent cuts via track-level CutTransition, Schema v2, RationalTime, provable handles, and premultiplied RGBA blend. Resolved for Phase 14 Keyframe Easing via ADR-004: Schema v3 clip-local keyframes, canonical Rust 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle, and ExportAutomationSchedule. Audio crossfade, transition easing, and generalized transition frameworks remain open/deferred).
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
**Automation:** CLI-first long-lived JSONL stdio and local stdio MCP adapter (ADR-005) over shared Command Service/Job Manager; durable project-owned idempotency surviving save/reopen with bounded retention horizon and SHA-256 fingerprint baseline (ADR-001); remote MCP/HTTP deferred.  
**Security:** least privilege; no raw shell/file authority from UI or agent.  
**Scaling:** local-first; no Redis/microservices until a real distributed requirement exists.
