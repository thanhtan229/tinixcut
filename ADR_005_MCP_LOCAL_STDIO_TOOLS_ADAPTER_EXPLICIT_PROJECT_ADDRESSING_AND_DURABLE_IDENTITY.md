# ADR-005: MCP Local Stdio Tools Adapter, Explicit Project Addressing, and Durable Identity

## Status

**ACCEPTED & IMPLEMENTED — Phase 15**  
**Date:** 2026-10-05  
**Deciders:** Core Architecture Team  
**Scope:** Model Context Protocol (MCP) Integration, Transport Model, Protocol Version Baseline, Tool Surface, Project Addressing, Concurrency Boundary, Durable Idempotency Mapping, Error Semantics, and Subprocess Stream Isolation  
**Consulted:** Phase 15 Narrow Repo Audit, Official MCP Specification (`2026-07-28`), Official Rust SDK (`modelcontextprotocol/rust-sdk` `rmcp` 3.x), JSON Schema 2020-12, ADR-001 v3, Phase 10 Automation Surface Architecture  

### Implementation Verification Note
Implemented and verified in Phase 15 (`src-tauri/src/modules/automation/mcp.rs`, `src-tauri/tests/phase15_mcp_acceptance.rs`):
- **rmcp stdio adapter**: Official `rmcp` 3.x stdio transport integrated under headless CLI dispatch `tinixcut mcp`.
- **2026-07-28 modern lifecycle/discovery**: Stateless protocol core with discovery via `server/discover` and zero legacy `initialize` handshake requirement; transparent SDK backward compatibility with `2025-11-25`.
- **Explicit projectPath**: All project-scoped tools mandate explicit `projectPath`; zero hidden project sessions or handle tables.
- **Exact eight Tools**: `tinixcut_inspect`, `tinixcut_capabilities`, `tinixcut_dry_run`, `tinixcut_apply`, `tinixcut_save`, `tinixcut_export_start`, `tinixcut_export_status`, `tinixcut_export_cancel`.
- **Caller-supplied ADR-001 identity**: `tinixcut_apply` requires caller `(client_id, client_epoch, sequence_number)`; server never auto-generates sequences on retry.
- **Atomic apply save**: Durably persisted via `save_project_atomic` before returning receipt (`load -> mutate -> save -> receipt`); Schema v3 unchanged.
- **Process-local write gate**: One process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths to prevent local read-modify-write races (per-path locking optimization is not implemented and remains deferred future work).
- **Error mapping**: Preserves structured machine-readable TinixCut domain errors (`code`, `message`, `details`) inside `isError: true` tool execution results while maintaining protocol errors for JSON-RPC framing failures.
- **JobManager export lifecycle**: Export start, status polling, and cancellation backed by shared `JobManager` with snapshot isolation.
- **Stdout/stderr purity**: Stdout strictly reserved for valid JSON-RPC frames; all diagnostics, resolver logs, and panic hooks route to stderr.
- **Clean EOF child/temp cleanup**: Stdin EOF cleanly shuts down the server with deterministic cleanup of active export child processes and temporary files.  

---

## 1. Context and Problem Statement

TinixCut established its headless automation foundation in Phase 10 (`AutomationSession`, `AutomationAdapter`, `tinixcut session` / `--stdio`) and hardened it in Phase 11 under ADR-001 v3 (Structured Scoped Operation Identity, Active Ledger Window $K=5000$, and cryptographic SHA-256 operation fingerprints). Automation operates strictly as an external client to the editor, routing every mutation through the canonical `CommandService`, `JobManager`, and atomic persistence engine.

With the core editor, transitions (Phase 13, ADR-003), and advanced keyframing (Phase 14, ADR-004) complete, Phase 15 introduces an adapter for the standard **Model Context Protocol (MCP)**. This enables AI coding agents and external automation orchestrators to interact with TinixCut through a standardized protocol.

However, integrating MCP poses critical architectural questions:
1. **Architectural Seam & Reducer Duplication**: Does MCP introduce its own state reducer, tool-specific command registry, or direct project-file mutation paths?
2. **Transport & Network Attack Surface**: Should MCP expose an HTTP/SSE/Streamable HTTP listener with authentication (OAuth, bearer tokens), or run as a local process?
3. **Protocol Evolution & Lifecycle**: The MCP specification evolved from earlier drafts (e.g. `2024-11-05`, `2025-11-25`) to the modern stable specification (`2026-07-28`). Early drafts mandated `initialize` / `notifications/initialized` handshakes and stateful protocol sessions. The modern `2026-07-28` specification adopts a stateless protocol core with discovery via `server/discover`. Which protocol version and lifecycle contract should TinixCut implement?
4. **State Management & Project Addressing**: Should the MCP server maintain hidden process-wide "currently opened project" state, or should tool calls explicitly address the target project?
5. **Mutation Persistence Semantics**: Can mutations remain unsaved across tool calls (e.g., exposing `saveAfter=false`), risking data loss or divergence if the agent process dies?
6. **Concurrency & Race Conditions**: While the Phase 10 JSONL stdio session is strictly sequential on a single thread, an MCP server runtime (e.g. Tokio-backed) may dispatch tool requests concurrently. If two concurrent tool calls target the same project file, how do we prevent interleaving read-modify-write races?
7. **Idempotency & Replay Safety**: If an MCP tool call succeeds but the response frame is dropped over stdio or lost in transit, how does the client retry without triggering duplicate edits? Does the server invent sequence numbers, or require caller-provided structured identity?
8. **Error Representation**: How are rich TinixCut domain errors (`STALE_REVISION`, `IDEMPOTENCY_COLLISION`, `OPERATION_ALREADY_COMMITTED`, `SEQUENCE_GAP_DETECTED`) surfaced through MCP without collapsing into opaque strings or violating JSON-RPC framing?
9. **Stdout Protocol Purity**: MCP stdio relies on pristine JSON-RPC on stdout. Any uncontrolled log, panic message, or third-party library trace written to stdout will corrupt the protocol stream.

This ADR resolves these architectural questions and locks the technical contract prior to Phase 15 implementation planning.

---

## 2. Decision Drivers

- **Zero Bypasses of Canonical Architecture**: MCP must be an external transport adapter only. It must reuse existing `CommandService`, `JobManager`, `load_project`, `save_project_atomic`, `ExportEngine`, and ADR-001 idempotency logic. Zero second reducers or parallel mutation paths.
- **Minimal Attack Surface & Local-First Alignment**: Personal-use desktop video editor running on the user's workstation. Avoid exposing remote network attack surfaces (no HTTP daemon, OAuth servers, or port binding).
- **Stateless Protocol Baseline**: Align with the modern MCP specification (`2026-07-28`) avoiding obsolete session-state assumptions.
- **Durable Correctness over Speculative Performance**: Every authoritative mutation must be durably committed and atomically saved to disk before reporting success. No invisible in-memory unsaved project states.
- **Strict ADR-001 v3 Compliance**: Caller-supplied structured identity `(client_id, client_epoch, sequence_number)` must govern authoritative mutations. Lost-response retries must never duplicate edits.
- **Machine-Readable Error Preservation**: Preserve domain error codes and structured diagnostics inside MCP tool execution results (`isError: true` with structuredContent).
- **Uncompromised Protocol Framing**: Strict separation of protocol frames on `stdout` and diagnostic logs on `stderr`.

---

## 3. Decision Outcome

We adopt a **Local Stdio Tools-Only MCP Adapter with Explicit Project Addressing, Process-Local Write Serialization, Caller-Supplied Structured Identity, and Modern 2026-07-28 Protocol Alignment**.

### 3.1 MCP Role & Architecture Boundary

MCP is strictly a **thin external transport adapter**. It sits at the exact same architectural layer as the CLI dispatcher and the Phase 10 JSONL session dispatcher:

```text
┌─────────────────────────────────────────────────────────────┐
│                       External Clients                      │
│   GUI (React / Tauri)   │   CLI / JSONL   │   MCP Client    │
└────────────┬────────────┴────────┬────────┴────────┬────────┘
             │                     │                 │ (stdio)
             ▼                     ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│              Application / Automation Layer                 │
│  Tauri Commands    │   AutomationSession / AutomationAdapter│
└────────────┬─────────────────────┬─────────────────┬────────┘
             └─────────────────────┼─────────────────┘
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│                 Authoritative Domain Services               │
│   CommandService (Domain Invariants & ADR-001 Idempotency)   │
│   JobManager (Snapshot Isolation & Cancellation)            │
│   Persistence (Atomic Save / Load / Relink)                 │
│   ExportEngine (CompositionPlan & FFmpeg Process)           │
└─────────────────────────────────────────────────────────────┘
```

**Explicit Invariants:**
1. MCP contains **no second reducer**.
2. MCP contains **no second command registry**.
3. MCP exposes **no MCP-only edit semantics**.
4. MCP provides **no project-file mutation bypass**.
5. MCP maintains **no independent idempotency ledger or token store**.

### 3.2 Transport: Local Stdio Only

- Phase 15 implements **local stdio transport only**.
- Entrypoint binary command:
  ```text
  tinixcut mcp
  ```
- `stdin` and `stdout` are strictly reserved for MCP JSON-RPC protocol frames.
- `stderr` is strictly reserved for diagnostic logs, tracing, and error telemetry.
- **Remote Streamable HTTP is explicitly DEFERRED**. Phase 15 includes:
  - NO HTTP listener / web server;
  - NO OAuth 2.0 authorization server or token validator;
  - NO bearer-token management infrastructure;
  - NO remote hosting, reverse proxy, or cloud ingress;
  - NO service discovery or mDNS advertisement.

### 3.3 Protocol Version: Primary Target MCP `2026-07-28`

The primary protocol target for TinixCut Phase 15 is **MCP `2026-07-28`**.

**Modern Lifecycle Rules:**
1. **No `initialize` or `notifications/initialized` handshake**: The modern specification removes the legacy initialization exchange.
2. **Stateless Protocol Core**: MCP connections do not negotiate or maintain stateful protocol sessions.
3. **Discovery via `server/discover`**: Supported for clients discovering server capabilities and tool sets; however, `server/discover` is not required prior to executing tool calls.
4. **Per-Request Metadata**: Tool calls convey modern protocol and client metadata headers as specified by the standard.
5. **Backward Compatibility**: Compatibility with legacy MCP `2025-11-25` is supported **only** if provided transparently by the official Rust SDK (`rmcp` 3.x). TinixCut will not implement, fork, or maintain a secondary legacy protocol stack.
6. **Audit Correction**: The narrow repository audit had suggested verifying `initialize` against `2026-07-28`. This ADR explicitly corrects that proposal: under `2026-07-28`, `initialize` is not part of the valid lifecycle.

### 3.4 Official Rust SDK & Schema Standard

- **SDK Selection**: Approved candidate is the official `modelcontextprotocol/rust-sdk`, crate `rmcp` version 3.x.
- **Runtime & Transport**: Use `rmcp` stdio server primitives and tool routing.
- **Schema Generation**: Use the `schemars` version re-exported by or strictly compatible with `rmcp` 3.x. Do NOT independently pin or downgrade to obsolete `schemars 0.8` merely because transitive dependencies reference it.
- **JSON Schema Target**: Tool argument and result schemas must conform to **JSON Schema 2020-12**.

### 3.5 Primitive Scope: Tools-Only

Phase 15 is strictly **Tools-only**.

| MCP Primitive | Phase 15 Status | Rationale |
|---|---|---|
| **Tools** | **IN SCOPE** | Core requirement for automated timeline editing, inspection, and export. |
| **Resources** | DEFERRED | Timeline entities are dynamic, mutable domain graphs, not static URI resources. |
| **Prompts** | DEFERRED | TinixCut is a domain editor, not an AI prompt library. |
| **Tasks** | DEFERRED | Process-local asynchronous export jobs are fully managed via standard tools (`jobId`). |
| **MCP Apps** | DEFERRED | UI is provided by Tauri native desktop shell; embedded UI extensions are out of scope. |
| **Sampling** | DEFERRED | Server never initiates model prompts or queries back to the LLM client. |
| **Roots** | DEFERRED | Project paths are explicitly supplied by the caller. |
| **Subscriptions** | DEFERRED | Stdio clients poll job status or execute synchronous queries. |
| **MRTR (Model-Router-Tool-Router)** | DEFERRED | Complex multi-hop routing is not needed for a dedicated desktop tool. |

### 3.6 Authoritative MCP Tool Surface

The MCP server exposes exactly eight (8) authoritative tools:

```text
1. tinixcut_inspect
   Params: { projectPath: string }
   Returns: ProjectInspectReport

2. tinixcut_capabilities
   Params: {}
   Returns: CapabilitiesReport

3. tinixcut_dry_run
   Params: { projectPath: string, envelope: CommandEnvelope }
   Returns: DiffReport

4. tinixcut_apply
   Params: { projectPath: string, envelope: CommandEnvelope }
   Returns: CommandReceipt

5. tinixcut_save
   Params: { projectPath: string, targetPath?: string }
   Returns: { savedPath: string }

6. tinixcut_export_start
   Params: { projectPath: string, outputPath: string, profile?: ExportProfile }
   Returns: ExportStartReceipt { jobId: string, status: string, outputPath: string }

7. tinixcut_export_status
   Params: { jobId: string }
   Returns: ExportJobStatusReport { jobId: string, status: string, progress: f32, error?: string }

8. tinixcut_export_cancel
   Params: { jobId: string }
   Returns: { cancelled: bool }
```

**Session Lifecycle Exclusion:**
The Phase 10 JSONL session lifecycle methods (`session.open`, `session.close`, `session.shutdown`) are **NOT** exposed as MCP tools. The OS process and stdio pipe lifetime govern the MCP connection.

### 3.7 Explicit Project Addressing

Phase 15 adopts **Explicit `projectPath` per project-scoped tool call**.

- **No Implicit Global State**: The server does NOT maintain a hidden "current project" across tool calls.
- **No Protocol Session State**: Avoids reliance on client connection state or session resumption cookies.
- **No Opaque Project Handles**: Eliminates the complexity and leak risk of a server-side handle table (`Map<HandleId, Project>`).
- **Execution Model**:
  ```text
  tinixcut_apply(projectPath, envelope)
    │
    ├── 1. Acquire process-local authoritative-write gate
    ├── 2. Load canonical project from disk (`load_project`)
    ├── 3. Execute CommandService transaction
    ├── 4. Atomically persist updated project (`save_project_atomic`)
    ├── 5. Release write gate
    └── 6. Return CommandReceipt
  ```
- **Performance Trade-off**: Project reloading on each mutation introduces repeated parsing overhead ($O(\text{project size}) \approx 2 \text{ ms}$ for 10,000-item projects per Phase 11 benchmarks). This trade-off is accepted for Phase 15 to prioritize absolute correctness, crash consistency, and stateless simplicity. In-memory caching with filesystem invalidation is deferred.

### 3.8 Mutation Persistence Semantics

Because Phase 15 operates without stateful project sessions:
1. `tinixcut_apply` is **inherently persistent**. A successful mutation must be durably written to disk via `save_project_atomic` before returning success to the MCP client.
2. **No `saveAfter=false` Parameter**: The MCP tool interface does not expose `saveAfter`. Allowing an unpersisted mutation in a stateless tool model would create invisible transient state that disappears on subsequent tool calls.
3. **Transaction Batching**: Multi-operation atomic edits remain fully supported via the standard `CommandEnvelope` (containing multiple `CommandOperation` entries committed atomically in a single transaction).
4. `tinixcut_save`: Provided for explicit Save-As / copy operations (`targetPath`), but does not imply the existence of uncommitted server-side state.

### 3.9 Concurrency & Serialization Boundary

While stdio communication is inherently a single serial character stream, MCP SDK runtimes (such as Tokio async tasks) may execute tool handler futures concurrently.

**Concurrency Invariants:**
1. **Process-Local Authoritative-Write Gate**: The adapter maintains one process-local async authoritative-write gate shared by the MCP server (`write_gate: Arc<tokio::sync::Mutex<()>>`). All authoritative writes (`tinixcut_apply`, `tinixcut_save`) acquire this gate, serializing writes across all project paths within the process.
2. Operations that perform `load -> mutate -> save` are strictly serialized within the process. Concurrent tool calls targeting project files will queue rather than race.
3. **Deferred Per-Path Optimization**: Per-path locking/keyed mutexes (`HashMap<PathBuf, Mutex>`) are NOT implemented in Phase 15 and remain deferred future work if ever justified by measurement.
4. **No Redesign of Domain Engine**: `CommandService` remains synchronous and single-threaded per transaction.
5. **No Cross-Process Distributed Locks**: Cross-process concurrency is governed by ADR-001 revision checking (`base_revision`) and atomic file replacement (`save_project_atomic`). Distributed locking mechanisms (e.g. file lock daemons) are rejected for Phase 15.

### 3.10 Durable Idempotency & Structured Identity

ADR-001 v3 is the sole and uncompromised authority on idempotency.

```text
Caller Retry Flow (Network / Stdio Loss):
Caller submits: (client_id: "agent-1", client_epoch: 1, sequence_number: 42)
  │
  ├── Server executes and saves revision 10 -> returns receipt
  └── MCP response dropped due to pipe glitch
Caller retries SAME request: (client_id: "agent-1", client_epoch: 1, sequence_number: 42)
  │
  ├── CommandService recognizes s <= W("agent-1", 1)
  ├── Returns prior CommandReceipt (deduplicated_noop)
  └── Zero duplicate mutation executed!
```

**Rules for MCP Mutations:**
1. **Mandatory Caller Identity for `tinixcut_apply`**: The caller must supply a valid structured identity:
   - `client_id` (string $\le 64$ chars)
   - `client_epoch` (`u64`)
   - `sequence_number` (`u64`, monotonic $1, 2, 3\dots$)
2. **Prohibition of Auto-Generated Sequences**: The MCP adapter must **NEVER** silently generate a fresh sequence number or client UUID when handling a tool retry. If a mutation succeeds on disk but the response is lost, retrying with a new sequence number would bypass ADR-001 and duplicate the edit.
3. **Passthrough to CommandService**: The caller's structured identity, `transaction_id`, `base_revision`, and `project_id` must reach `CommandService` completely unmodified.
4. **Dry-Run Isolation**: `tinixcut_dry_run` does not allocate or advance client sequence numbers and appends nothing to the project ledger.
5. **No Second Token Store**: The MCP adapter maintains no local database, SQLite file, or cache for idempotency. The canonical `.tcproj` `command_ledger` remains the single source of truth.

### 3.11 Error Mapping Boundary

TinixCut machine-readable errors must remain fully visible and actionable to AI agents:

```text
1. Malformed JSON-RPC / Unsupported Method / Invalid Argument Types:
   --> Return JSON-RPC Protocol Error (e.g. InvalidParams -32602)

2. Valid MCP Tool Call Rejected by Domain / Application Logic:
   --> Return Tool Result with `isError: true`
   --> Content contains structured JSON matching TinixCut error schema:
       {
         "code": "STALE_REVISION" | "IDEMPOTENCY_COLLISION" | "OPERATION_ALREADY_COMMITTED"
                 | "CLIENT_EPOCH_EXPIRED" | "SEQUENCE_GAP_DETECTED" | "INSUFFICIENT_TRANSITION_HANDLE"
                 | "OPERATION_VALIDATION_FAILED" | "PROJECT_LOAD_FAILED" | ...,
         "message": "Human readable context",
         "details": { ... }
       }

3. Unrecoverable Adapter Panic / Internal Server Invariant:
   --> Return MCP Internal Error (-32603)
```

**Key Invariant**: Domain rejections (e.g. stale revision or sequence gap) are valid execution responses communicating domain state; they must never be collapsed into generic, unparseable string errors.

### 3.12 Export Job Lifecycle

Export rendering remains asynchronous and snapshot-isolated via `JobManager`:
- `tinixcut_export_start(projectPath, outputPath, profile)`: Loads project snapshot, submits job to `JobManager`, spawns background render thread, and returns immediately with `jobId`.
- `tinixcut_export_status(jobId)`: Queries `JobManager` for current status (`queued`, `running`, `completed`, `failed`, `cancelled`), progress ($0.0 \dots 1.0$), and error descriptions.
- `tinixcut_export_cancel(jobId)`: Signals cancellation flag to running export pipeline.

**Process-Local Lifecycle:**
The `jobId` is an explicit application handle valid for the lifetime of the `tinixcut mcp` process. If the MCP server process exits or is killed, running child jobs terminate according to standard OS process cleanup. No remote persistent task queues (Celery, Redis) or complex MCP Tasks extensions are introduced.

### 3.13 Security & Isolation

- **Local User Authority**: The `tinixcut mcp` subprocess runs under the operating system permissions of the calling user.
- **Zero Authority Expansion**: The MCP tool surface must never expose capabilities beyond TinixCut's core video editing domain.
- **Disallowed Tools**:
  - NO arbitrary shell command execution;
  - NO native process spawning outside configured FFmpeg paths;
  - NO general filesystem browsing or arbitrary directory enumeration;
  - NO arbitrary file deletion or manipulation tools;
  - NO raw FFmpeg filtergraph construction strings bypassing domain validation.
- All file paths (`projectPath`, media assets, `outputPath`) undergo standard domain validation.

### 3.14 Standard I/O Stream Discipline

Because `stdout` is the transport medium for MCP JSON-RPC frames:
1. **Exclusive Ownership of Stdout**: The MCP protocol engine has exclusive ownership of `stdout`.
2. **Diagnostic Redirection**: All application tracing, log messages (`tracing`, `log`), FFmpeg resolver messages, and fallback notifications must write exclusively to `stderr`.
3. **Panic Safety**: A top-level panic hook must redirect any panic traces to `stderr` to prevent corrupted frames on `stdout`.
4. **Spawned-Binary Verification**: Phase 15 acceptance testing must execute a real spawned subprocess (`tinixcut mcp`) and assert 100% stdout protocol cleanliness under valid, invalid, and high-frequency calls.

---

## 4. Deferred Scope & Explicit Non-Goals

The following capabilities are explicitly deferred from Phase 15:

1. **Streamable HTTP & Remote Transport**: No HTTP endpoints, SSE streams, WebSocket listeners, or network port binding.
2. **Authentication Infrastructure**: No OAuth 2.0, PKCE, bearer tokens, or API key management.
3. **MCP Non-Tool Primitives**: No Resources, Prompts, Tasks, Sampling, Roots, Subscriptions, or MCP Apps.
4. **Remote / Distributed Render**: No distributed rendering nodes, worker pools, or cloud storage sync.
5. **Project Handle / Caching Layer**: In-memory caching of open projects across calls is deferred until profiling proves file reloading is an actual bottleneck.
6. **Multi-User Collaboration**: No operational transformation (OT), CRDTs, or concurrent multi-writer project editing.
7. **Direct AI / LLM Provider Integration**: TinixCut does not embed LLM clients, API keys, or provider SDKs; it remains a tool provider to external agents.
8. **Per-Path Lock Optimization & Distributed Locking**: Per-path mutex registries are deferred (Phase 15 uses a single process-local write gate shared by the MCP server); no distributed or cross-process locking is introduced.

---

## 5. Candidate Dependencies & Version Strategy

Implementation of Phase 15 will require introducing minimal Rust dependencies. Dependencies must remain candidate-only until implementation begins:

1. **`rmcp` 3.x**: Official Rust SDK for Model Context Protocol (`modelcontextprotocol/rust-sdk`).
   - Features: `stdio`, server primitives.
   - Target Protocol: `2026-07-28`.
2. **`schemars`**: JSON Schema 2020-12 generation compatible with `rmcp`.
   - Use the exact version re-exported by or matching `rmcp` 3.x.
   - Do NOT independently pin legacy `schemars 0.8`.
3. **`tokio`**: Async runtime for `rmcp` server execution.
   - Minimal required features only: `rt-multi-thread` (or `rt`), `io-std`, `macros`, `sync`.
   - Avoid pulling unnecessary network / socket features.

*Note: No changes to `Cargo.toml` are made during this reconciliation gate. Dependencies will be locked in the Phase 15 implementation plan.*

---

## 6. Future Phase-15 Acceptance Test Contract

When Phase 15 implementation is planned and executed, the acceptance suite must verify:

1. **Protocol Discovery**: Client can issue `server/discover` and receive valid `2026-07-28` capabilities.
2. **No `initialize` Requirement**: Client can issue tool calls directly without prior `initialize` handshake.
3. **Tool Listing**: `tools/list` returns the exact 8 defined tools with valid JSON Schema 2020-12 input/output schemas.
4. **Inspection & Capabilities**: `tinixcut_inspect` and `tinixcut_capabilities` return accurate project reports.
5. **Dry-Run Validation**: `tinixcut_dry_run` evaluates envelope changes without modifying disk project file.
6. **Transactional Apply & Atomic Save**: `tinixcut_apply` validates, executes, and persists mutations atomically.
7. **Revision Conflict Rejection**: `tinixcut_apply` with stale `base_revision` fails with `STALE_REVISION`.
8. **Durable Idempotency Replay**: Re-submitting the identical `(client_id, client_epoch, sequence_number)` returns the cached receipt with `deduplicated_noop` without re-executing mutations.
9. **Idempotency Collision & Gap Rejection**: Mismatched fingerprints trigger `IDEMPOTENCY_COLLISION`; sequence increments $> 1$ trigger `SEQUENCE_GAP_DETECTED`.
10. **Export Job Lifecycle**: `tinixcut_export_start` returns `jobId`; `tinixcut_export_status` tracks progress; `tinixcut_export_cancel` cancels cleanly.
11. **Machine-Readable Errors**: Domain errors retain exact error codes and details inside `isError: true` tool results.
12. **Protocol Error Handling**: Malformed JSON-RPC or invalid parameters return proper JSON-RPC error codes.
13. **Concurrent Mutation Serialization**: Two concurrent `tinixcut_apply` calls for the same project path serialize safely without file corruption.
14. **Process Stdout Purity**: Real spawned `tinixcut mcp` process outputs 100% valid JSON-RPC frames to stdout; all logs and diagnostics appear exclusively on stderr.
15. **Clean Process Termination**: Process terminates cleanly upon stdin EOF or SIGINT without orphaned child processes or temp file leaks.
