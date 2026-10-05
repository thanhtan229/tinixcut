# plan_phase10.md — Phase 10 Automation Surface

**Project:** TinixCut / Automation-First Desktop Video Editor  
**Phase:** 10 — Automation Surface  
**Plan status:** READY FOR CODEX IMPLEMENTATION  
**Implementation status:** NOT STARTED  
**Research gate:** CLOSED  
**Transport decision:** CLI-first, long-lived machine-readable stdio session  
**MCP:** DEFERRED  
**Acceptance-critical requirement:** idempotency MUST survive canonical save/reopen  
**Date:** 2026-10-04

---

## 1. Objective

Expose TinixCut's existing editing and export semantics to headless/agent workflows without creating a second editor, second reducer, second persistence path, or transport-specific command semantics.

Phase 10 must deliver a local automation surface that can:

1. open an existing TinixCut project headlessly;
2. inspect canonical project state and current revision;
3. discover supported automation capabilities/operations;
4. dry-run the same domain commands used by real apply;
5. apply atomic command transactions through the existing `CommandService`;
6. return structured receipts and stable machine-readable errors;
7. save through the existing atomic project persistence path;
8. close/reopen the project and preserve command idempotency;
9. start/query/cancel exports through the existing `JobManager` / export APIs;
10. operate through a CLI-first transport, including a long-lived JSONL stdio session;
11. keep stdout protocol-clean and stderr diagnostic-only;
12. preserve all existing editor behavior outside the automation surface.

The phase is complete only when the automation E2E proves the required inspect → dry-run → apply → save/reopen → retry → stale-revision → export flow.

---

## 2. Context and Durable State

### 2.1 Prior phase state

Phase 9 is accepted as `COMPLETE WITH DEFERRED ITEMS`.

Phase 9 deferred items remain deferred and MUST NOT be pulled into Phase 10:

- adjacent-cut / overlap-based `CrossDissolve`;
- non-linear Bezier/custom easing keyframes;
- multi-channel / spectral waveform visualization.

Phase 10 is not a creator-feature expansion phase.

### 2.2 Research decision

The Phase 10 research gate is closed with the following decision:

- initial automation transport is CLI-first;
- the primary machine transport is a long-lived stdio session;
- requests/responses are machine-readable JSONL;
- stdout is reserved for protocol frames;
- stderr is reserved for diagnostics/logging;
- the session maintains an active in-memory project/command/job context;
- MCP is deferred;
- future MCP must be a thin adapter over the same application services.

### 2.3 Repo-verification assumptions accepted for this phase

The research/repo-verification gate has established that the phase can proceed without a major architecture change.

Codex should treat the following as existing-state constraints and verify actual symbol/file names before editing:

- the authoritative `CommandService` is reusable outside React/UI code;
- dry-run and apply already share the same command validation/reducer path;
- project revision, transaction, operation ID, stale revision, and atomic command semantics already exist;
- canonical load/save can be invoked headlessly;
- project persistence uses an existing atomic save path;
- the existing export path is integrated with a reusable local `JobManager`;
- export job state can be queried/cancelled without inventing a second job system;
- structured domain/job errors are available or can be adapted without changing their semantic meaning;
- a headless long-lived session can own the same application services used by the UI without depending on a Tauri window;
- stdout/stderr separation is feasible for the CLI transport.

If implementation reality contradicts any of these assumptions, Codex MUST stop the contradictory implementation, report the evidence, and propose the smallest plan correction rather than creating a parallel path.

---

## 3. In Scope

### 3.1 Headless application/session host

Create the smallest reusable headless host required to own:

- one active canonical project session;
- the existing `CommandService`;
- existing history/revision/idempotency state;
- canonical project persistence services;
- the existing local `JobManager`;
- export services needed by Phase 10.

The headless host is an application adapter, not a new domain layer.

### 3.2 Inspect/capability API

Expose structured reads for:

- project identity;
- schema version;
- project revision;
- project settings relevant to automation;
- tracks;
- timeline items/clips;
- assets;
- supported command operations/capabilities;
- export/job capabilities needed by this phase.

### 3.3 Dry-run/diff API

Expose the existing command-engine dry-run through the automation surface.

It must:

- use the same command registry and validators as apply;
- validate `projectId`, `baseRevision`, operation types and parameters;
- perform no authoritative project mutation;
- perform no persistence;
- perform no durable idempotency-ledger mutation;
- return structured change summary/warnings/errors.

### 3.4 Apply API

Expose authoritative transactions through the existing `CommandService`.

Support:

- `projectId`;
- `baseRevision`;
- `transactionId`;
- stable `operationId` values;
- one or more typed operations;
- atomic transaction semantics;
- structured receipts;
- revision before/after;
- changed entity IDs or a stable change summary where already supported.

### 3.5 Durable idempotency across save/reopen

This is acceptance-critical and is a hard Phase 10 gate.

A successfully applied automation operation/transaction MUST remain idempotent after:

1. successful apply;
2. canonical save;
3. process/session termination if applicable;
4. reopening the saved project;
5. retrying the identical request with the same idempotency identity.

A process-memory-only `HashMap`, CLI cache, session cache, temporary file, or transport-specific receipt store is NOT sufficient.

### 3.6 CLI transport

Provide:

- a standalone headless CLI entrypoint;
- one-shot automation commands where useful;
- a long-lived machine-readable stdio session;
- JSONL request/response framing;
- protocol-safe stdout;
- diagnostics only on stderr;
- deterministic process exit behavior.

### 3.7 Export automation

Expose the existing export job path for:

- start export;
- query job status/progress;
- cancel export;
- return completed/failed/cancelled state;
- return stable structured errors.

### 3.8 Automation E2E

Add an end-to-end test covering the complete Phase 10 acceptance flow, including a real save/reopen boundary before retrying the same operation ID.

---

## 4. Out of Scope

Do NOT implement the following in Phase 10:

- MCP server;
- built-in AI agent/provider integration;
- cloud automation;
- remote automation server;
- HTTP/WebSocket automation daemon unless required by an existing test harness;
- multi-user/collaborative editing;
- distributed job queues;
- Redis/RabbitMQ/Kafka;
- arbitrary shell execution;
- arbitrary filesystem enumeration;
- generic scripting language;
- plugin SDK;
- macro recorder;
- UI mouse/keyboard simulation;
- project-file direct editing from CLI;
- a CLI-specific timeline reducer;
- a CLI-specific export pipeline;
- Phase 11 hardening/performance qualification beyond regressions introduced by Phase 10;
- Phase 9 deferred creator features;
- new editor feature semantics merely to make automation demos richer.

---

## 5. Architecture Constraints

These constraints are non-negotiable for Phase 10.

### P10-A1 — One mutation path

Every automation mutation MUST invoke the same authoritative `CommandService` semantics used by the editor.

Forbidden:

- directly mutating project structs from CLI handlers;
- editing serialized project JSON;
- duplicating reducers;
- bypassing validation;
- transport-specific edit logic.

### P10-A2 — Automation is a client

The transport flow is:

```text
CLI / stdio request
    -> automation adapter
    -> existing application/domain services
    -> CommandService / JobManager / persistence
    -> structured response
```

Not:

```text
CLI
    -> special CLI project model
    -> direct file mutation
```

### P10-A3 — Canonical state remains application-owned

The CLI/session does not become a new project source of truth.

### P10-A4 — Canonical time remains canonical

Automation payloads must use the existing canonical time representation or an existing typed boundary representation that deterministically converts into it.

Do not introduce arbitrary float seconds as authoritative command state.

### P10-A5 — Atomic command semantics remain unchanged

A failed atomic transaction leaves canonical project state unchanged.

A failed transaction must also leave durable idempotency state unchanged.

### P10-A6 — Stale revision is explicit

Unseen operations with stale `baseRevision` must be rejected with a stable machine-readable conflict/error.

Do not silently rebase.

### P10-A7 — Durable idempotency precedes ordinary retry validation

An exact retry of an already-successful request must be recognized as a replay before the old `baseRevision` causes a stale-revision failure.

This is required because an exact retry after a successful prior apply naturally carries the original `baseRevision`.

### P10-A8 — Security boundary

Automation must not receive:

- unrestricted shell;
- arbitrary process spawn;
- arbitrary filesystem enumeration;
- raw FFmpeg command execution;
- direct project-file write authority.

All export/media process spawning remains inside trusted native services with typed arguments.

### P10-A9 — Existing atomic save path

Automation save must reuse the canonical atomic save implementation.

Do not add a CLI-only serializer or save implementation.

### P10-A10 — Existing JobManager

Export automation must use the existing local job lifecycle and cancellation behavior.

Do not introduce a second queue.

---

## 6. Data and State Contracts

### 6.1 Protocol version

Define an explicit protocol version for the machine-readable CLI surface.

Minimum:

```text
protocolVersion = 1
```

Protocol versioning is transport-level compatibility metadata. It must not replace project `schemaVersion`.

### 6.2 JSONL request envelope

Use one complete JSON object per input line.

Recommended v1 shape:

```json
{
  "protocolVersion": 1,
  "requestId": "req-123",
  "method": "project.inspect",
  "params": {}
}
```

Requirements:

- `requestId` is transport correlation identity, not command idempotency identity;
- `method` selects a reviewed typed automation operation;
- `params` must deserialize into typed request DTOs;
- malformed requests must not crash the session;
- one malformed line must not corrupt project state;
- unknown protocol/method must return a structured error.

Exact Rust type names may follow repository conventions.

### 6.3 JSONL success response

Recommended shape:

```json
{
  "protocolVersion": 1,
  "requestId": "req-123",
  "ok": true,
  "result": {}
}
```

### 6.4 JSONL error response

Recommended shape:

```json
{
  "protocolVersion": 1,
  "requestId": "req-123",
  "ok": false,
  "error": {
    "code": "STALE_REVISION",
    "message": "human-readable diagnostic",
    "details": {}
  }
}
```

Requirements:

- `code` is stable/machine-readable;
- `message` may evolve;
- `details` may carry structured context;
- domain errors should retain their existing semantic category;
- transport parse/protocol errors should be distinguishable from domain errors.

### 6.5 Session ownership

Use the simplest session model consistent with the research decision:

- one CLI process may maintain one active project session at a time;
- opening a project constructs/loads the canonical project session through existing application services;
- command requests operate against that active session;
- export jobs are owned/tracked by the existing `JobManager`;
- session shutdown performs defined cleanup;
- do not invent multi-project concurrency unless already trivial and required.

### 6.6 Project inspect contract

The inspect response should expose only canonical/project-known state and reviewed capabilities.

At minimum:

```text
project:
  projectId
  schemaVersion
  revision
  settings summary

tracks[]
items/clips[]
assets[]
capabilities:
  supported command operation types
  dryRun/apply support
  export job support
```

Requirements:

- preserve stable IDs;
- do not expose internal Rust handles;
- do not expose FFmpeg filtergraphs/command lines as canonical semantics;
- do not expose arbitrary filesystem browsing APIs;
- project-known asset locators may be represented only according to the existing project/security policy.

### 6.7 Command transaction contract

Automation apply/dry-run must use the existing conceptual command envelope:

```text
projectId
baseRevision
transactionId
operations[]
  operationId
  type
  targetId / typed parameters
```

Do not create a second CLI-only command syntax if the existing typed command model can be serialized/adapted.

### 6.8 Receipt contract

A successful apply response must expose enough stable data to support audit and exact replay recognition.

At minimum, where supported:

- transaction ID;
- operation IDs;
- project ID;
- revision before;
- revision after;
- applied/no-op status;
- changed entity IDs or change summary;
- warnings;
- job-independent domain errors when relevant.

The transport may adapt internal receipts into serializable DTOs but may not change command meaning.

---

## 7. Acceptance-Critical Durable Idempotency Design

### 7.1 Required semantic guarantee

After a successful command is saved and the project is reopened:

```text
retry same idempotency identity + same semantic request
    => no second mutation
    => no revision increment
    => return/reconstruct the prior receipt
```

This guarantee applies across a real canonical persistence boundary.

### 7.2 Durable storage requirement

Persist enough idempotency metadata through the canonical project save/load path to recognize previously committed automation operations/transactions after reopen.

The durable state MUST be owned by the command/project layer, not by CLI transport.

Preferred implementation direction:

- project-owned, versioned command receipt/idempotency metadata;
- serialized with canonical project persistence;
- loaded with the project before accepting automation commands.

If the repository already has a durable command receipt structure, extend/reuse it.

If the project schema has a supported project-owned extension metadata area, it may be used only if it provides deterministic versioned persistence and does not create transport-specific semantics.

If a schema change is required:

- bump/extend schema according to existing repository migration rules;
- add deterministic migration from existing project version/state;
- old projects load with an empty idempotency ledger;
- migration failure must not overwrite the original project.

Do NOT use a CLI-local sidecar database merely to avoid a schema/persistence change.

### 7.3 Durable receipt identity

Persist sufficient identity to distinguish:

#### Exact replay

Same operation/transaction identity and same semantic request.

Result:

- return prior receipt;
- no reducer execution;
- no history entry;
- no revision increment;
- no persistence mutation required solely for replay.

#### Identity collision

Same `operationId` or `transactionId`, but a different semantic request/payload.

Result:

- reject with a stable idempotency-key-reuse/conflict error;
- no mutation.

Do not silently accept a reused key for a different operation.

### 7.4 Request fingerprint

Use a deterministic request fingerprint/equivalent semantic identity mechanism so an existing ID cannot be reused with different content.

The fingerprint must derive from canonical/typed command content, not from unstable JSON field ordering or human-readable display strings.

Do not hash raw non-canonical JSON if equivalent requests can serialize differently.

### 7.5 Validation order

For apply:

1. parse/validate the transport envelope;
2. validate project identity/session identity;
3. check durable idempotency/transaction replay state;
4. if exact committed replay, return/reconstruct prior receipt immediately;
5. if ID collision with different semantic request, reject;
6. for unseen requests, enforce `baseRevision`;
7. run normal command validation/preconditions;
8. apply atomically through `CommandService`;
9. on success, record receipt/idempotency metadata as part of the authoritative in-memory project state;
10. increment revision according to existing command policy;
11. persist only when explicitly requested by the existing save contract / one-shot command policy.

This ordering is essential for save/reopen retry behavior.

### 7.6 Atomic transactions and replay

For multi-operation atomic transactions:

- persist transaction-level replay identity/receipt or equivalent;
- persist operation IDs consistently with the successful transaction;
- an exact retry of the entire committed transaction returns the prior transaction receipt;
- a failed atomic transaction records no successful operation receipts;
- no partial ledger commit is allowed.

For a new transaction that reuses an operation ID already committed by a different transaction:

- reject as idempotency conflict;
- do not partially execute unseen operations.

Do not implement "some operations replayed, some newly executed" inside one atomic transaction in Phase 10.

### 7.7 Dry-run behavior

Dry-run:

- may inspect existing idempotency state for validation/reporting if necessary;
- MUST NOT add durable idempotency receipts;
- MUST NOT increment revision;
- MUST NOT create history entries;
- MUST NOT persist project changes.

### 7.8 No-op behavior

Follow the existing `CommandService` no-op policy.

If a successful no-op currently returns an idempotent receipt, preserve/replay that receipt after reopen.

Do not invent a second no-op rule solely for CLI.

### 7.9 Undo/redo interaction

Idempotency identity represents a historical command request, not "current effect exists."

Therefore:

- if a command was applied successfully and later undone, retrying the same committed `operationId` must not silently apply it again;
- return/reconstruct the prior receipt according to existing history semantics;
- do not turn retry into redo.

Add a targeted test if the current history architecture makes this path practical.

### 7.10 Retention

Do not implement unsafe receipt pruning in Phase 10.

If a bounded-retention scheme is proposed, it MUST preserve the Phase 10 idempotency guarantee and must be explicitly reviewed before implementation.

Prefer correctness over premature compaction.

Performance/storage optimization of a growing receipt ledger may be deferred to Phase 11 only if no correctness guarantee is weakened.

---

## 8. Required Implementation Work

### Workstream 10.0 — Baseline and Phase Guardrails

Before code changes:

1. read:
   - `PLAN.md` Phase 10;
   - relevant `ARCHITECTURE.md` sections;
   - `PROJECT_STATUS.md`;
   - repository development workflow;
   - current command/persistence/job tests;
2. identify actual modules/symbols for:
   - `CommandService`;
   - project load/save;
   - schema/migrations;
   - history;
   - job manager;
   - export service;
   - structured errors;
3. record any discrepancy from repo-verification assumptions;
4. do not modify code until the implementation path clearly reuses existing services.

Deliverable:

- no production artifact required;
- Codex report must list the reused modules and any necessary adapters.

### Workstream 10.1 — Headless Application Session

Create a small application-level session/host that can run without React/Tauri window state.

Responsibilities:

- load/open canonical project;
- own/access `CommandService`;
- expose current revision;
- call canonical save;
- access `JobManager`;
- access export start/status/cancel;
- close/cleanup project session.

Constraints:

- no duplicate project model;
- no duplicate history;
- no duplicate revision counter;
- no direct project-file mutation;
- no dependency on browser/WebView state.

Tests:

- open fixture headlessly;
- inspect project ID/revision;
- apply an existing command through the host;
- verify canonical state matches a non-CLI caller for the same command;
- close/reopen through existing persistence.

### Workstream 10.2 — Inspect and Capability Surface

Implement typed automation queries.

Minimum methods/capabilities:

- project summary/inspect;
- tracks;
- items/clips;
- assets;
- current revision;
- supported operation/capability discovery.

Prefer a small coherent surface.

Do not expose internal implementation types directly if they are unstable/non-serializable; introduce thin DTOs at the adapter boundary.

Tests:

- inspect is read-only;
- revision unchanged;
- stable IDs preserved;
- canonical time values are deterministic;
- unsupported/internal-only fields are not leaked;
- capabilities correspond to actual registered/supported command operations.

### Workstream 10.3 — Dry-Run / Diff Adapter

Expose the existing dry-run path.

Requirements:

- deserialize typed command request;
- invoke the same command registry/validators as apply;
- return validation errors;
- return stale revision for unseen stale requests;
- return change summary;
- return warnings;
- no write/history/revision/receipt persistence.

Tests:

- valid dry-run returns expected summary;
- invalid target returns structured error;
- unsupported operation returns structured error;
- stale revision returns conflict;
- state before/after is canonically equivalent;
- revision unchanged;
- save output unchanged;
- durable idempotency ledger unchanged.

### Workstream 10.4 — Apply Adapter and Structured Receipts

Expose atomic apply through existing `CommandService`.

Requirements:

- `projectId`;
- `baseRevision`;
- `transactionId`;
- operation IDs;
- typed operations;
- atomic mode;
- structured receipt.

Do not add CLI-specific domain operations.

Tests:

- successful single operation;
- successful multi-op transaction;
- revision increment;
- failed atomic transaction changes nothing;
- history receives one logical transaction according to existing semantics;
- result state matches an equivalent UI/internal caller.

### Workstream 10.5 — Durable Idempotency Persistence

Implement/extend the command receipt persistence required for save/reopen idempotency.

Required behavior:

1. apply operation/transaction successfully;
2. retain receipt identity in authoritative in-memory state;
3. canonical save includes durable receipt/idempotency metadata;
4. reopen restores it;
5. retry identical request returns/reconstructs prior receipt;
6. no edit is duplicated;
7. revision does not change;
8. same ID + different command is rejected.

Required migration/compatibility work:

- old project without ledger loads normally;
- empty ledger is deterministic default;
- existing projects remain editable;
- save/reopen round trip preserves ledger;
- migration follows existing project safety rules.

Prohibited implementation:

- process-only map as sole source;
- CLI cache directory as sole source;
- transport-local SQLite/DB as sole source;
- relying on current revision alone to infer replay;
- storing only operation ID without enough semantic identity to detect collision.

Tests in Section 11 are mandatory.

### Workstream 10.6 — Canonical Save/Open Integration

Expose project lifecycle needed by automation while preserving persistence semantics.

Required operations:

- open;
- save;
- optionally save-as only if already safe/easy through existing service;
- close/shutdown cleanup.

Save must:

- call the existing atomic save path;
- serialize project state plus required durable idempotency state;
- preserve previous valid file on save failure according to existing persistence rules.

Tests:

- apply → save → reopen semantic equivalence;
- idempotency metadata survives;
- interrupted/failed save behavior is not regressed;
- CLI does not write project JSON directly.

### Workstream 10.7 — CLI One-Shot Commands

Provide the smallest useful one-shot interface.

Suggested functional groups:

```text
inspect
dry-run
apply
export
```

Exact command spelling should follow repository CLI conventions if any exist.

One-shot commands must be wrappers over the same automation dispatch/application session used by long-lived stdio.

Do not duplicate handlers.

For mutating one-shot commands:

- load through canonical project open;
- execute through `CommandService`;
- save through canonical atomic save when the command contract says to persist;
- return non-zero exit status on machine-relevant failure;
- write machine output to stdout only.

Add one E2E test that performs apply in one process and retry in a fresh process after reopen, if the test infrastructure supports subprocess tests reliably.

### Workstream 10.8 — Long-Lived JSONL Stdio Session

Implement the primary Phase 10 transport.

Requirements:

- read one JSON request per stdin line;
- emit exactly one correlated JSON response per accepted request;
- flush stdout after each response;
- never write human logs to stdout;
- diagnostics/logging go to stderr;
- malformed line returns protocol error without killing healthy session unless input stream/process state is unrecoverable;
- session maintains active canonical project/services;
- export jobs continue through the existing async job manager while the session remains alive;
- support clean shutdown.

Recommended methods:

```text
session.open
session.close
session.shutdown

project.inspect
project.capabilities
project.save

command.dryRun
command.apply

export.start
export.status
export.cancel
```

Naming may be adjusted to existing code style, but the semantic groups must remain clear.

Concurrency policy for Phase 10:

- process requests deterministically;
- do not introduce concurrent project mutations unless existing `CommandService` already defines safe serialization;
- export jobs may run asynchronously through `JobManager`;
- inspection/status calls must not mutate project state.

Tests:

- multiple sequential requests in one session;
- request/response correlation;
- stdout contains valid JSONL only;
- stderr diagnostic output does not corrupt protocol;
- malformed request recovery;
- shutdown cleanup.

### Workstream 10.9 — Export Job Automation

Reuse the existing export/job path.

Expose:

#### start

Input:

- output destination according to existing safe path policy;
- existing export profile or supported profile selector;
- current/specified project revision according to existing export snapshot semantics.

Output:

- job ID;
- accepted state;
- snapshot/revision metadata where already supported.

#### status

Return:

- queued/running/completed/failed/cancelled;
- progress;
- actionable stable failure category;
- output info only when valid/safe.

#### cancel

- explicit cancellation;
- no silent supersession;
- cleanup remains owned by existing job/export code.

Tests:

- start export via automation;
- observe running/completed state;
- cancel path;
- invalid destination/unsupported export returns structured error;
- export uses a revision snapshot and is not silently altered by subsequent edits;
- no raw FFmpeg command is accepted from automation.

### Workstream 10.10 — Error Model and Protocol Mapping

Map existing domain/job/persistence errors into stable automation errors.

At minimum preserve/distinguish:

- validation error;
- stale revision;
- entity not found;
- unsupported operation;
- idempotency key reused with different request;
- session/project not open;
- project identity mismatch;
- permission/path error;
- missing/offline asset where relevant;
- job cancelled;
- render failure;
- corrupted project;
- internal invariant violation;
- malformed JSON/protocol error;
- unsupported protocol version/method.

Do not leak Rust debug dumps or stack traces into stable machine-readable fields.

Detailed developer diagnostics may go to stderr in debug/dev modes.

### Workstream 10.11 — Security Review for the New Surface

Verify Phase 10 does not accidentally expand authority.

Required checks:

- CLI automation cannot execute arbitrary shell commands;
- no arbitrary process spawn endpoint;
- no raw FFmpeg argument endpoint;
- no generic filesystem list/read/write API;
- project path/output path handling goes through existing validated native APIs;
- untrusted JSON strings are never concatenated into shell commands;
- protocol input size/pathological requests follow reasonable existing limits where practical;
- project file content cannot define executable commands;
- stdout never leaks secrets through accidental diagnostics;
- MCP code/dependencies are not introduced.

Any broad capability accidentally introduced by a generic helper is a phase blocker.

### Workstream 10.12 — Documentation and Project State

At completion, update durable project state.

Required:

- `PLAN.md` Phase 10 status/result;
- `PROJECT_STATUS.md`;
- relevant developer workflow/CLI usage docs if the repo has them;
- protocol usage documentation sufficient for another agent to run the CLI;
- document the idempotency-across-reopen guarantee and collision behavior;
- record any deferred work/technical debt.

Update `ARCHITECTURE.md` only if implementation changes an architecture decision/boundary.

Do not create an ADR for routine implementation of already-approved CLI-first semantics.

If durable idempotency implementation introduces a genuinely new cross-cutting architecture decision beyond the existing command/persistence model, report it before finalizing the phase and update architecture/ADR as required.

---

## 9. Expected Files / Modules Affected

Codex must use the actual repository paths discovered during repo-verification. Do not invent duplicate layers merely to match this list.

Expected areas:

- existing project/domain model;
- existing command service/registry;
- existing command receipts/idempotency support;
- existing project serialization and migration code;
- existing atomic load/save service;
- existing application/session/service composition layer;
- existing job manager;
- existing export service/backend adapter;
- structured error types;
- a new or existing headless CLI binary/entrypoint;
- automation protocol DTO/dispatcher module;
- Rust unit/integration tests;
- automation E2E/subprocess tests;
- project status/developer documentation.

Prefer adding a thin automation adapter around existing services over moving/restructuring stable modules.

Large refactors are out of scope unless required to expose existing services headlessly.

---

## 10. Error and Edge-Case Handling

Codex must explicitly handle the following.

### 10.1 No active project

Inspect/apply/save/export should return a structured session error.

### 10.2 Wrong project identity

A command envelope targeting a different `projectId` must be rejected.

### 10.3 Stale revision

An unseen command with old `baseRevision` returns explicit conflict.

### 10.4 Exact retry after commit

Return/reconstruct prior receipt before stale-revision rejection.

### 10.5 Same operation ID, different payload

Reject; no mutation.

### 10.6 Same transaction ID, different transaction

Reject; no mutation.

### 10.7 Previously committed operation ID reused in a new transaction

Reject the new transaction before mutation.

### 10.8 Partial/failed transaction

No canonical state change and no durable successful receipts.

### 10.9 Dry-run

No revision/history/persistence/idempotency mutation.

### 10.10 Malformed JSONL request

Return protocol error; keep session alive where safe.

### 10.11 Unsupported protocol version/method

Return explicit machine error.

### 10.12 Save failure

Preserve existing persistence guarantees; do not report durable success incorrectly.

### 10.13 Reopen old project

Project without Phase 10 ledger metadata loads with deterministic empty ledger/migrated state.

### 10.14 Export already terminal

Status remains deterministic; cancel on terminal job follows existing job policy.

### 10.15 Session termination with running export

Follow existing application/job cleanup policy; do not invent unsafe background daemon behavior.

### 10.16 stdout contamination

Any non-protocol stdout output is a test failure for session mode.

---

## 11. Testing Strategy

Testing must be added at the lowest practical layer plus automation E2E.

### 11.1 Unit tests — protocol/DTO

Cover:

- request parsing;
- response serialization;
- unsupported version;
- unsupported method;
- malformed JSON;
- stable error mapping;
- stdout response shape where practical.

### 11.2 Unit tests — durable idempotency

Mandatory:

1. record successful operation receipt;
2. same ID + same semantic fingerprint -> exact replay;
3. same ID + different fingerprint -> conflict;
4. failed transaction -> no receipts;
5. dry-run -> no receipts;
6. transaction exact replay;
7. transaction ID collision;
8. old project/default empty ledger;
9. receipt serialization/deserialization round trip.

### 11.3 Command integration tests

Mandatory:

- CLI/application adapter and existing internal caller produce equivalent canonical state for the same operation;
- stale revision rejection;
- atomic rollback;
- exact replay does not increment revision;
- exact replay does not create history;
- ID collision does not mutate.

### 11.4 Persistence integration tests — ACCEPTANCE CRITICAL

Mandatory sequence:

```text
load fixture
inspect revision R
apply operation op-A at R
verify revision R+1
save canonically
destroy/drop in-memory session
reopen saved project
retry exact op-A request
verify:
  returned/reconstructed prior receipt
  no duplicate edit
  revision remains R+1
  history does not gain a new edit
save/reopen again
verify state still equivalent
```

Also mandatory:

```text
reopen saved project
reuse op-A with changed payload
verify:
  idempotency conflict
  no mutation
  revision unchanged
```

For a multi-operation transaction:

```text
apply tx-A(op-A1, op-A2)
save
reopen
retry exact tx-A
verify no duplicate mutation
```

If subprocess E2E is reliable, perform at least one of these across two separate CLI process lifetimes, not merely two Rust objects in one process.

### 11.5 Inspect tests

- read-only;
- deterministic stable IDs;
- revision correct;
- capabilities correct;
- no internal handles.

### 11.6 Dry-run tests

- same validation path as apply;
- valid change summary;
- stale revision;
- invalid target;
- no write;
- no receipt persistence.

### 11.7 CLI/stdin-stdout tests

- one-shot inspect;
- one-shot dry-run;
- one-shot apply;
- long-lived session multiple requests;
- stdout valid JSONL only;
- diagnostics on stderr;
- request correlation;
- clean EOF/shutdown;
- malformed input recovery.

### 11.8 Export automation tests

- start/status/completion;
- cancel;
- failed export structured error;
- job ID stable;
- output safety unchanged;
- no direct FFmpeg command injection.

### 11.9 Regression validation

Run repository-standard:

- Rust format;
- Rust lint/clippy if part of repo workflow;
- Rust tests;
- frontend tests where shared contracts changed;
- TypeScript typecheck where frontend/shared generated types changed;
- build;
- existing persistence/recovery tests;
- existing command engine tests;
- existing export tests;
- existing preview/export parity tests if shared export code changes.

Do not weaken existing tests.

---

## 12. Acceptance Criteria

Phase 10 MUST NOT be marked complete unless every applicable critical criterion below passes.

### P10-AC-01 — Headless reuse

Automation can open a project and operate without React/Tauri UI state while using the existing application/domain services.

### P10-AC-02 — Inspect

Automation returns structured project/tracks/items/assets/revision/capabilities using stable IDs.

### P10-AC-03 — Dry-run parity

Automation dry-run invokes the same validators/reducers as real apply and writes nothing.

### P10-AC-04 — Apply through CommandService

No authoritative automation edit bypasses the existing `CommandService`.

### P10-AC-05 — Atomic transaction

Failed atomic transaction changes neither project state nor durable idempotency state.

### P10-AC-06 — Stale revision

An unseen stale request is deterministically rejected.

### P10-AC-07 — In-session idempotency

Retrying an already-successful exact operation in the same session does not mutate twice.

### P10-AC-08 — SAVE/REOPEN IDEMPOTENCY — CRITICAL

After successful apply + canonical save + complete reopen, retrying the identical operation/transaction ID and semantic request:

- does not execute the edit again;
- does not increment project revision;
- does not create another history entry;
- returns/reconstructs the prior receipt.

Failure of this criterion means Phase 10 is `INCOMPLETE`.

It may NOT be deferred.

### P10-AC-09 — ID collision safety — CRITICAL

After save/reopen, reusing a committed operation/transaction ID with different semantic content is rejected and changes nothing.

### P10-AC-10 — Old-project compatibility

Projects without Phase 10 idempotency metadata open safely with deterministic migration/default behavior.

### P10-AC-11 — Canonical save path

Automation saves only through the existing atomic project persistence implementation.

### P10-AC-12 — Long-lived stdio

A long-lived JSONL stdio session supports inspect, dry-run, apply, save, and export job requests.

### P10-AC-13 — Protocol cleanliness

Session stdout contains protocol JSONL only; diagnostics go to stderr.

### P10-AC-14 — Structured errors

Validation, stale revision, idempotency conflict, unsupported operation, protocol errors, persistence errors, and export/job errors are machine-readable.

### P10-AC-15 — Export automation

Automation can start/query/cancel an export through the existing typed `JobManager`/export path.

### P10-AC-16 — Security

No unrestricted shell/process/filesystem/FFmpeg-command capability is exposed.

### P10-AC-17 — UI/automation equivalence

At least one representative edit executed through automation produces canonical state equivalent to the existing internal/UI command caller.

### P10-AC-18 — Required automation E2E

The following scenario passes:

1. open fixture project;
2. inspect revision;
3. dry-run plan;
4. apply plan;
5. save;
6. destroy/close session;
7. reopen project;
8. retry same operation ID;
9. verify no duplicate edit and same revision;
10. submit an unseen request with stale base revision;
11. verify explicit stale conflict;
12. export through job API;
13. query export;
14. verify completion or explicit tested cancellation path.

### P10-AC-19 — Repository health

Applicable tests/typecheck/lint/build pass and no regression in command/persistence/export foundations is known.

### P10-AC-20 — Durable state reconciliation

`PLAN.md` / `PROJECT_STATUS.md` are updated with:

- Phase 10 status;
- implemented automation surface;
- durable idempotency behavior;
- validation performed;
- deferred work;
- technical debt;
- any architecture implications.

---

## 13. Recommended Codex Implementation Order

Follow this order to keep failures localized.

### Step 1 — Identify reusable service boundaries

Confirm actual repo symbols/paths and avoid refactors.

### Step 2 — Solve durable idempotency first

Before building transport polish:

- define durable receipt representation;
- implement serialization/load/migration;
- implement replay/collision behavior;
- add save/reopen tests.

This is the highest-risk acceptance item.

### Step 3 — Add headless application session

Make existing command/persistence/job services callable without UI.

### Step 4 — Add inspect/capability DTOs

Keep them read-only.

### Step 5 — Add automation dry-run/apply dispatcher

Reuse CommandService directly.

### Step 6 — Add one-shot CLI wrapper

Use the dispatcher/session, not separate handlers.

### Step 7 — Add JSONL stdio session

Add protocol framing and stdout/stderr discipline.

### Step 8 — Add export job methods

Reuse JobManager/export services.

### Step 9 — Add full automation E2E

Run across a real save/reopen boundary; prefer separate process lifetimes for the critical retry test.

### Step 10 — Full regression suite and docs

Only after acceptance tests pass.

---

## 14. Manual QA

Automated tests are primary, but perform a focused manual smoke test using the built CLI.

Suggested session:

1. copy a small fixture project;
2. start long-lived CLI session;
3. open fixture;
4. inspect;
5. dry-run one visible timeline change;
6. apply it;
7. save;
8. end the process;
9. start a fresh process;
10. reopen;
11. resend the exact original apply request;
12. verify prior receipt/no duplicate/revision unchanged;
13. inspect state;
14. send same operation ID with different parameters;
15. verify idempotency conflict;
16. start export;
17. poll status;
18. cancel or allow completion;
19. confirm stdout remained parseable JSONL throughout.

Record the command/protocol transcript or equivalent evidence in the Codex completion report without including secrets/user-private paths unnecessarily.

---

## 15. Deferred / Future Work

Explicitly defer:

- MCP transport;
- AI/provider agent integration;
- remote/cloud automation;
- network automation server;
- scripting language;
- plugin SDK;
- richer automation workflow orchestration;
- streaming event subscription protocol beyond what export polling needs;
- multi-project concurrent stdio sessions;
- receipt-ledger compaction/retention optimization unless required for correctness;
- broad performance/security hardening belonging to Phase 11.

Future MCP must reuse this phase's automation application API and must not define new edit semantics.

---

## 16. Known Risks and Mitigations

### Risk P10-R1 — CLI becomes a second editor

Mitigation:

- adapters only;
- no direct mutations;
- UI/automation equivalence test.

### Risk P10-R2 — Idempotency works only in memory

Mitigation:

- durable receipt ledger through canonical persistence;
- mandatory save/reopen test;
- acceptance-critical gate.

### Risk P10-R3 — Retry is rejected as stale after prior success

Mitigation:

- exact durable replay check precedes `baseRevision` rejection for previously committed requests.

### Risk P10-R4 — Reused operation ID applies different content

Mitigation:

- deterministic semantic request fingerprint;
- explicit collision error.

### Risk P10-R5 — Partial transaction receipt corruption

Mitigation:

- update canonical mutation + receipt state atomically;
- failed transaction persists no successful receipts.

### Risk P10-R6 — CLI logs corrupt JSONL

Mitigation:

- stdout protocol-only;
- stderr diagnostics;
- subprocess protocol tests.

### Risk P10-R7 — Transport gains unsafe filesystem/shell authority

Mitigation:

- narrow method registry;
- typed existing services;
- explicit security review.

### Risk P10-R8 — Receipt persistence causes incompatible project changes

Mitigation:

- use existing schema/migration framework;
- old-project fixture;
- save/reopen/migration tests.

### Risk P10-R9 — Automation export bypasses JobManager

Mitigation:

- expose existing typed job API only;
- no direct sidecar invocation from transport.

---

## 17. Completion Report Requirements

Codex must provide a Phase 10 completion report with all of the following.

### 17.1 Summary

- what automation capabilities were implemented;
- actual CLI/session entrypoint;
- protocol version.

### 17.2 Scope completed

Map implementation to Workstreams 10.0–10.12.

### 17.3 Files/modules changed

List actual repository paths.

### 17.4 Reused architecture boundaries

Explicitly identify:

- `CommandService`;
- persistence/save path;
- idempotency receipt storage;
- `JobManager`;
- export service;
- error mapping.

### 17.5 Durable idempotency implementation

Report:

- where durable receipt state is owned;
- how it is serialized;
- migration/default behavior;
- request fingerprint/collision behavior;
- retry validation order;
- transaction replay behavior;
- why save/reopen cannot duplicate a committed edit.

### 17.6 Tests and validation

Include exact commands and pass/fail results for:

- targeted idempotency tests;
- persistence save/reopen test;
- automation E2E;
- stale revision;
- transaction rollback;
- CLI/stdout protocol tests;
- export automation tests;
- repository-standard format/lint/typecheck/tests/build.

### 17.7 Acceptance matrix

Report PASS/FAIL for every `P10-AC-*` criterion.

`P10-AC-08` and `P10-AC-09` must have explicit evidence, not a generic "tests pass" statement.

### 17.8 Manual QA

Report whether the focused manual smoke test was performed and any deviations.

### 17.9 Deviations

Any plan deviation must be explicit.

### 17.10 Deferred work

List only genuinely non-blocking deferred items.

Do not defer save/reopen idempotency.

### 17.11 Technical debt

Record any:

- ledger size concern;
- protocol limitation;
- subprocess/test limitation;
- serialization compatibility compromise;
- cleanup intentionally left for Phase 11.

### 17.12 Architecture implications

State one of:

- no architecture change; or
- architecture update/ADR required, with reason.

### 17.13 Project-state updates

Confirm the exact updates made to:

- `PLAN.md`;
- `PROJECT_STATUS.md`;
- automation/CLI developer documentation;
- architecture/ADR only if required.

### 17.14 Final Codex recommendation

Codex may recommend `COMPLETE`, `COMPLETE WITH DEFERRED ITEMS`, `INCOMPLETE`, or `BLOCKED`, but ChatGPT will independently review the report and repository evidence before accepting the phase.

---

## 18. Phase Completion Decision Rules

### COMPLETE

Use only if:

- every acceptance-critical criterion passes;
- save/reopen idempotency is proven;
- collision safety is proven;
- automation E2E passes;
- build/tests are adequate;
- no blocking security/architecture issue exists.

### COMPLETE WITH DEFERRED ITEMS

Allowed only when deferred items are non-blocking improvements such as:

- protocol ergonomics;
- nonessential one-shot command UX;
- receipt-ledger optimization;
- future MCP preparation.

It is NOT allowed if durable idempotency, stale revision, atomicity, canonical persistence, or export job safety is missing.

### INCOMPLETE

Use when required behavior exists partially but any acceptance criterion remains unmet, especially `P10-AC-08` or `P10-AC-09`.

### BLOCKED

Use only for a real external/architectural dependency that prevents correct implementation without an approved decision.

---

## 19. Phase 10 Exit Statement

Phase 10 exits successfully only when TinixCut has one safe local automation surface whose behavior is equivalent to normal editor semantics:

```text
inspect
  -> dry-run
  -> atomic apply
  -> structured receipt
  -> canonical save
  -> reopen
  -> exact retry without duplicate mutation
  -> explicit stale conflict for unseen stale work
  -> export through existing JobManager
```

The most important Phase 10 correctness property is:

> A successful automation operation is not merely idempotent for the lifetime of one process. Once canonically saved, its idempotency identity survives reopening the project, so an uncertain client retry cannot duplicate the edit.
