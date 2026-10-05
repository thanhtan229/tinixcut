# Phase 11 Plan — Reliability, Security, Performance Hardening + ADR-001 v3

## 0. Plan Metadata

**Project:** TinixCut / Automation-First Desktop Video Editor  
**Phase:** 11 — Reliability, Security, and Performance Hardening  
**Plan status:** APPROVED FOR CODEX IMPLEMENTATION  
**Research gate:** `COMPLETE — READY FOR PLANNING`  
**Authoritative durable-idempotency architecture:** `ADR-001 v3`  
**Production-code status at plan creation:** Do not implement production code as part of planning  
**Primary target:** Windows 10/11, local-first desktop application  

### Normative inputs and precedence

Codex must read, in this order, before implementation:

1. `AGENTS.md` and repository coding-agent instructions.
2. `PLAN.md`, Phase 11 and cross-cutting requirements.
3. `ARCHITECTURE.md`, especially Command Service, persistence, jobs, security, testing, performance, and architecture invariants.
4. `ADR-001 v3` in its current repository location. **ADR-001 v3 is authoritative for all durable-idempotency design details.**
5. The accepted Phase 11 research document/report marked `COMPLETE — READY FOR PLANNING`.
6. Latest `PROJECT_STATUS.md`.
7. `DEVELOPMENT_WORKFLOW.md` for exact validation/build/test commands.

If `PLAN.md`, `ARCHITECTURE.md`, an older idempotency implementation, or an older ADR revision conflicts with ADR-001 v3, do **not** invent a compromise. Implement ADR-001 v3 and update stale architecture/status documentation as required by the architecture-change/documentation-freshness rules.

If the repository does not contain the accepted Phase 11 research artifact or ADR-001 v3, or if their exact acceptance thresholds/contracts cannot be recovered, stop before changing the durable idempotency design and report the missing authoritative input as a blocker.

---

## 1. Objective

Qualify TinixCut against realistic failure, hostile-input, sustained-load, and long-project conditions, while implementing ADR-001 v3 as the authoritative durable-idempotency architecture.

Phase 11 must convert the current feature-complete Automation-Ready MVP into a hardened release-candidate foundation by proving that:

- crashes and failed sidecars do not corrupt canonical project state;
- failed saves/exports do not replace known-good artifacts incorrectly;
- automation retries remain durably idempotent according to ADR-001 v3 across process lifetimes and canonical save/reopen cycles;
- idempotency storage, lookup, retention/compaction, migration, and fingerprinting satisfy the accepted research threat model and growth requirements;
- the WebView, automation surface, project/media inputs, sidecar invocation, and filesystem paths remain least-privilege and non-executable by untrusted data;
- background work is bounded, cancellable, deduplicated/superseded according to policy, and resistant to rapid-request overload;
- representative editing, preview, import, proxy, export, and long-project workloads meet the quantitative thresholds approved by Phase 11 research;
- performance and memory do not exhibit unbounded growth or accidental O(n²) behavior under the accepted stress fixtures;
- Phase 0–10 behavior remains regression-safe.

This is a hardening phase, not a feature-expansion phase.

---

## 2. Dependencies / Entry Conditions

Implementation may begin only if all of the following are true:

- Phases 0–10 are accepted and reconciled.
- Phase 10 Automation Surface remains the single headless editing path over the same `CommandService`, `JobManager`, and atomic persistence used by the UI.
- Phase 11 repository-first research is accepted as `COMPLETE — READY FOR PLANNING`.
- ADR-001 v3 is accepted as the authoritative durable-idempotency architecture.
- Current baseline tests/build/typecheck/lint are green before Phase 11 changes.

Before editing production code, Codex must record the exact baseline command results and the current benchmark baseline required by the accepted Phase 11 research. Existing failures discovered before Phase 11 changes must be classified as pre-existing rather than silently absorbed into this phase.

---

## 3. In Scope

### 3.1 ADR-001 v3 implementation

Implement the complete ADR-001 v3 durable-idempotency design, including all repository changes required by that ADR, such as applicable domain structures, canonical persistence representation, migration/compatibility handling, fingerprinting, replay lookup, retention/compaction behavior, transaction semantics, receipt reconstruction/storage, and automation integration.

The **exact** algorithm, schema fields, retention rules, fingerprint primitive/canonicalization rules, compaction representation, and compatibility policy must come from ADR-001 v3. This plan intentionally does not redefine those architecture-level choices.

### 3.2 Reliability hardening

Cover the Phase 11 failure scenarios:

- process termination during autosave/recovery activity;
- FFmpeg/ffprobe termination during proxy/export or other sidecar-backed jobs;
- cache deletion;
- moved/renamed source media;
- malformed/corrupt project input;
- low-disk / write-failure behavior;
- permission denial;
- read-only destination;
- unsupported codec/media;
- corrupt/truncated media;
- cancellation at difficult timing boundaries;
- failed or interrupted durable-idempotency maintenance/migration according to ADR-001 v3.

### 3.3 Security hardening

Verify and harden:

- Tauri capabilities / permissions remain least privilege;
- no raw shell capability is exposed to the WebView or automation clients;
- navigation and CSP policy are constrained to intended application behavior;
- FFmpeg/ffprobe/sidecar arguments are constructed by trusted typed native code;
- untrusted project/media metadata, filenames, captions/subtitles, fonts, paths, and automation payload fields never become shell fragments or executable commands;
- path validation/promotion rules prevent unsafe overwrite or traversal behavior;
- malformed/unusually large inputs are handled with practical resource limits derived from the accepted research;
- protocol stdout/stderr separation remains strict for machine-readable automation sessions;
- stable machine-readable errors remain non-secret-bearing and do not leak unintended filesystem/process authority.

### 3.4 Performance qualification

Measure and harden at least:

- application startup / project open;
- project serialization/load/save, including ADR-001 v3 idempotency state;
- media import/probe;
- timeline interaction latency;
- seek-to-visible-frame latency;
- continuous playback behavior;
- memory stability during sustained editing/playback/automation;
- proxy generation;
- export throughput;
- rapid-cancel behavior;
- automation apply/replay lookup cost;
- long-project command/idempotency overhead;
- command-ledger / ADR-001 v3 storage-growth behavior under sustained operation counts.

### 3.5 Queue/backpressure hardening

Stress and fix bounded behavior for rapid:

- seeks/scrubs;
- import/probe requests;
- proxy requests;
- thumbnail/waveform requests where applicable;
- automation requests within supported session semantics;
- cancellations and superseding interactive jobs.

### 3.6 Long-project behavior

Exercise representative large projects with enough tracks, clips/items, automation operations, and persisted idempotency state to expose:

- accidental O(n²) or worse hot paths;
- unbounded memory retention;
- pathological save/open costs;
- replay lookup degradation;
- UI/domain iteration costs that violate research thresholds;
- queue explosion under project-scale operations.

### 3.7 Regression and documentation reconciliation

Preserve all accepted Phase 0–10 behavior and reconcile durable project state after Phase 11 completion.

---

## 4. Explicitly Out of Scope

Do **not** expand Phase 11 into:

- new creator-facing editing features;
- the deferred `CrossDissolve` work from Phase 9;
- non-linear Bezier/custom keyframes;
- multi-channel/spectral waveform UI;
- MCP server implementation;
- cloud automation/rendering/collaboration;
- Redis, RabbitMQ, Kafka, remote worker queues, or a database-backed distributed job system;
- public plugin SDK / native plugin execution;
- new AI/provider integration;
- HDR / advanced color-management expansion;
- 4K real-time multi-layer playback guarantee beyond approved research/roadmap targets;
- H.265/AV1 launch support unless already part of an approved supported-media matrix;
- Phase 12 installer/package/release-distribution work;
- changing preview/export semantics unless required to fix a Phase 11-discovered correctness regression;
- broad refactors unrelated to a measured Phase 11 failure, security issue, performance bottleneck, or ADR-001 v3 requirement.

A nonessential cleanup discovered during Phase 11 must be recorded as deferred technical debt unless it is required to satisfy an acceptance criterion or remove a regression introduced by this phase.

---

## 5. Existing-State Assumptions to Verify, Not Rebuild

Codex should confirm actual repository state before modifying it. Expected existing capabilities include:

- versioned canonical Project Domain Model;
- single authoritative `CommandService` for UI and automation mutations;
- atomic transactions, revision checks, structured errors, dry-run/diff;
- save/reopen, autosave/recovery, migration harness, offline/relink behavior;
- local bounded `JobManager`;
- FFmpeg/ffprobe behind trusted native interfaces;
- normalized composition semantics shared by preview/export;
- safe export temp-output promotion and snapshot isolation;
- CLI-first automation with long-lived JSONL stdio session;
- machine protocol frames on stdout and diagnostics on stderr;
- Phase 10 durable idempotency behavior based on the pre-ADR-001-v3 implementation, including existing `command_ledger` persistence and FNV-1a fingerprinting where still present.

Do not preserve an old implementation detail merely because tests exist for it if ADR-001 v3 supersedes that detail. Update tests to assert the authoritative behavior, not obsolete internals.

---

## 6. Architecture Constraints / Non-Negotiable Invariants

Phase 11 must preserve these invariants:

1. Canonical project state is application-owned and versioned.
2. UI and automation mutate through one authoritative `CommandService`.
3. Failed atomic mutations leave canonical project state unchanged.
4. Stale automation revisions are rejected explicitly.
5. Durable idempotency remains project-owned correctness state and survives the process/save/reopen boundaries required by ADR-001 v3.
6. Idempotency maintenance/retention/compaction must not create a second mutation path or bypass canonical persistence.
7. Dry-run must not mutate project state or durable idempotency state.
8. Cache deletion must not remove canonical replay protection.
9. High-frequency UI gestures must not create synchronous heavy disk/native work per event.
10. Cache/derived media remain rebuildable and non-canonical.
11. Missing media preserves editorial state.
12. Preview/export share normalized semantic intent.
13. No raw shell or arbitrary filesystem authority is exposed to the WebView/automation surface.
14. Sidecar process arguments originate in trusted native code and are typed/validated.
15. Background work remains local, bounded, cancellable, and policy-driven.
16. No distributed infrastructure is introduced.

Any implementation pressure that appears to require breaking one of these constraints is an architecture discrepancy, not an invitation to improvise.

---

## 7. Workstream 11.0 — Baseline, Research Binding, and Test Harness Preparation

### 11.0.1 Verify authoritative inputs

- Locate ADR-001 v3 and the accepted Phase 11 research artifact in the repository.
- Record their exact paths in the Codex completion report.
- Extract the research-approved quantitative thresholds into the implementation/test notes without changing them.
- Identify any stale Phase 10/FNV/ledger statements in `PLAN.md`, `ARCHITECTURE.md`, `PROJECT_STATUS.md`, or tests that ADR-001 v3 supersedes.

### 11.0.2 Capture clean baseline

Run the repository's standard validation path before changes:

- frontend tests;
- Rust tests;
- typecheck;
- Rust lint/clippy with warnings denied if that is the repository standard;
- build;
- relevant automation E2E;
- relevant preview/export parity checks.

Capture baseline benchmark measurements for each research-defined metric on the designated benchmark hardware/profile. Do not compare post-change results to guessed or historic values when a same-environment baseline can be obtained.

### 11.0.3 Add deterministic hardening fixtures/harnesses

Prefer test-only fault-injection and deterministic fixtures over ad-hoc manual corruption.

Where feasible, create reusable test utilities for:

- injected write/rename/fsync/promotion failures;
- sidecar process termination;
- read-only/permission-denied paths;
- constrained/free-space simulation or injectable storage-failure abstraction;
- malformed project fixtures;
- corrupt/truncated media fixtures;
- rapid cancellation/supersession;
- large synthetic project generation;
- large idempotency-history generation according to ADR-001 v3.

Test harness changes are in scope; production behavior changes should follow only where a failing hardening test identifies a real gap.

---

## 8. Workstream 11.1 — Implement ADR-001 v3 Durable Idempotency

This workstream is release-blocking. The accepted ADR is the design contract; the checklist below is the implementation/verification envelope.

### 11.1.1 Domain and persistence representation

- Implement the ADR-001 v3 canonical durable-idempotency representation in the Project domain/persistence layer.
- Remove, migrate, or adapt the pre-v3 representation exactly as ADR-001 v3 requires.
- Preserve deterministic serialization and schema validation.
- Ensure a project cannot deserialize into an ambiguous mixed old/new idempotency state.
- If ADR-001 v3 introduces schema-version changes, use the existing migration harness; migrations must be deterministic, tested, and non-destructive.
- Migration failure must never overwrite the original project.

### 11.1.2 Canonical operation fingerprinting

- Replace/retain the fingerprint implementation exactly according to ADR-001 v3.
- Canonicalize semantic operation payloads exactly once in trusted core code; transports must not define their own fingerprints.
- Ensure semantically identical retries produce the same fingerprint under the ADR contract.
- Ensure semantically different operations reusing an ID are rejected according to the ADR collision semantics.
- Include adversarial/untrusted automation payload cases required by research.
- Do not silently accept an old fingerprint algorithm for newly committed entries if ADR-001 v3 supersedes it.

### 11.1.3 Replay behavior

For a previously committed operation/transaction identity within ADR-001 v3 guarantees:

- exact replay must not mutate canonical project state;
- project revision must not increment;
- undo/redo history must not gain a new logical entry;
- the response must be the prior receipt or the ADR-defined deterministic reconstruction;
- affected-entity semantics/status must remain compatible with the automation contract;
- replay must behave identically after save/reopen and across process boundaries.

### 11.1.4 Collision / identity reuse behavior

- Same identity + different semantic payload must fail with the ADR-defined stable machine-readable collision error.
- Collision failure must not mutate the project, history, idempotency state, revision, recovery snapshot, or output files.
- Base-revision semantics for unseen operations remain deterministic and explicit.
- Do not let collision handling degrade into “last write wins.”

### 11.1.5 Atomic transaction integration

- A failed atomic transaction must persist neither project mutations nor partial idempotency state.
- A successful multi-operation transaction must commit project changes and ADR-defined durable-idempotency state as one logical authoritative outcome.
- Retry behavior for partially/fully replayed transaction contents must follow ADR-001 v3 exactly; do not infer new transaction semantics.

### 11.1.6 Dry-run isolation

- `command.dryRun` must perform the same relevant validation/fingerprint/precondition logic needed for trustworthy results while writing nothing.
- Dry-run must not append idempotency records, advance retention state, trigger compaction, modify revision, or write canonical/recovery files except where the ADR explicitly requires a read-only metadata observation.

### 11.1.7 Retention / compaction / growth control

Implement the ADR-001 v3 retention/compaction model exactly.

Required verification regardless of implementation form:

- replay guarantees remain valid for the ADR-defined guarantee scope;
- maintenance is crash-safe and atomic with respect to canonical correctness;
- interrupted maintenance cannot produce a project that both loses committed edits and claims success;
- compaction/retention cannot cause a previously protected identity inside the guarantee scope to be applied again;
- old/new state can be reopened deterministically;
- maintenance is bounded and does not cause uncontrolled pause/memory amplification;
- cache deletion does not affect replay protection;
- save/reopen after maintenance preserves canonical semantic state.

### 11.1.8 Compatibility / migration tests

At minimum add fixtures for:

- project produced by the Phase 10/pre-v3 durable ledger implementation;
- project already in ADR-001 v3 format;
- empty/new project;
- project with many committed operation identities;
- project containing exact replay cases;
- project containing intentional identity/payload mismatch;
- interrupted/failed migration or maintenance path where fault injection is feasible.

### 11.1.9 ADR-001 v3 acceptance tests

The automated suite must prove:

1. apply -> save -> process exit -> reopen -> retry exact identity => no duplicate mutation;
2. revision unchanged on deduplicated replay;
3. no additional history entry on deduplicated replay;
4. same identity + altered semantic payload => explicit collision error;
5. failed atomic transaction => no project mutation and no partial durable-idempotency mutation;
6. dry-run => no durable-idempotency mutation;
7. cache deletion => replay guarantee unchanged;
8. old-format project migration/reopen preserves existing project semantics and ADR-defined replay guarantees;
9. retention/compaction maintenance preserves the ADR-defined replay guarantee;
10. large-history lookup/save/open behavior satisfies research thresholds.

---

## 9. Workstream 11.2 — Crash, Persistence, Recovery, and Sidecar Failure Hardening

### 11.2.1 Atomic save / autosave kill scenarios

Create deterministic tests or a subprocess/fault-injection harness for termination/failure at meaningful boundaries:

- before temp write completes;
- after temp write but before promotion;
- during flush/close where testable;
- immediately before replace/rename;
- immediately after promotion;
- during autosave/recovery update;
- during ADR-001 v3 migration/maintenance when applicable.

Acceptance:

- last known-good canonical project remains readable when promotion did not complete;
- no malformed file is promoted as canonical success;
- recovery/autosave remains separately identifiable from canonical save;
- reopening never chooses a corrupt partial temp artifact as authoritative;
- the UI/automation receives a structured, actionable error rather than silent success.

### 11.2.2 Sidecar termination

Cover proxy/export and applicable probe/thumbnail/waveform jobs:

- kill FFmpeg/ffprobe after start;
- kill near completion;
- terminate during output creation;
- cancel concurrently with process completion/failure.

Acceptance:

- project state remains valid;
- job reaches one terminal state only;
- temporary outputs are cleaned or safely quarantined according to existing policy;
- final export path is not promoted on failure;
- a previous valid output is not destroyed by a failed replacement;
- no orphan child process remains after cancellation/shutdown tests.

### 11.2.3 Low disk / permission / read-only behavior

Exercise:

- project save destination;
- recovery/autosave destination;
- cache/derived output destination;
- proxy output;
- export final/temp output.

Acceptance:

- errors are classifiable and surfaced through stable error categories;
- no success is reported when durability/promotion failed;
- canonical project and prior valid outputs remain intact;
- retries after restoring permission/space behave correctly;
- no busy-loop retry floods the job manager.

### 11.2.4 Corrupt / malformed input

Test malformed project documents and corrupt/truncated media.

Acceptance:

- load/probe/render paths fail explicitly;
- parser/validator failures do not panic the application process;
- no project rewrite occurs merely because malformed input was opened;
- untrusted error strings are displayed/logged as data, not executed or interpolated into commands.

### 11.2.5 Cache deletion / moved source regression

Re-run the established Phase 2/4 guarantees under the hardened code:

- deleting all derived cache preserves project correctness and ADR-001 v3 replay protection;
- moved/renamed media becomes offline without deleting timeline items;
- relink restores association without changing timeline semantics.

---

## 10. Workstream 11.3 — Security Review and Hardening

### 11.3.1 Tauri capability / permission audit

Inspect Tauri configuration, capabilities, allowlists, plugins, window permissions, and native commands.

Acceptance:

- editor WebView does not receive unrestricted shell/process authority;
- arbitrary filesystem write/enumeration is not exposed beyond approved product flows;
- capabilities are scoped to the minimum windows/commands that require them;
- no Phase 11 change broadens permissions merely to simplify testing.

### 11.3.2 Navigation and CSP

Verify current Tauri/WebView configuration:

- intended application origins/resources only;
- arbitrary external navigation is blocked or routed through an explicit reviewed path;
- CSP is present and no broader than required by the current frontend/runtime.

Add automated/static config checks where practical so future changes cannot silently reopen the boundary.

### 11.3.3 Sidecar command construction

Review every FFmpeg/ffprobe/process-spawn boundary.

Acceptance:

- executable identity is application-controlled;
- arguments are passed as argument arrays/typed values, not a shell-concatenated command string;
- filenames/metadata/captions/project strings cannot inject additional options/commands beyond explicitly allowed typed fields;
- paths are validated/canonicalized according to existing path policy before destructive output promotion;
- no automation payload can choose an arbitrary executable.

### 11.3.4 Untrusted project/media/automation inputs

Create adversarial fixtures for practical cases identified in Phase 11 research, including where relevant:

- special characters and shell metacharacters in filenames/metadata;
- traversal-like relative path strings;
- oversized metadata/caption/font/input fields within the research threat model;
- malformed JSONL protocol messages;
- duplicate/unknown request IDs;
- invalid enum/operation fields;
- maliciously reused idempotency IDs/payloads;
- invalid UTF-8/binary boundaries where APIs can receive them.

Acceptance:

- input is rejected, normalized, or safely treated as data;
- no process execution or filesystem authority is gained;
- application remains responsive within research-defined resource limits;
- structured errors do not expose secrets or protocol-corrupting raw output.

### 11.3.5 Automation protocol regression

Re-run protocol/subprocess tests to prove:

- stdout remains JSONL protocol only;
- stderr remains diagnostics only;
- one malformed request does not desynchronize subsequent valid frames unless session policy explicitly terminates the session;
- session shutdown/cancel cleans up jobs/processes;
- automation still uses `CommandService` / `JobManager`; no security workaround creates a second reducer.

---

## 11. Workstream 11.4 — Performance Qualification

### 11.4.1 Benchmark discipline

Use the hardware/profile matrix and absolute thresholds approved by Phase 11 research. Do not invent looser substitute targets in implementation.

For each benchmark, record:

- hardware/OS/build mode;
- fixture/project size;
- warm/cold condition if relevant;
- sample count;
- metric definition;
- P50/P95/P99 where research requires them;
- peak/steady memory where relevant;
- baseline and post-change result;
- pass/fail against the approved threshold.

Debug builds may be used for correctness diagnostics but release-performance qualification must use the build mode specified by research.

### 11.4.2 Startup / project open

Benchmark at least:

- small normal project;
- representative project;
- long-project fixture with substantial ADR-001 v3 idempotency history/state.

Separate parsing/deserialization, migration/validation, idempotency-state reconstruction/indexing, and media/cache initialization when instrumentation makes that useful.

### 11.4.3 Save / serialization / durable-idempotency overhead

Measure:

- serialized project size as idempotency history grows;
- save latency;
- load/open latency;
- peak memory during serialization/load;
- maintenance/compaction cost according to ADR-001 v3;
- replay lookup latency for early/middle/recent identities where relevant.

Acceptance is the research-approved growth/latency/memory envelope, not merely “faster than before.”

### 11.4.4 Media import/probe and proxy generation

Measure supported representative fixtures, including normal and failure cases.

Confirm UI/event-loop responsiveness is not blocked by synchronous heavy work.

### 11.4.5 Timeline interaction latency

Measure representative drag/trim/transform/zoom/scroll behavior with large project fixtures.

Verify transient pointer updates do not create canonical command/history/persistence writes per event.

### 11.4.6 Seek / playback

Re-run representative preview workloads and research thresholds:

- repeated seeks;
- rapid scrubbing;
- sustained 30–60 second playback or longer if research requires;
- stacked layers / audio workload matching current supported scope.

Measure at least seek-to-visible-frame latency, dropped frames where observable, memory behavior, stale work, and UI responsiveness.

### 11.4.7 Export

Measure deterministic supported output profile(s):

- wall-clock export time / realtime factor as defined by research;
- peak memory;
- cancellation latency/cleanup;
- output promotion behavior.

Do not turn Phase 11 into codec/hardware-distribution qualification; that remains Phase 12 unless research explicitly made a Phase 11 hardening requirement.

### 11.4.8 Memory stability / leak detection

Run sustained scenarios defined by research, including repeated:

- open/close or project load where supported;
- seek/scrub cycles;
- proxy/job cancellation cycles;
- export start/cancel cycles;
- automation apply/replay cycles;
- save/reopen cycles.

Acceptance:

- memory reaches a bounded steady behavior within the research-defined limit;
- no monotonic leak from completed/cancelled jobs, child processes, preview buffers, automation session state, or ADR-001 v3 replay structures.

---

## 12. Workstream 11.5 — Queue / Backpressure / Cancellation Hardening

### 11.5.1 Validate per-job-class policy

For each active job class, verify explicit:

- max concurrency;
- queue bound or equivalent boundedness policy;
- priority;
- dedupe key;
- cancellation behavior;
- supersession/stale-result policy;
- retry policy;
- temp-output cleanup.

Do not introduce a generic unbounded queue to simplify scheduling.

### 11.5.2 Rapid seek/scrub stress

Generate request rates above normal interaction frequency.

Acceptance:

- obsolete pending work is cancelled/dropped/superseded according to policy;
- stale results cannot overwrite the latest request;
- pending-work count remains bounded;
- memory does not grow proportionally with every obsolete request;
- UI remains responsive within research thresholds.

### 11.5.3 Import/proxy/thumbnail/waveform stress

Submit duplicates and bursts.

Acceptance:

- dedupe prevents redundant equivalent work where policy says it should;
- concurrency never exceeds configured class limits;
- cancellation frees resources;
- one failing job does not wedge the queue;
- retries do not form an unbounded retry storm.

### 11.5.4 Export isolation

Confirm exports remain non-supersedable explicit jobs:

- later requests do not silently replace an active export;
- only explicit cancellation stops it;
- cancellation and completion races yield exactly one terminal state.

---

## 13. Workstream 11.6 — Long-Project / Complexity Qualification

### 11.6.1 Synthetic project profiles

Use deterministic project generators instead of committing huge hand-authored fixtures when practical.

Include profiles from Phase 11 research covering enough:

- tracks;
- clips/items;
- assets;
- text/keyframes/transitions within supported scope;
- command history / automation operations;
- ADR-001 v3 durable-idempotency state;
- offline assets where useful.

### 11.6.2 Complexity probes

Instrument or benchmark likely hot paths:

- project validation;
- item lookup by stable ID;
- command target lookup;
- dry-run/apply;
- history operations where impacted;
- composition-plan generation;
- project serialization/deserialization;
- idempotency replay lookup;
- ADR-001 v3 maintenance/compaction;
- UI projection/selector logic for large timelines.

### 11.6.3 Fix only measured blockers

Optimize only paths that violate accepted thresholds or clearly exhibit pathological scaling. Prefer narrow data-structure/indexing changes with invariant tests over broad rewrites.

Any new index/cache must have explicit invalidation/ownership semantics and must not become a second canonical source of truth.

---

## 14. Workstream 11.7 — Error Model, Diagnostics, and Failure Observability

Phase 11 may add diagnostics needed to prove hardening, but must not create a telemetry/cloud requirement.

Ensure failures remain distinguishable through stable machine-readable categories, including where applicable:

- validation error;
- stale revision;
- idempotency collision;
- unsupported operation/media/codec;
- missing/offline asset;
- permission/path error;
- low disk/storage error;
- job cancelled;
- sidecar/process failure;
- render failure;
- corrupted project;
- schema migration failure;
- internal invariant violation.

Requirements:

- one failure should map to one clear terminal outcome;
- user-facing details should be actionable but not expose unnecessary sensitive paths/data;
- automation errors must remain valid machine-readable protocol frames;
- diagnostic logs go to stderr in automation stdio mode;
- no test should rely only on log text when a stable structured error exists.

---

## 15. Expected Files / Modules Affected

Codex must inspect the repository and use actual paths; do not create parallel modules merely to match this list.

Likely affected areas include:

- Project domain/schema and migration modules;
- Command Service / idempotency / transaction code;
- persistence / atomic save / recovery modules;
- automation protocol/session tests and adapters;
- Job Manager and job-type policies;
- media/FFmpeg process wrapper;
- export output-promotion/cancellation code;
- preview seek/cancellation/backpressure code if measurements find a gap;
- Tauri capability/config/CSP/navigation policy;
- test fixtures/fault-injection utilities;
- benchmark/stress harnesses;
- `ARCHITECTURE.md` only where ADR-001 v3 or implementation reality requires reconciliation;
- `PLAN.md` Phase 11 status/reconciliation after acceptance;
- `PROJECT_STATUS.md` current snapshot;
- ADR/research references if their implementation status needs recording.

Do not reorganize modules solely for aesthetic reasons.

---

## 16. Test Plan

### 16.1 Mandatory targeted automated tests

Add/retain tests for:

**ADR-001 v3**

- exact replay in-memory;
- exact replay after save/reopen;
- exact replay across separate process execution where harness exists;
- mismatched payload collision;
- dry-run isolation;
- transaction rollback isolation;
- migration from pre-v3 state;
- retention/compaction correctness;
- crash/interruption around maintenance where feasible;
- large-state replay lookup and persistence behavior.

**Persistence/recovery**

- atomic save interruption points;
- autosave/recovery interruption;
- malformed project;
- migration failure preserving original;
- permission/low-disk/read-only failures.

**Media/jobs/export**

- sidecar kill;
- corrupt/unsupported media;
- proxy/export cancellation races;
- temp cleanup;
- prior final output preservation;
- queue bound/dedupe/supersession.

**Security**

- shell metacharacter filenames/metadata;
- traversal-like paths;
- malformed automation frames;
- argument-array/typed invocation invariants where testable;
- Tauri capability/config assertions where practical.

**Long project / performance correctness**

- deterministic large synthetic project generation;
- operations retain semantic correctness at scale;
- no stale-result overwrite under stress;
- no duplicate mutation under high replay volume.

### 16.2 Full regression suites

Before completion run all repository-standard checks, including applicable:

- `cargo test` / Rust workspace tests;
- frontend tests (`vitest` or current equivalent);
- TypeScript typecheck;
- clippy/lint with repository warning policy;
- frontend/native build;
- automation E2E 10-step scenario;
- preview/export parity suite;
- persistence/recovery/migration suites;
- media fixture tests;
- Phase 11 hardening/stress suites.

Use the exact repository commands from `DEVELOPMENT_WORKFLOW.md`; do not invent command names in the completion report.

### 16.3 Test quality rules

- Do not weaken old assertions to make new architecture pass.
- If a test encodes a superseded pre-ADR internal detail, replace it with an assertion of ADR-001 v3 externally required behavior and explain why.
- Prefer deterministic fault injection over timing sleeps.
- Stress tests must have bounded execution time and deterministic pass/fail criteria where possible.
- Performance benchmarks must be separated from flaky unit-test gates when hardware variance makes absolute timing unsuitable for ordinary CI; release-qualification thresholds still need recorded benchmark evidence on the approved hardware profile.

---

## 17. Benchmark / Performance Gate

The Phase 11 completion report must contain a table with every research-approved metric and its result.

Minimum categories:

| Category | Required evidence |
|---|---|
| Startup / open | Baseline vs Phase 11 result on research fixture(s) |
| Save / serialize | Latency, file-size growth, peak memory, ADR-001 v3 overhead |
| Idempotency replay | Lookup/replay latency at research-defined history sizes |
| Retention/compaction | Runtime, memory, post-maintenance size, correctness |
| Import/probe | Representative supported fixture timings |
| Timeline interaction | Research-defined latency measure under representative/large project |
| Seek | P50/P95 or exact research-defined metric |
| Continuous playback | Dropped-frame/responsiveness/memory measures required by research |
| Proxy generation | Throughput and cancellation behavior |
| Export | Throughput/realtime factor, memory, cancel cleanup |
| Queue stress | Max observed pending/running work vs configured bounds |
| Long project | Scaling behavior at each research-defined size point |
| Memory stability | Start/steady/end memory or research-defined leak criterion |

**Pass rule:** every release-blocking metric must satisfy the exact accepted Phase 11 research threshold. If a threshold is not present in the repository research artifact, Codex must not silently invent one; report the missing criterion as a documentation blocker for formal Phase 11 acceptance.

---

## 18. Manual QA / Runtime Checks

Automated testing is primary, but Phase 11 completion also requires focused manual QA on the target Windows development machine for behaviors that are hard to model reliably in unit tests.

At minimum, where applicable:

1. open a representative real-media project;
2. perform rapid seek/scrub and normal timeline edits while background work runs;
3. start/cancel proxy/export jobs at different progress points;
4. remove cache and confirm regeneration without editorial loss;
5. temporarily move a source and confirm offline/relink behavior;
6. attempt save/export to a denied or read-only location;
7. run the automation stdio session and verify clean JSONL stdout under error/log activity;
8. save/reopen then retry a committed automation operation identity and confirm deduplicated behavior;
9. exercise the large-project fixture/profile enough to observe UI responsiveness and job boundedness;
10. verify no orphan FFmpeg/ffprobe processes remain after cancellation/app shutdown scenarios tested by the phase.

Manual QA findings must be recorded as PASS/FAIL with enough detail to reproduce failures.

---

## 19. Regression Checks Against Prior Phases

Phase 11 is not complete unless the following remain true:

### Phase 1 / Command safety

- UI-like and automation-like callers still reach the same command semantics;
- stale revision rejected;
- failed atomic transaction changes nothing;
- dry-run writes nothing;
- canonical time/domain invariants still pass.

### Phase 2 / Persistence and recovery

- save/reopen semantic equivalence;
- interrupted save preserves last valid project;
- cache deletion preserves project correctness;
- offline/relink semantics preserved;
- undo/redo round trips preserved;
- migrations remain deterministic/non-destructive.

### Phase 4 / Jobs and media

- supported import remains asynchronous/non-blocking;
- jobs remain bounded/cancellable/deduped according to policy;
- corrupt/unsupported media produces explicit errors;
- raw input cannot inject sidecar commands.

### Phase 6–8 / Preview/export

- selected preview backend still meets established correctness behavior;
- preview/export parity suite remains within approved tolerance;
- export snapshot isolation preserved;
- failed/cancelled export never promotes misleading final output.

### Phase 9 / Creator features

- supported text/audio/fade/keyframe/waveform behaviors retain save/reopen, preview, export, parity, and undo/redo coverage appropriate to their existing support status;
- previously deferred feature scope remains deferred.

### Phase 10 / Automation

- inspect/diff/apply/render surface remains compatible;
- stdout/stderr protocol separation preserved;
- exact replay remains deduplicated according to ADR-001 v3;
- stale revision and ID reuse collision remain explicit;
- export automation remains routed through `JobManager`.

---

## 20. Acceptance Criteria

Phase 11 may be classified `COMPLETE` only when all material criteria below are satisfied, or `COMPLETE WITH DEFERRED ITEMS` only if the remaining items are explicitly non-blocking and do not weaken release safety.

### P11-AC-01 — ADR authority implemented

Repository production behavior conforms to ADR-001 v3. No old Phase 10 idempotency implementation remains authoritative where the ADR supersedes it.

### P11-AC-02 — Durable replay guarantee

Exact retry of a previously committed identity behaves according to ADR-001 v3 across canonical save/reopen and process lifetimes, without duplicate mutation, revision bump, or duplicate logical history entry.

### P11-AC-03 — Collision safety

Reusing an identity with a semantically different payload produces the ADR-defined explicit stable error and leaves canonical state unchanged.

### P11-AC-04 — Transaction / dry-run isolation

Failed atomic transactions and dry-runs do not partially mutate project state or durable idempotency state.

### P11-AC-05 — ADR migration/compatibility

Pre-v3 persisted projects covered by the compatibility policy migrate/open deterministically without silent data loss, and migration failure preserves the original file.

### P11-AC-06 — Retention/growth hardening

ADR-001 v3 retention/compaction/growth-control behavior is implemented and proven to preserve its stated replay guarantees while meeting research-defined project-size/save/open/memory thresholds.

### P11-AC-07 — Save/recovery crash safety

Injected interruption/failure around canonical save/autosave/recovery cannot promote a corrupt partial project over the last known-good canonical state.

### P11-AC-08 — Sidecar/export failure safety

Killing/cancelling FFmpeg/ffprobe at tested difficult boundaries cannot corrupt the project, cannot incorrectly promote a failed export, and leaves no unbounded/orphaned process work.

### P11-AC-09 — Filesystem failure safety

Low-disk, permission-denied, and read-only cases return explicit errors while preserving the prior valid canonical project/output where applicable.

### P11-AC-10 — Untrusted input safety

Malformed project/media/automation input and adversarial filenames/metadata cannot escape typed validation into unrestricted shell/process/filesystem authority.

### P11-AC-11 — Least privilege

Tauri/WebView/automation capabilities remain least-privilege with no unrestricted shell, arbitrary process launch, or arbitrary filesystem mutation path.

### P11-AC-12 — Bounded queues

Stress tests show rapid seeks/imports/proxy/derived-media requests remain within explicit queue/concurrency bounds and obsolete interactive work cannot accumulate unboundedly or overwrite newer state.

### P11-AC-13 — Cancellation races

Jobs subjected to cancellation/completion/failure races produce exactly one terminal outcome and clean up temporary resources according to policy.

### P11-AC-14 — Performance qualification

All research-designated release-blocking startup/open/import/timeline/seek/playback/memory/proxy/export/cancel metrics pass on the approved target hardware/profile.

### P11-AC-15 — Long-project qualification

Research-defined long-project fixtures do not expose an unresolved accidental O(n²) hot path, unbounded memory growth, or threshold-breaking idempotency/save/open degradation in release-blocking workflows.

### P11-AC-16 — Regression suite

All applicable Phase 0–10 automated regression suites pass, including automation E2E and preview/export parity.

### P11-AC-17 — Repository health

Typecheck, lint/clippy, tests, and builds pass with no new unclassified warnings/errors.

### P11-AC-18 — Documentation reconciliation

`PLAN.md`, `ARCHITECTURE.md`/ADR references as needed, and `PROJECT_STATUS.md` accurately describe the implemented ADR-001 v3 state, Phase 11 outcome, measured performance gate, deferred items, and remaining Phase 12 dependencies.

---

## 21. Completion Gate / Phase Decision Rules

### COMPLETE

Use only when:

- all release-blocking acceptance criteria pass;
- ADR-001 v3 is fully implemented;
- research-defined performance/security/reliability gates pass;
- no known corruption, duplicate-automation-mutation, unrestricted-authority, or unbounded-background-work path remains;
- documentation/state reconciliation is complete.

### COMPLETE WITH DEFERRED ITEMS

Allowed only when:

- all correctness/security/data-loss/idempotency/backpressure gates pass;
- deferred items are non-blocking optimizations or explicitly future product scope;
- each deferred item is recorded with impact and future owner/phase.

Examples of acceptable deferral may include a non-release-blocking benchmark tooling improvement or optional diagnostic polish, but **not** a failed data-safety, idempotency, security-boundary, queue-boundedness, or release-blocking performance criterion.

### INCOMPLETE

Use when required tests/benchmarks/acceptance criteria are missing, failed, or insufficiently evidenced.

### BLOCKED

Use when ADR/research inputs are unavailable/contradictory, required target hardware/resource is unavailable for a mandatory gate, or implementation reveals an architecture conflict that cannot be resolved within the approved phase scope.

---

## 22. Deferred / Future Work

The following remain outside Phase 11 unless implementation evidence makes one strictly necessary for a Phase 11 acceptance criterion:

- Phase 12 FFmpeg distribution/build/license inventory;
- installer/upgrade/uninstall qualification;
- final packaging and release suite orchestration;
- broad hardware-acceleration support matrix and packaging fallback qualification assigned to Phase 12;
- MCP transport;
- cloud/remote execution;
- cross-platform release parity;
- public plugin system;
- advanced creator features previously deferred in Phase 9;
- new codecs/features outside supported v1 scope.

Any new deferred item discovered during hardening must state:

- what was found;
- why it is non-blocking;
- risk/impact;
- recommended future phase/workstream;
- whether it changes Phase 12 entry conditions.

---

## 23. Required Documentation / Project-State Updates After Successful Implementation

Do not perform these updates merely because coding stopped; perform them only after Codex self-review shows the phase criteria are satisfied, and ChatGPT review can later accept/reconcile the phase.

Codex should prepare/update durable state so review can verify:

### `PROJECT_STATUS.md`

Record:

- Phase 11 implementation/review status;
- ADR-001 v3 implemented state;
- hardening test summary;
- benchmark results/gate status;
- current known issues/deferred items;
- Phase 12 dependency readiness.

### `PLAN.md`

After Phase 11 is accepted, reconcile:

- Phase 11 status;
- completed hardening scope;
- ADR-001 v3 implementation summary;
- accepted deferred items;
- any changed Phase 12 dependency.

### `ARCHITECTURE.md` / ADR

Update only where required to align stale architecture prose with ADR-001 v3 or a genuinely new cross-phase invariant discovered during implementation.

Do not duplicate detailed benchmark logs into architecture documentation.

---

## 24. Codex Execution Order

Implement in the following order unless repository evidence requires a small documented adjustment:

1. **Read authoritative docs and capture clean baseline.**
2. **Bind research thresholds into test/benchmark plan.**
3. **Implement ADR-001 v3 domain/persistence/migration/fingerprint/replay/retention behavior.**
4. **Land ADR-focused unit/integration/migration/replay tests.**
5. **Add deterministic failure-injection harnesses.**
6. **Harden save/recovery/filesystem/sidecar failure paths.**
7. **Perform security audit and fix only verified gaps.**
8. **Stress Job Manager/backpressure/cancellation and fix boundedness gaps.**
9. **Run long-project profiling; optimize measured release-blocking hot paths.**
10. **Run full performance qualification on approved profiles.**
11. **Run full Phase 0–10 regression suites, parity, automation E2E, typecheck, lint, build.**
12. **Perform focused manual QA.**
13. **Self-review every P11 acceptance criterion.**
14. **Update durable project status/docs required for review.**
15. **Produce the Codex completion report.**

Do not mark Phase 11 complete merely because ADR-001 v3 compiles or because the unit suite is green. The failure/security/performance gates are part of the phase definition.

---

## 25. Codex Completion Report Requirements

The completion report must include all of the following.

### A. Summary

- Phase result proposed by Codex: `COMPLETE`, `COMPLETE WITH DEFERRED ITEMS`, `INCOMPLETE`, or `BLOCKED`.
- One-paragraph summary of implemented hardening.

### B. ADR-001 v3 implementation evidence

- ADR path/version read;
- research artifact path/status read;
- old -> new durable-idempotency representation summary;
- migration/compatibility behavior;
- fingerprint/canonicalization behavior at a high level without redefining the ADR;
- retention/compaction/growth-control behavior;
- replay/collision/transaction/dry-run semantics;
- exact tests proving them.

### C. Reliability hardening

For each Phase 11 failure scenario:

- test/harness used;
- observed pre-fix issue if any;
- implementation change;
- post-fix result.

### D. Security review

- Tauri capability findings;
- CSP/navigation findings;
- sidecar invocation review;
- untrusted-input tests;
- any security issue fixed or deferred with rationale.

### E. Performance / benchmark table

For every research-approved metric:

- hardware/profile;
- fixture;
- baseline;
- final result;
- threshold;
- PASS/FAIL.

### F. Queue/backpressure / long-project results

- configured bounds;
- stress request volume/profile;
- max observed running/pending counts;
- stale-result/cancellation evidence;
- long-project scaling results;
- any measured complexity improvement.

### G. Validation commands

Report exact repository commands and results for:

- targeted tests;
- full tests;
- typecheck;
- lint/clippy;
- build;
- automation E2E;
- preview/export parity;
- manual QA;
- benchmarks/stress harnesses.

### H. Files/modules changed

Group by:

- domain/idempotency;
- persistence/migrations;
- jobs/media/export/preview;
- security/config;
- tests/fixtures/benchmarks;
- documentation.

### I. Acceptance criteria matrix

List `P11-AC-01` through `P11-AC-18` with `PASS`, `FAIL`, `BLOCKED`, or `N/A` plus evidence.

### J. Deviations, debt, deferred work

Explicitly report:

- deviations from this plan;
- unresolved known issues;
- accepted deferred items;
- technical debt;
- architecture implications/ADR follow-up;
- any impact on Phase 12 entry conditions.

---

## 26. Stop / Escalation Conditions During Implementation

Codex must stop expanding scope and report rather than improvise if any of these occur:

- ADR-001 v3 cannot be found or is internally inconsistent with the accepted research;
- implementing the ADR appears to require a second canonical mutation path;
- migration would risk irreversible project/idempotency data loss without an approved policy;
- durable-idempotency semantics cannot satisfy the ADR under retention/compaction;
- a security fix requires materially broadening WebView/automation filesystem or shell authority;
- a release-blocking performance target cannot be met without replacing a major architecture boundary;
- target-hardware evidence required by research is unavailable;
- fixing a discovered issue would require a new product feature or major Phase 12 scope.

In such a report, include evidence, affected acceptance criteria, smallest viable architectural correction, and what work can still be safely completed without crossing the boundary.

---

## 27. Definition of Done for Phase 11

Phase 11 is done only when the repository demonstrates, with durable evidence rather than claims, that:

- ADR-001 v3 is the implemented durable-idempotency architecture;
- canonical project save/recovery and idempotency maintenance are crash/failure safe;
- automation retry cannot duplicate mutations within the ADR guarantee;
- hostile/untrusted input cannot escape into unrestricted shell/process/filesystem authority;
- local jobs remain bounded and cancellable under stress;
- long-project and idempotency-state growth remain within approved research limits;
- required performance metrics pass on the approved profile;
- all applicable prior-phase regression gates remain green;
- no unclassified release-blocking correctness/security/performance issue remains;
- durable project documentation is reconciled for Phase 12 planning.

Only after ChatGPT reviews the Codex report and reconciles `PLAN.md` / `PROJECT_STATUS.md` / architecture state should Phase 12 planning proceed.
