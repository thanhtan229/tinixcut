# Phase 15 — MCP Automation Adapter

## Status

Status: COMPLETE — accepted by ChatGPT after implementation and remediation

Governance and Reconciliation History:
- The plan was submitted for user review based on ADR-005 and the existing Phase 10/11 automation architecture.
- Explicit user approval occurred before implementation ("Duyệt Phase 15").
- Approved scope, architectural constraints, and acceptance criteria P15-AC-01 through P15-AC-41 were locked for implementation.
- **Completion-Review Reconciliation History**:
  - An intermediate report misquoted/drifted acceptance criteria numbering and test-count transcription.
  - This drift was detected during ChatGPT completion review.
  - The durable plan was restored to the exact user-approved criteria (P15-AC-01 through P15-AC-41).
  - Production implementation already matched the approved eight-tool surface; no scope expansion was accepted.
  - Focused remediation confirmed:
    - Pre-implementation user approval was verified;
    - Existing 129-test regression coverage remained fully intact;
    - 26 new Phase-15 acceptance tests were implemented and verified;
    - 155 total Rust tests green, 14 frontend tests green;
    - Canonical `CommandOperation` fingerprint compatibility preserved;
    - Real `server/discover` verified;
    - Snapshot isolation verified;
    - Real subprocess shutdown leaves zero orphaned FFmpeg processes or temporary artifacts;
  - Final ChatGPT completion decision: **COMPLETE**.

---

## 1. Objective

Deliver a production-ready, local-first Model Context Protocol (MCP) server adapter for TinixCut.

Phase 15 must:

1. Expose a local stdio MCP server via `tinixcut mcp` CLI dispatch before GUI startup.
2. Target the primary modern MCP specification (`2026-07-28`) with stateless lifecycle and discovery, with backward compatibility for `2025-11-25` if provided transparently by the official Rust SDK (`rmcp` 3.x).
3. Implement exactly eight (8) authoritative tools:
   - `tinixcut_inspect`
   - `tinixcut_capabilities`
   - `tinixcut_dry_run`
   - `tinixcut_apply`
   - `tinixcut_save`
   - `tinixcut_export_start`
   - `tinixcut_export_status`
   - `tinixcut_export_cancel`
4. Enforce explicit `projectPath` addressing on all project-scoped tools, maintaining zero hidden in-memory project sessions.
5. Reuse the existing canonical `CommandService`, `JobManager`, atomic persistence (`load_project`, `save_project_atomic`), and ADR-001 v3 durable idempotency engine.
6. Require caller-supplied structured identity `(client_id, client_epoch, sequence_number)` on mutations (`tinixcut_apply`), strictly forbidding server-side auto-generation of sequence numbers.
7. Enforce durable atomic persistence before returning mutation success (`load -> mutate -> save_atomic -> receipt`).
8. Implement a process-local authoritative-write serialization gate to prevent concurrent read-modify-write races.
9. Preserve structured, machine-readable domain error codes inside `isError: true` tool execution results while maintaining protocol errors for JSON-RPC framing failures.
10. Ensure absolute stdout protocol purity, redirecting all logs, diagnostics, and panic traces to stderr.
11. Ensure clean stdin EOF termination and bounded cleanup of active export child processes and temporary files.

This phase is governed by ADR-005 and ADR-001 v3, and must introduce zero schema changes and zero second reducers.

---

## 2. Authoritative Inputs

Implementation must follow:

- `ADR_005_MCP_LOCAL_STDIO_TOOLS_ADAPTER_EXPLICIT_PROJECT_ADDRESSING_AND_DURABLE_IDENTITY.md`
- `ADR_001_DURABLE_IDEMPOTENCY_TOPOLOGY_AND_FINGERPRINT.md`
- `ARCHITECTURE.md`
- `PLAN.md`
- `PROJECT_STATUS.md`
- Existing Phase 10/11 automation architecture (`src-tauri/src/modules/automation/`)

---

## 3. Scope and Non-Goals

### 3.1 In Scope

- `rmcp` 3.x crates.io integration with minimal features (`server`, `macros`, `transport-io`, `schemars`).
- CLI sub-command dispatch: `tinixcut mcp`.
- Primary modern MCP `2026-07-28` protocol compliance.
- Typed tool schemas conforming to JSON Schema 2020-12 with camelCase wire format.
- Exactly 8 authoritative tools.
- Explicit `projectPath` parameter for project operations.
- Direct integration with `CommandService` and `JobManager`.
- Atomic persistence on mutation (`tinixcut_apply`).
- Concurrency serialization via process-local async write gate.
- Structured domain error preservation.
- Export job start, status polling, and cancellation.
- Absolute stdout protocol purity and stderr diagnostic routing.
- Graceful stdin EOF shutdown and child cleanup.
- Integration tests with in-process client and real spawned subprocess.

### 3.2 Explicit Non-Goals / Deferred Scope

- Remote Streamable HTTP, SSE listeners, WebSockets, or network port bindings.
- OAuth 2.0 authorization servers, PKCE, bearer tokens, or API key infrastructure.
- Non-tool MCP primitives: Resources, Prompts, Tasks, Sampling, Roots, Subscriptions, MCP Apps, MRTR.
- Hidden "currently open project" session state or project-handle registries.
- Remote/distributed render or cloud rendering queues.
- Direct AI/LLM provider integrations or client SDKs.
- Project schema modifications (Schema v3 remains unchanged).
- Secondary command engines, reducers, or persistence formats.

---

## 4. Implementation Slices

### Slice A — MCP Server Skeleton
- Add `rmcp` 3.x and `tokio` to `src-tauri/Cargo.toml` with minimal required features.
- Create `src-tauri/src/modules/automation/mcp.rs`.
- Wire `tinixcut mcp` in `src-tauri/src/main.rs` before GUI initialization.
- Setup stdio transport with pure stdout framing and stderr logging.
- Verify modern protocol discovery and tool listing.

### Slice B — Typed Schemas and Read-Only Tools
- Define typed argument and response structs with JSON Schema derives (camelCase).
- Implement `tinixcut_inspect`: loads project and returns inspection report.
- Implement `tinixcut_capabilities`: returns system and editor capabilities.
- Implement `tinixcut_dry_run`: evaluates envelope via `CommandService::dry_run` without saving or modifying the project file or advancing sequence numbers.

### Slice C — `tinixcut_apply` Mutation Engine
- Implement `tinixcut_apply` flow:
  1. Acquire async authoritative-write gate.
  2. Load canonical project via `load_project(projectPath)`.
  3. Validate caller-supplied structured identity (`client_id`, `client_epoch`, `sequence_number`, `transaction_id`, `base_revision`, `project_id`).
  4. Execute transaction via `CommandService::apply_envelope`.
  5. Atomically persist via `save_project_atomic(projectPath)`.
  6. Return `CommandReceipt`.
  7. Release write gate.
- Support idempotency replay, collision detection, sequence gap rejection, and stale revision rejection.

### Slice D — Explicit Save
- Implement `tinixcut_save`:
  1. Acquire write gate.
  2. Load and validate project at `projectPath`.
  3. Atomically write to `projectPath` or `targetPath` (if specified).
  4. Return `{ savedPath }`.
  5. Release write gate.

### Slice E — Export Tools and Process Lifecycle
- Implement `tinixcut_export_start`: loads snapshot, registers job with `JobManager`, spawns render, returns `jobId`.
- Implement `tinixcut_export_status`: queries `JobManager` for status, progress, and error.
- Implement `tinixcut_export_cancel`: signals job cancellation.
- Implement graceful stdin EOF shutdown with active job and child process cleanup.

### Slice F — Acceptance Qualification
- Create `src-tauri/tests/phase15_mcp_acceptance.rs`.
- Include in-process tests and real spawned subprocess (`tinixcut mcp`) tests.
- Validate all 41 acceptance criteria.

---

## 5. Acceptance Criteria (P15-AC-01 through P15-AC-41)

- **P15-AC-01**: `tinixcut mcp` launches MCP stdio server without GUI.
- **P15-AC-02**: Primary tested protocol is `2026-07-28`.
- **P15-AC-03**: Modern primary flow works without legacy initialize dependency.
- **P15-AC-04**: Uses official stable crates.io `rmcp` 3.x.
- **P15-AC-05**: No HTTP/OAuth/network listener introduced.
- **P15-AC-06**: `tools/list` exposes exactly eight approved Tools.
- **P15-AC-07**: No Resources/Prompts/Tasks/Apps/Sampling/Roots/subscription/MRTR product scope.
- **P15-AC-08**: Tool schemas are valid JSON Schema 2020-12.
- **P15-AC-09**: Wire field names preserve camelCase conventions.
- **P15-AC-10**: Inspect matches existing automation semantics.
- **P15-AC-11**: Capabilities exposes current Phase-13/14 operation support.
- **P15-AC-12**: Dry-run performs no project mutation/save.
- **P15-AC-13**: Project-scoped Tools require explicit `projectPath`.
- **P15-AC-14**: No project-handle manager exists.
- **P15-AC-15**: Successful apply is atomically persisted before success.
- **P15-AC-16**: No hidden unsaved cross-request project state.
- **P15-AC-17**: MCP apply requires caller-provided structured identity.
- **P15-AC-18**: MCP does not auto-generate a durable apply sequence.
- **P15-AC-19**: Lost-response retry with same identity cannot duplicate edit.
- **P15-AC-20**: Idempotency/revision failures retain TinixCut stable error codes.
- **P15-AC-21**: Stale revision appears as structured failed Tool result.
- **P15-AC-22**: Protocol errors and TinixCut domain errors remain correctly separated.
- **P15-AC-23**: Domain Tool errors expose structured `code`, `message`, safe details.
- **P15-AC-24**: Concurrent authoritative MCP writes serialize safely.
- **P15-AC-25**: No distributed lock/database is added.
- **P15-AC-26**: Save Tool is explicit load/validate/atomic-save, not hidden flush.
- **P15-AC-27**: Export start uses existing JobManager and returns jobId.
- **P15-AC-28**: Export status matches existing JobManager semantics.
- **P15-AC-29**: Export cancel uses existing cancellation and output safety.
- **P15-AC-30**: MCP Tasks is unnecessary for export lifecycle.
- **P15-AC-31**: Export preserves existing snapshot isolation.
- **P15-AC-32**: Spawned MCP binary stdout is 100% valid protocol messages.
- **P15-AC-33**: Diagnostics do not contaminate stdout.
- **P15-AC-34**: stdin EOF cleanly stops server.
- **P15-AC-35**: Shutdown cleans MCP-owned export jobs without orphaned media processes/temp artifacts.
- **P15-AC-36**: MCP adds no arbitrary shell/process/filesystem enumeration authority.
- **P15-AC-37**: No second reducer, command engine, persistence format or idempotency store.
- **P15-AC-38**: `.tcproj` schema/version is unchanged by MCP.
- **P15-AC-39**: Any `2025-11-25` compatibility is SDK-provided only.
- **P15-AC-40**: Existing CLI/JSONL/domain/export/Phase-13/Phase-14 behavior remains green.
- **P15-AC-41**: Final verification passes full Rust tests, clippy, fmt, cargo build, frontend tests, typecheck and production build.

---

## 6. Verification Results

- **Full Rust Suite**: 155 passed, 0 failed (106 unit tests, 8 export POC tests, 15 Phase 14 acceptance tests, 26 Phase 15 acceptance tests).
- **Phase 15 Acceptance Suite** (`src-tauri/tests/phase15_mcp_acceptance.rs`): 26 passed, 0 failed.
- **Frontend Tests** (`src/App.test.ts`): 14 passed, 0 failed.
- **Static Analysis & Builds**:
  - `cargo clippy -- -D warnings`: 0 warnings, clean.
  - `cargo fmt --check`: clean.
  - `cargo build`: clean.
  - `npm run typecheck` (`tsc --noEmit`): 0 errors.
  - `npm run build` (`tsc && vite build`): clean production build.
- **Windows Real MCP Subprocess Qualification**:
  - `tinixcut mcp` real spawned process verified on Windows 10/11 workstation (`test_p15_ac01_ac02_ac32_ac33_ac34_ac40_real_subprocess_e2e`).
  - Stdin EOF cleanly terminates process.
  - Stdout contains 100% valid JSON-RPC frames with zero diagnostic contamination.
  - Real subprocess shutdown verified with zero orphaned media processes or temporary files (`test_p15_ac35_subprocess_shutdown_orphan_and_temp_cleanup`).

---

## 7. Deferred Items

The following capabilities are explicitly deferred from Phase 15 (recorded as deferred, NOT incomplete Phase-15 work):
- Streamable HTTP transport;
- OAuth 2.0 / authorization server / token infrastructure;
- Remote hosting, reverse proxy, or cloud ingress;
- MCP non-tool primitives: Resources, Prompts, Tasks extension, MCP Apps, Sampling, Roots, Subscriptions;
- MRTR (Model-Router-Tool-Router);
- Remote render / distributed render queues (Celery, Redis);
- Cloud project storage;
- Multi-user collaboration (CRDT / OT);
- Direct AI/LLM model/provider integrations;
- Public plugin framework / WASM host;
- Project-handle registry / server-side handle tables;
- Project caching across MCP calls (reload/parse per project-scoped request is accepted);
- Per-path locking or caching optimization (deferred; one process-local write gate shared by the MCP server);
- Distributed or cross-process file locking.

---

## 8. Implementation Characteristics & Technical Debt Boundaries

The following non-blocking characteristics are accepted architectural boundaries, not Phase-15 defects:
- **Per-Request Project Reload/Parse**: Each project-scoped MCP tool call reads and parses `.tcproj` from disk ($O(\text{project size}) \approx 2 \text{ ms}$ overhead), ensuring stateless simplicity, crash consistency, and zero stale in-memory cache drift.
- **Process-Local Write Serialization**: Authoritative MCP writes (`tinixcut_apply`, `tinixcut_save`) serialize through one process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths, preventing local read-modify-write races. Per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists.
- **Cross-Process Locking Boundary**: No distributed or cross-process file lock was added; cross-process safety relies on ADR-001 revision checking (`base_revision`) and atomic file replacement (`save_project_atomic`).
- **Export Job Lifecycle**: Export jobs are process-local to the running `tinixcut mcp` process and terminate cleanly if the MCP process exits.
- **Protocol Compatibility**: Backward compatibility with legacy `2025-11-25` clients is provided strictly by the official `rmcp` 3.x SDK; no secondary legacy protocol stack is maintained.

