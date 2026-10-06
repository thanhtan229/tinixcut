# PROJECT_STATUS.md

> **Current Project Execution Snapshot**
>
> This document records the current durable state of the project so another coding agent or a future session can resume quickly.
>
> It is a snapshot, not a chronological development log.

---

## 0. Status Metadata

**Project:** Automation-First Desktop Video Editor  
**Overall Status:** PHASES 0–15 COMPLETE, PHASE 16 COMPLETE WITH DEFERRED ITEMS, PHASE 17 RESEARCH GATE COMPLETE  
**Current Selected Milestone:** Phase 17 — Performance Requalification & Release Foundation (Research Gate Complete)  
**Current Active Implementation Phase:** NONE  
**Phase 16 Status:** COMPLETE WITH DEFERRED ITEMS  
**Deferred Acceptance Qualification:** P16-AC-56 (Real interactive Windows GUI qualification deferred), True Clean-VM qualification deferred  
**Implementation:** Phases 0–16 COMPLETE; Phase 17 Implementation NOT STARTED  
**ADR:** No new ADR required (ADR-006 NOT required)  
**Plan Status:** Phase 17 Research Gate Reconciled; implementation plan to be authored in `Plans/plan_phase17.md`  
**Project Schema:** Schema v3 unchanged  
**Phase 17:** RESEARCH GATE COMPLETE — READY FOR PLANNING (Implementation: NOT STARTED)  
**Baseline Commit:** `95a9f16` (`docs: reconcile phase 16 completion with deferred GUI QA`)  
**Next Action:** ChatGPT authors plan_phase17.md using reconciled research state.  
**Last Updated:** 2026-10-06  

---

## 1. Current Objective

Phase 17 Research Gate is COMPLETE — READY FOR PLANNING; implementation is NOT STARTED:
- **Phases 0–15 Final State**: `COMPLETE`.
- **Phase 16 Status**: `COMPLETE WITH DEFERRED ITEMS` (`P16-AC-56` real interactive Windows GUI QA deferred).
- **Phase 17 Milestone**: `Phase 17 — Performance Requalification & Release Foundation`.
- **Phase 17 Research Status**: `COMPLETE — READY FOR PLANNING`.
- **Current Active Implementation Phase**: NONE.
- **Phase 17 Implementation Status**: `NOT STARTED`.
- **ADR Decision**: No new ADR required (ADR-006 NOT required; Phase 17 is qualification-first; zero production architecture, schema, or protocol changes).
- **Plan Status**: Reconciled research state in `RESEARCH.md` and roadmap in `PLAN.md`; `Plans/plan_phase17.md` to be authored by ChatGPT.
- **Project Schema**: Schema v3 invariant (no Schema v4).
- **Automated Verification**: PASS (198 Rust tests + 37 frontend tests, clean clippy, fmt, build, typecheck).
- **Deferred External Acceptance Gates**:
  1. `P16-AC-56`: Real interactive Windows GUI qualification deferred (environment constraint; automated tests do not substitute).
  2. `True Clean Windows 10/11 VM Qualification`: Provisioning a sanitized clean virtual machine without dev tools remains an external release gate prior to distribution.
- **Reconciled Research Conclusions**:
  1. Dual-fixture benchmark framework standardized: Fixture A (semantic-rich Schema-v3, 6.0s @ 30fps) and Fixture B (scale Schema-v3, 20 tracks [14 video, 4 audio, 2 text], 200 items, 20 CrossDissolves, 10.0s @ 30fps).
  2. Empirical release baselines verified on reference workstation: Fixture B save ~3.89 ms, load ~2.99 ms, compile ~0.28 ms, sampling ~0.22 ms; Phase-16 commands 49–64 µs; JSONL 18–23 µs; MCP inspect ~0.24 ms, dry-run ~0.24 ms, apply ~12.20 ms; CPU export (preset fast, CRF 23) 720p RTF $\ge 2.20\times$, 1080p RTF $\ge 1.00\times$.
  3. Prior invalid MCP ~0.06 ms measurements disclosed and superseded: root cause was wire parameter `_meta` formatting error returning JSON-RPC `-32602` before tool dispatch. Validated ~12.2 ms OS-stdio measurements durably recorded.
  4. Hybrid timing budget policy (Qualification Ceilings vs Reference-Host Budgets vs Informational Baselines) and CI isolation guarantee (`#[ignore]` for performance benchmarks).
  5. Host-Isolated Workstation qualification contract `CM-01..CM-07` (bundled self-sufficiency, software fallback, unicode paths, read-only media, atomic persistence, user-scope install, uninstall data preservation).
  6. Normative contract `PR-01` through `PR-18` accepted.
- **Next Action**: ChatGPT authors plan_phase17.md using reconciled research state.

---

## 2. Current State Summary

- Phase 0 Repository and Engineering Baseline complete (Tauri 2.12 + React 19 + Rust).
- Phase 1 Project Domain Model v1 and Command Engine complete (Gate A & POC 3 passed).
- Phase 2 Persistence, History, Recovery, and Relink complete (POC 4 passed).
- Phase 3 Desktop Shell and Basic Editor UI complete.
- Phase 4 Media Foundation, Cache, Job Manager, and FFmpeg Service complete.
- Phase 5 Core Timeline Editing Vertical Slice complete.
- Phase 6 Preview Backend POC and Architecture Decision complete (Gate D / POC 1 passed).
- Phase 7 Normalized Composition Plan & Parity complete (Gate E / POC 2 passed).
- Phase 8 Export / Render Foundation complete (snapshot isolation, atomic promotion, cancellation safety).
- Phase 9 Bounded Creator Feature Expansion & Editing UX reconciliation complete (Fade transitions, keyframe volume, waveform job execution; CrossDissolve deferred in Phase 9, resumed in Phase 13).
- Phase 10 Automation Surface complete & reconciled.
- Phase 11 Reliability, Security, and Performance Hardening complete (ADR-001 v3, bounded active window, SHA-256, staged transaction isolation, client registry quota & epoch fencing, queue stress, long-project scalability).
- Phase 12 FFmpeg Distribution, Packaging, and Release Qualification: **COMPLETE WITH DEFERRED ITEMS** (Isolated Workstation Qualified, required `libx264` export qualified; true clean Windows 10/11 VM qualification and hardware acceleration deferred).
- Phase 13 Transition Compositor & CrossDissolve: **COMPLETE** (Schema v2 implemented and current, ADR-003 implemented, cut-owned topology, isolated preview transition group, bounded export premultiply filtergraph, parity fixtures verified; accepted by ChatGPT).
- Phase 14 Advanced Keyframe Interpolation: **COMPLETE** (Schema v3 current, ADR-004 implemented, canonical 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle semantics, ExportAutomationSchedule sendcmd video runtime commands, sample-accurate audio gain envelope, CrossDissolve coexistence, P14-PREREQ-01 resolved; accepted by ChatGPT and ratified by user post-implementation).
- Phase 15 MCP Automation Adapter: **COMPLETE** (ADR-005 implemented, `rmcp` 3.x local stdio transport, MCP `2026-07-28` primary protocol, exact 8-tool surface, explicit `projectPath` addressing, caller-supplied ADR-001 structured identity, atomic apply persistence, process-local write serialization, `JobManager` export lifecycle, structured domain error preservation, spawned subprocess stdout purity verified on Windows; accepted by ChatGPT after implementation and remediation).
- Phase 16 Timeline Editing Productivity: **COMPLETE WITH DEFERRED ITEMS** (Ephemeral Ctrl multi-select & primary selection, exact 8px snapping, atomic ClipGroupMove, single-track bounded ClipRippleDelete, centralized shortcut framework, non-ripple trim, multi-delete, primary-only duplicate, generic CLI/JSONL/MCP transport inheritance, canonical RationalTime semantic helpers; automated test suite 100% green; P16-AC-56 real interactive Windows GUI QA deferred).
- Automated Tests: 235 total automated tests green (198 `cargo test` passed [107 unit + 8 POC + 15 Phase 14 acceptance + 26 Phase 15 acceptance + 12 Phase 16 research POC + 30 Phase 16 timeline productivity acceptance], 37 `vitest` passed across 4 test suites, `cargo clippy` 0 warnings, `cargo fmt` clean, frontend typecheck/build clean, production binary build green).

---

## 3. Completed

- [x] Foundational external technical research completed in `RESEARCH.md`.
- [x] Baseline architecture documented in `ARCHITECTURE.md`.
- [x] Implementation sequence, phases, gates, and POCs documented in `PLAN.md`.
- [x] `AGENTS.md` aligned with the architecture and plan.
- [x] `DEVELOPMENT_WORKFLOW.md` initialized and updated with real commands.
- [x] `UX_UI.md` initialized for compact dark creative workspace.
- [x] Phase 0 Repository and Engineering Baseline complete (Tauri 2.12 + React 19 + Rust).
- [x] Phase 1 Project Domain, Canonical Time, and Command Engine complete (Gate A & POC 3 passed).
- [x] Phase 2 Persistence, History, Recovery, and Media Relink Foundation complete (POC 4 passed).
- [x] Phase 3 Desktop Shell and Basic Editor UI complete.
- [x] Phase 4 Media Foundation, Cache, Job Manager, and FFmpeg Service complete.
- [x] Phase 5 Core Timeline Editing Vertical Slice complete.
- [x] Phase 6 Preview Backend POC and Architecture Decision complete (Gate D / POC 1 passed).
- [x] Phase 7 Normalized Composition Plan & Parity complete (Gate E / POC 2 passed).
- [x] Phase 8 Export / Render Foundation complete.
- [x] Phase 9 Bounded Creator Feature Expansion complete.
- [x] Phase 10 Automation Surface complete & reconciled.
- [x] Phase 11 Reliability, Security, and Performance Hardening complete (ADR-001 v3 implemented & verified).
- [x] Phase 12 FFmpeg Distribution, Packaging, and Release Qualification: **COMPLETE WITH DEFERRED ITEMS** (Isolated Workstation Qualified; True Clean-VM and Hardware Acceleration Deferred).
- [x] Phase 13 Transition Compositor & CrossDissolve: **COMPLETE** (Schema v2, cut-owned CrossDissolve, preview/export parity, verified via 26 Phase-13 tests, 103 Rust tests, 11 Vitest tests; accepted by ChatGPT).
- [x] Phase 14 Advanced Keyframe Interpolation: **COMPLETE** (Schema v3 promoted, ADR-004 implemented, canonical 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle semantics, ExportAutomationSchedule sendcmd video runtime commands, sample-accurate audio gain envelope, CrossDissolve coexistence, P14-PREREQ-01 resolved, 129 Rust tests + 14 Vitest tests passing; accepted by ChatGPT and ratified by user post-implementation).
- [x] Phase 15 MCP Automation Adapter: **COMPLETE** (ADR-005 implemented & verified, `rmcp` 3.x local stdio adapter, `2026-07-28` modern protocol, exact 8 tools, explicit `projectPath`, caller-supplied structured identity, atomic apply persistence, process-local write gate, `JobManager` export lifecycle, pure stdout, clean EOF termination, Schema v3 unchanged, 26 Phase-15 acceptance tests passing, 155 total Rust tests green; accepted by ChatGPT after implementation and remediation).
- [x] Phase 16 Timeline Editing Productivity: **COMPLETE WITH DEFERRED ITEMS** (Approved plan implemented, 30 new acceptance tests + 23 new vitest tests green, RationalTime contract remediated, generic transport inheritance proven; P16-AC-56 manual Windows GUI QA deferred).
- [x] Phase 17 Research Gate: Performance Requalification & Release Foundation: **COMPLETE — READY FOR PLANNING** (Milestone defined, dual fixtures standardized, empirical release baselines established, invalid MCP measurement disclosed & superseded, hybrid timing policy defined, host-isolated workstation contract CM-01..CM-07 formalized, external gates preserved, normative contract PR-01..PR-18 accepted).

---

## 4. In Progress

None. Phases 0–15 are complete. Phase 16 is COMPLETE WITH DEFERRED ITEMS. Phase 17 Research Gate is COMPLETE. Current active implementation phase: NONE. Phase 17 implementation is NOT STARTED. Next action: ChatGPT authors plan_phase17.md using reconciled research state.

---

## 5. Blockers / Known Issues

### Active Technical Blockers / Technical Debt
- **Active Technical Blockers**: None. All 235 automated tests pass cleanly (198 Rust, 37 Vitest).
- **Known Code Defects**: Zero known Phase-16 code defects remain from automated verification.
- **RationalTime Semantics**: Zero known RationalTime semantic contract violations; representation `Ord`/`Eq` preserved ($a.\text{cmp}(b) == \text{Equal} \iff a == b$), explicit 128-bit cross-multiplication semantic helpers used for all domain timeline decisions.
- **Schema & Parity**: Zero schema regressions (Schema v3 intact); zero preview/export semantic regressions.
- **Active Deferred Acceptance Qualification**:
  ```text
  1. P16-AC-56:
     Manual Windows interactive GUI QA deferred.
     Automated coverage is green, but real desktop interaction has not been qualified.
     Both Codex and ChatGPT Work environments reported BLOCKED — GUI interaction capability unavailable.
     The user elected not to perform the 23-item manual checklist personally. This is an environment constraint, not a product defect.

  2. True Clean Windows 10/11 VM Qualification:
     Provisioning a sanitized clean virtual machine without dev tools, Visual C++ runtimes, or local state
     remains an external release gate prior to general distribution.
  ```

### Phase 15 Technical Status (RESOLVED by ADR-005 & Phase 15 Implementation)
- **Local Stdio MCP Server**: Exposes `tinixcut mcp` headless command using the official `rmcp` 3.x crate.
- **Protocol Baseline**: Targets MCP `2026-07-28` with modern stateless lifecycle (no `initialize` dependency; discovery via `server/discover`). SDK-only backward compatibility with `2025-11-25`.
- **Authoritative Tool Surface**: Exactly eight (8) Tools exposed (`tinixcut_inspect`, `tinixcut_capabilities`, `tinixcut_dry_run`, `tinixcut_apply`, `tinixcut_save`, `tinixcut_export_start`, `tinixcut_export_status`, `tinixcut_export_cancel`). No Resources, Prompts, Tasks, Apps, Sampling, Roots, or Subscriptions.
- **Explicit Project Addressing**: Every project-scoped tool requires explicit `projectPath`; zero hidden in-memory project sessions or handle tables.
- **Durable Idempotency & Concurrency**: Caller-supplied structured identity `(client_id, client_epoch, sequence_number)` per ADR-001 v3; server never auto-generates sequence numbers; one process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths (per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists).
- **Atomic Persistence**: Successful mutations commit to disk via `save_project_atomic` before returning receipts (`load -> mutate -> save -> receipt`). Schema v3 remains unchanged; zero second reducers or persistence formats.
- **Export Lifecycle**: Direct integration with existing `JobManager` via process-local `jobId`; snapshot-isolated export execution with cancellation and graceful cleanup on stdin EOF.
- **Stream Purity**: Subprocess stdio transport guarantees 100% stdout JSON-RPC framing purity; diagnostics and panic hooks route exclusively to stderr.
- **Known Implementation Characteristics (Non-blocking)**:
  - Project reload/parse occurs per project-scoped MCP request ($O(\text{project size}) \approx 2 \text{ ms}$ overhead);
  - Authoritative MCP writes serialize through one process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths (per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists);
  - No cross-process locking guarantee was added (governed by ADR-001 revisions and atomic saves);
  - Export jobs are process-local and terminate with the MCP process;
  - Backward compatibility with 2025-11-25 is SDK-provided only.

### Phase 15 Completion-Review Reconciliation History
- Implementation was completed after correct pre-implementation user approval ("Duyệt Phase 15").
- First completion report had incorrect acceptance numbering / test-count transcription.
- Completion review detected:
  - incorrect AC mapping;
  - test-count reporting discrepancy;
  - fingerprint compatibility risk requiring verification;
  - missing real server/discover evidence;
  - missing exact snapshot/shutdown evidence;
- Focused remediation proved:
  - previous 129-test regression coverage intact;
  - 26 new Phase-15 acceptance tests;
  - 155 total Rust tests green;
  - canonical `CommandOperation` fingerprint compatibility preserved;
  - real `server/discover` works;
  - snapshot isolation works;
  - real subprocess shutdown leaves no orphan FFmpeg/temp artifacts;
- Durable phase plan drift was detected and restored to exact user-approved P15-AC-01..41.
- Final ChatGPT classification = **COMPLETE**.

### Phase 14 Technical Status (RESOLVED by ADR-004)
- **P14-PREREQ-01 RESOLVED**: Full export consumes dynamic keyframes via `ExportAutomationSchedule` runtime command transport (`sendcmd`) and sample-accurate audio gain envelopes (`amultiply`). Phase 9 remains closed and is not reopened.
- **Schema State**: `Schema v3` is now the active production schema. Lossless forward migration from Schema v2 tested and verified (existing keyframes initialized with `outgoing_easing: Linear`).
- **Evaluator & Parity**: 32-iteration bisection evaluator in Rust/core is the single source of truth for preview and export.
- **Split & Trim Lifecycle**: Boundary value synthesis, de Casteljau Bézier subdivision, and hidden keyframe retention on head/tail trim verified.
- **CrossDissolve Coexistence**: ADR-003 transitions and ADR-004 keyframe automation coexist without filtergraph conflict.
- **Audio Profile Boundary**: Supported export audio sample rates are 48,000 Hz and 44,100 Hz. Arbitrary additional sample rates remain unsupported unless separately planned.
- **Known Implementation Characteristics (Non-blocking)**: Per-frame `sendcmd` transport has measurable runtime command overhead; long `f32le` audio envelopes require significant temp disk space (~691.2 MB for 60 min at 48kHz); handled via preflight disk check, chunked streaming, and deterministic temp cleanup.

### Phase 14 Governance Deviation Record
- **Approval-Order Deviation**: Phase 14 implementation was executed prior to the expected explicit user approval gate.
- **Detection & Remediation**: This deviation was detected during ChatGPT completion review. The implementation was subsequently reviewed against the full plan, remediated, verified, and explicitly ratified by the user (`Duyệt Phase 14`).
- **Durable Policy**: This does not rewrite history as pre-implementation approval. Future phases must restore the normal approval-before-implementation lifecycle: no feature implementation may proceed without explicit user approval of the phase plan.

### Phase 13 Technical Status (RESOLVED by ADR-003)
- Cut-owned CrossDissolve, duration invariance ($\Delta T=0$), isolated DOM `PreviewTransitionGroup`, and bounded export `xfade=fade:offset=0` verified green.
- Global `RationalTime PartialEq/Eq` remains representation equality by design; semantic comparisons use canonical helpers (`is_same_time`, `is_adjacent_to`, `is_adjacent_to_with_rate`).

### Hardening research inputs (RESOLVED by ADR-001 v3)
- Command ledger retention, SHA-256 fingerprints, bounded client registry ($C_{\max}=64$), active window ($K=5,000$), Staged Transaction Isolation verified and green.

### Release Gate Decisions (RESOLVED by ADR-002 & Phase 12 Reconciliation)
- **Distribution Scope**: Personal use only (non-commercial / non-public distribution); commercial licensing, EULA, and patent reviews are OUT OF SCOPE.
- **FFmpeg Distribution**: Pinned to Gyan Release Essentials 9.0.2 (x86_64 Windows), verified via `ffmpeg_provenance.json` SHA-256 digests.
- **Packaging Mechanism**: Tauri `bundle.resources` (`resources/bin/*`) with app-owned `MediaBinaryResolver`. Packaged release builds strictly require app-owned binary; system PATH is prohibited in packaged production.
- **WebView2 Mode**: Evergreen `downloadBootstrapper` in NSIS `currentUser` installer.
- **Required Qualified Export Path**: `CPU/libx264` remains the only required qualified export path.
- **Hardware Fallback Ladder**: `h264_nvenc` -> `h264_qsv` -> `h264_amf` -> `libx264` implemented with automatic fallback to `libx264`.

### Deferred Items (Non-Blocking for Personal-Use v1)

#### 1. In-Scope Verification Deferred at Completion
- **P16-AC-56 — Manual Windows Interactive GUI QA**: DEFERRED (NOT PASS). Both Codex and ChatGPT Work environments reported `BLOCKED — GUI interaction capability unavailable` due to lack of native desktop GUI automation tools. The user elected not to perform the 23-item manual checklist personally. Automated tests (198 Rust + 37 Vitest) are all passing but are not considered a substitute for this interactive qualification. To be revisited before any release milestone where interactive desktop behavior is release-critical.

#### 2. Planned Out-of-Scope Future Features
- **Phase 16 Out-of-Scope Editing Capabilities**: Shift range selection; marquee / rubber-band selection; multi-clip duplication; ripple insert; ripple trim / Q-W semantics; magnetic timeline; cross-track ripple; persistent timeline markers; track targeting matrix; Project Schema v4.
- **Draggable graph/curve editor**: DEFERRED (minimal inspector preset/coordinate UI delivered).
- **Motion paths / spatial tangents**: DEFERRED.
- **Velocity/tangent editing**: DEFERRED.
- **Overshoot Bézier curves / physics / springs**: DEFERRED ($y \in [0.0, 1.0]$ enforced in Phase 14).
- **Step/Hold interpolation**: DEFERRED.
- **Shortest-path rotation**: DEFERRED (raw scalar degree interpolation current).
- **Transition easing curves**: DEFERRED (ADR-003 transitions remain linear progress).
- **Audio crossfade**: DEFERRED to future dedicated audio phase (audio cuts remain exact butt-cuts).
- **Spectral / multi-channel waveform visualization**: DEFERRED.
- **MCP adapter**: IMPLEMENTED in Phase 15 (ADR-005 accepted and verified, local stdio server complete).
- **Phase 15 Deferred Platform Scope**: Streamable HTTP; OAuth 2.0 / auth server; remote hosting; Resources; Prompts; Tasks extension; MCP Apps; Sampling; Roots; subscriptions; MRTR; remote render; cloud project storage; collaboration; model/provider integrations; plugin framework; project-handle registry; project caching across MCP calls; distributed/cross-process locking.
- **GPU compositor redesign**: DEFERRED / not authorized for v1.
- **Stable keyframe IDs**: DEFERRED (keyframes addressed strictly by property and semantic offset).
- **Future alignment modes**: DEFERRED (start-on-cut, end-on-cut; centered semantics current).
- **Additional transition types**: DEFERRED (wipes, slides, push, iris, etc.).
- **True clean Windows 10/11 VM qualification**: DEFERRED / non-blocking for personal-use v1 (host Windows test does not prove fresh Windows installation).
- **Hardware acceleration NVENC/QSV/AMF qualification**: DEFERRED / best-effort (NVENC API 13.1 requires NVIDIA driver >= 610.00; host has 595.79; QSV/AMF host hardware absent).

### Product questions intentionally deferred

These do not block the current phase unless the user changes scope:

- 60 fps launch requirement;
- HDR / advanced color management;
- required 4K real-time multi-layer performance;
- H.265/AV1 launch requirement;
- built-in AI/provider choice;
- cloud automation/rendering;
- cross-platform release parity;
- plugin SDK / marketplace.

---

## 6. Important Recent Decisions

- Build a new application rather than forking an existing editor as the product foundation.
- Windows 10/11 is the first-class initial target.
- Tauri + React/TypeScript + Rust is the baseline runtime direction.
- The editable versioned Project Domain Model is the source of truth.
- Manual UI and future automation mutate projects through the same Command Service.
- Arbitrary floating-point seconds are not acceptable as canonical timeline time.
- FFmpeg/ffprobe stay behind a controlled media/render abstraction.
- Preview and export may use different execution backends but must share normalized composition semantics.
- Local bounded background jobs are sufficient for v1; no Redis/distributed worker architecture is justified.
- Dark compact creative-workspace UI is the project default.
- Advanced feature breadth is secondary to save/recovery correctness, command safety, preview/export parity, and deterministic export.
- **ADR-001 v3**: Structured Scoped Operation Identity (`client_id:epoch:seq`), SHA-256 fingerprints, $K=5000$ active window, Staged Transaction Isolation.
- **ADR-002**: Personal-use scope; FFmpeg 9.0.2 Gyan Release Essentials bundled via Tauri `bundle.resources`; `MediaBinaryResolver` enforcing app-owned binary in packaged builds; NSIS `downloadBootstrapper`; hardware fallback to `libx264`.
- **ADR-003**: Video CrossDissolve on adjacent cuts via track-level `CutTransition` (Schema v2); duration invariance ($\Delta T_{\text{project}} = 0$); preview `PreviewTransitionGroup` + `plus-lighter`; export bounded premultiplied `xfade=fade:offset=0`.
- **ADR-004**: Canonical Keyframe Easing, Clip-Local Lifecycle, and Sampled Export Automation Semantics: Schema v3, canonical Rust 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle, `ExportAutomationSchedule` literal `sendcmd` video commands and sample-accurate mono `f32le` audio envelopes (`amultiply`), coexistence with ADR-003 transitions. Implemented and verified in Phase 14.
- **ADR-005**: MCP Local Stdio Tools Adapter, Explicit Project Addressing, and Durable Identity: Local stdio transport (`tinixcut mcp`), primary protocol `2026-07-28`, exact 8 authoritative Tools, explicit `projectPath` addressing, caller-supplied ADR-001 structured identity, atomic apply persistence, one process-local async authoritative-write gate shared by the MCP server (Phase-15 authoritative writes serialize across all project paths), `JobManager` export lifecycle, stdout stream purity, clean stdin EOF termination and orphan child/temp cleanup. Accepted and implemented in Phase 15.
- **Phase 16 Decision & Invariants**: Ephemeral Ctrl multi-select and primary selection (`selectedItemIds`, `primarySelectedItemId`); exact 8px UI snapping; dedicated atomic `ClipGroupMove` command (simultaneous validation, collision detection); bounded single-track `ClipRippleDelete` command (exact downstream shift via `semantic_ge`); centralized shortcut framework with input suppression; non-ripple trim (`[` and `]`); primary-only duplicate (`Ctrl+D`); atomic multi-delete (`Delete`); generic CLI/JSONL/MCP transport inheritance without transport-specific edit logic; canonical `RationalTime` semantic comparison helpers (`semantic_cmp`, `semantic_ge`, `semantic_lt`, etc.) preserving the Rust stdlib representation-based `Ord`/`Eq` invariant ($a.\text{cmp}(b) == \text{Equal} \iff a == b$); Schema v3 unchanged; zero renderer/compositor/export semantic changes; `P16-AC-56` real interactive Windows GUI QA deferred.
- **Phase 14 Governance Record**: Implementation was executed prior to explicit user approval; detected during ChatGPT completion review; remediated, verified, and explicitly ratified by user (`Duyệt Phase 14`). Normal approval-before-implementation lifecycle must be restored for future phases.

---

## 7. Verification State

```text
Full cargo test: PASS (198 passed, 0 failed)
- Rust unit tests: 107 passed
- Export POC tests: 8 passed
- Phase 14 acceptance tests: 15 passed
- Phase 15 acceptance tests: 26 passed
- Phase 16 research POC tests: 12 passed
- Phase 16 timeline productivity acceptance tests: 30 passed
Frontend tests: PASS (37 vitest passed across 4 test suites, 0 failed)
- timelineSelection.test.ts: 6 passed
- App.test.ts: 14 passed
- timelineSnapping.test.ts: 12 passed
- timelineShortcuts.test.ts: 5 passed
Typecheck: PASS (tsc --noEmit, 0 errors)
Lint: PASS (cargo clippy -- -D warnings, 0 warnings)
Formatting: PASS (cargo fmt --check)
Build: PASS (pnpm run build, cargo build)
FFmpeg: PASS (app-owned bundled FFmpeg 9.0.2 verified via MediaBinaryResolver)
P14-PREREQ-01: RESOLVED (dynamic keyframes affect full video/audio export)
Phase 15 Qualification: PASS (Windows real spawned MCP subprocess, stdout purity, EOF cleanup)
Automation Acceptance: PASS (10-step E2E scenario passed, transition & easing automation verified)
Preview/Export Parity: PASS (Gate E / POC 2 within <0.1px tolerance, Phase 13/14 fixtures verified)
ADR-001 Idempotency / Replay: PASS (ClipGroupMove and ClipRippleDelete deduplication and collision verified)
CLI / JSONL / MCP Generic Transport: PASS (Generic CommandEnvelope dry-run/apply verified without custom transport logic)
P16-AC-56 (Manual Windows GUI QA): DEFERRED (Desktop GUI automation unavailable in agent environments; manual checklist not executed; not marked PASS)
Known pre-existing failures: None
```

---

## 8. Runtime / Environment State

```text
Frontend: React 19 + TypeScript + Vite in Tauri WebView (Dark Compact Creative Workspace)
Native core: Tauri 2.12.x + Rust (Modular Monolith: domain, command, persistence, media, preview, composition, export, job, hardening, automation)
Standalone backend: None (Local-first architecture)
Media sidecars: Bundled FFmpeg / FFprobe 9.0.2 Essentials (App-owned packaged resource boundary)
Local services: Controlled Command Service, Job Manager (bounded concurrency & backpressure), MCP Stdio Server (rmcp 3.x)
Database: Versioned Project Domain JSON files (Schema v3 currently in production; promoted losslessly from Schema v2) + atomic safe promotion + recovery journals
Packaging: NSIS standalone installer (currentUser, downloadBootstrapper)
Current development mode: Phases 0–15 COMPLETE, Phase 16 COMPLETE WITH DEFERRED ITEMS, Phase 17 Research Gate COMPLETE. Current active implementation phase: NONE. Phase 17 Implementation: NOT STARTED.
Relevant process/runtime notes: 235 total automated tests passing cleanly (198 Rust, 37 Vitest), clean process cleanup, zero background leaks
```

---

## 9. Next Actions

1. ChatGPT authors plan_phase17.md using reconciled research state.
2. Phase 17 implementation remains NOT STARTED. Do NOT begin Phase 17 implementation without approved plan and explicit user instruction.

---

## 10. Resume Context

```text
Current implementation checkpoint:
- Phases 0 through 15 fully completed.
- Phase 16: COMPLETE WITH DEFERRED ITEMS (P16-AC-56 deferred).
- Phase 17 Research Gate: COMPLETE — READY FOR PLANNING.
- Phase 17 Implementation: NOT STARTED.
- Current active implementation phase: NONE.
- Selected Milestone: Phase 17 — Performance Requalification & Release Foundation.
- Project Schema: Schema v3 unchanged.
- ADR Decision: ADR-006 NOT REQUIRED.
- Automated tests: 198 Rust tests passed + 37 Vitest tests passed (all green).
- Next Action: ChatGPT authors plan_phase17.md using reconciled research state.

Lifecycle State:
Phases 0-15 COMPLETE -> Phase 16 COMPLETE WITH DEFERRED ITEMS (P16-AC-56 deferred) -> Phase 17 Research Gate COMPLETE -> Current Active Phase: NONE -> Phase 17 Implementation: NOT STARTED -> Next Action: ChatGPT authors plan_phase17.md using reconciled research state.
```

---

## 11. Relevant Documents

```text
AGENTS.md
DEVELOPMENT_WORKFLOW.md
UX_UI.md
RESEARCH.md
ARCHITECTURE.md
PLAN.md
PROJECT_STATUS.md
ADR_001_DURABLE_IDEMPOTENCY_TOPOLOGY_AND_FINGERPRINT.md
ADR_002_WINDOWS_FFMPEG_DISTRIBUTION_AND_HARDWARE_FALLBACK.md
ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md
ADR_004_CANONICAL_KEYFRAME_EASING_CLIP_LOCAL_LIFECYCLE_AND_SAMPLED_EXPORT_AUTOMATION_SEMANTICS.md
ADR_005_MCP_LOCAL_STDIO_TOOLS_ADAPTER_EXPLICIT_PROJECT_ADDRESSING_AND_DURABLE_IDENTITY.md
```

---

## 12. Status Maintenance Rule

Update `PROJECT_STATUS.md` when there is a meaningful durable checkpoint, such as:

- a phase completed;
- the active objective changed;
- a significant blocker appeared or was resolved;
- a major decision changed the next work;
- a long session is ending and continuation context should be preserved.

Do **not** update this file after every small code change.

Always maintain it as the current snapshot.

Do not append an endless history.

Remove or replace stale information.

---

## 13. Relationship to Other Documents

```text
RESEARCH.md
→ Why the baseline decisions were made and what evidence/limitations exist.

ARCHITECTURE.md
→ How the durable system is structured.

PLAN.md
→ What is planned and in what order.

PROJECT_STATUS.md
→ Where implementation currently stands.

AGENTS.md
→ How coding agents should operate.

DEVELOPMENT_WORKFLOW.md
→ How the real repository is validated, run, restarted, and prepared for testing.

UX_UI.md
→ How the user-facing editor should look and behave.
```

Do not duplicate information unnecessarily across these files.
