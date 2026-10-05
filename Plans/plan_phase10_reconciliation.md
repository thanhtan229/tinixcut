# plan_phase10_reconciliation.md — Phase 10 Project-State Reconciliation

**Project:** TinixCut  
**Task type:** Post-Phase reconciliation  
**Phase 10 status:** COMPLETE  
**Implementation scope:** Documentation/state reconciliation only  
**Production code changes:** NOT ALLOWED unless a documentation-discovered correctness contradiction is proven  
**Next gate after completion:** Phase 11 repo-first hardening research  
**Date:** 2026-10-04

## 1. Objective

Reconcile the durable project state after accepting Phase 10 as `COMPLETE`.

This task must:
1. verify `PLAN.md` and `PROJECT_STATUS.md` both reflect Phase 10 as complete;
2. update `ARCHITECTURE.md` with the Phase 10 automation transport/session decision;
3. document durable idempotency / persisted `command_ledger`;
4. record ledger growth/pruning as Phase 11 hardening debt/risk;
5. record fingerprint-strength review as a Phase 11 security-hardening item;
6. leave unrelated phase scope unchanged;
7. establish Phase 11 as next, with research required before `plan_phase11.md`.

## 2. Source of Truth

Use this precedence:
1. repository code;
2. `PLAN.md`;
3. `ARCHITECTURE.md`;
4. `PROJECT_STATUS.md`;
5. Phase 10 completion report;
6. chat/review notes.

If code/report/docs disagree, determine actual implementation first and correct documentation. Do not change production code merely to simplify reconciliation.

## 3. Accepted Phase 10 State to Preserve

Unless repo inspection disproves it, preserve:
- Phase 10 `COMPLETE`;
- inspect/capability discovery;
- dry-run through existing command semantics;
- transactional apply through authoritative `CommandService`;
- CLI-first automation;
- long-lived machine-readable JSONL stdio;
- stdout protocol-only, stderr diagnostics;
- MCP deferred;
- typed export/job automation;
- no unrestricted shell/filesystem authority;
- durable idempotency across canonical save/reopen and fresh process lifetime;
- exact replay does not duplicate mutation;
- unseen stale revision is explicitly rejected;
- persisted command idempotency state is stored in canonical project state via `command_ledger` or actual equivalent;
- current payload fingerprint is FNV-1a 64-bit, subject to Phase 11 review;
- ledger compaction/pruning remains deferred.

Do not reopen Phase 10 for non-blocking hardening preferences.

## 4. In Scope

### 4.1 `PLAN.md`
Confirm/normalize:
- Phase 10 status `COMPLETE`;
- CLI-first + long-lived stdio + MCP deferred;
- durable idempotency as completed behavior;
- no Phase 10 acceptance item incorrectly remains open.

If already correct, avoid unnecessary rewriting.

### 4.2 `PROJECT_STATUS.md`
Ensure current snapshot records:
- Phase 10 `COMPLETE`;
- automation surface exists;
- CLI-first + JSONL stdio;
- MCP deferred;
- durable save/reopen idempotency;
- export automation;
- debt: ledger growth/pruning;
- debt: fingerprint-strength review;
- next lifecycle step: Phase 11 research gate.

Keep it concise, not a changelog.

### 4.3 `ARCHITECTURE.md` — transport/session
Record:
- initial automation transport = CLI-first;
- long-lived JSONL stdio session;
- stdout reserved for protocol frames;
- stderr reserved for diagnostics/logging;
- CLI is an automation adapter/client, not a second editor;
- session reuses application-owned services;
- MCP deferred;
- future MCP must be a thin transport over the same `CommandService`, `JobManager`, and persistence semantics.

### 4.4 `ARCHITECTURE.md` — durable idempotency
Document:
- successful command/transaction identity survives canonical save/reopen;
- command receipt/idempotency state is project-owned;
- durable structure is `command_ledger` or actual repo symbol;
- exact replay returns/reconstructs prior receipt;
- exact replay does not mutate, increment revision, or create another history edit;
- same identity + different semantic payload is rejected;
- failed atomic transaction persists no successful receipt;
- dry-run persists no ledger entry;
- ledger is canonical persistence state, not CLI/session cache;
- future transports reuse the same semantics.

Do not over-specify private serialization layout unless it is an intentional durable contract.

### 4.5 Phase 11 risk/debt — ledger growth
Add a Phase 11 research/hardening item covering:
- ledger size growth;
- load-time memory overhead;
- save/serialization cost;
- project-open cost;
- long-project degradation;
- whether compaction/pruning is needed;
- invariants pruning must preserve.

Do not design or implement pruning now.

### 4.6 Phase 11 security item — fingerprint strength
Record a Phase 11 research question:
- is FNV-1a 64-bit only accidental-collision detection, or must it resist adversarial collision from untrusted inputs?
- if adversarial resistance is required, should a stronger canonical representation/hash replace it?

Do not replace the hash during reconciliation unless repo inspection reveals an actual blocker invalidating Phase 10 acceptance.

## 5. Out of Scope

Do NOT:
- modify Phase 10 production implementation;
- add CLI features;
- implement MCP;
- implement ledger pruning;
- replace FNV-1a;
- run Phase 11 fixes;
- create `plan_phase11.md`;
- redesign persistence or Command Service;
- broaden/reorder unrelated roadmap phases;
- reopen Phase 9 deferred items.

## 6. Required `ARCHITECTURE.md` Changes

### 6.1 Automation Architecture
Update the existing automation section rather than creating a disconnected duplicate section.

Suggested durable summary:
```text
Initial automation transport: CLI-first
Machine session: long-lived JSONL stdio
stdout: protocol frames only
stderr: diagnostics/logs only
MCP: deferred; future thin adapter over same application services
```

### 6.2 Command Service / Idempotency
Preserve:
```text
inspect -> propose -> dry-run -> apply -> receipt
```

Add:
```text
successful apply
-> durable command ledger entry
-> canonical save
-> reopen
-> exact replay recognized
-> no duplicate mutation
```

### 6.3 Canonical Project Model / Persistence
Clarify:
- ledger is project-owned correctness state;
- it is not rebuildable cache;
- deleting cache must not delete replay protection;
- process lifetime is not the idempotency boundary;
- canonical save/reopen preserves ledger semantics.

### 6.4 Architecture Summary / Invariants
Add only if useful to prevent regression.

Candidate invariant:
> Durable automation idempotency is project-owned and survives canonical save/reopen; transport-local memory is not the source of truth for committed operation receipts.

Candidate summary:
> Automation: CLI-first long-lived JSONL stdio over shared Command Service/Job Manager; MCP deferred.

## 7. Required `PLAN.md` Changes

### Phase 10
Verify/normalize:
- `Status: COMPLETE`;
- completion notes match implementation;
- transport decision resolved;
- durable idempotency completed;
- no misleading open Phase 10 item.

### Phase 11
Add explicit research/hardening inputs:

**Ledger retention/growth**
- long-project ledger growth;
- serialization/open/save/memory impact;
- safe retention/compaction necessity.

**Fingerprint threat model**
- FNV-1a 64-bit suitability;
- accidental vs adversarial collision requirement;
- whether stronger canonical fingerprint is required.

Do not preselect a replacement hash or pruning algorithm before research.

## 8. Required `PROJECT_STATUS.md` State

Final snapshot should communicate approximately:
```text
Current phase:
Phase 10 COMPLETE

Completed:
- headless automation surface
- inspect / dry-run / apply
- CLI-first
- long-lived JSONL stdio
- export job automation
- durable save/reopen idempotency

Deferred / hardening inputs:
- MCP
- command ledger retention/compaction review
- idempotency fingerprint-strength review

Next:
Phase 11 research gate — reliability/security/performance hardening
```

Use repo conventions instead of copying literally.

## 9. ADR Decision

Default: no new ADR if `ARCHITECTURE.md` cleanly captures the already-implemented decisions.

Create/update an ADR only if repo conventions require it or inspection reveals a broader durable decision.

If none is needed, report:
`ADR not required; reconciliation captured in ARCHITECTURE.md.`

## 10. Validation

This is documentation/state reconciliation, but claims must be verified against repo.

Minimum verification:
1. inspect actual CLI/session implementation;
2. inspect actual persisted `command_ledger`/equivalent;
3. confirm actual fingerprint is FNV-1a 64-bit before documenting;
4. inspect save/load path proving ledger persistence;
5. inspect current `PLAN.md`;
6. inspect current `PROJECT_STATUS.md`;
7. inspect current `ARCHITECTURE.md`;
8. run doc formatting/lint if repo provides it.

Do not run a broad production suite unless repo policy requires tests after documentation-only edits.

If a report claim is unsupported by code, stop and report the discrepancy rather than documenting it as fact.

## 11. Acceptance Criteria

- **REC-10-01:** `PLAN.md` records Phase 10 `COMPLETE`.
- **REC-10-02:** `PROJECT_STATUS.md` reflects accepted state and Phase 11 research next.
- **REC-10-03:** `ARCHITECTURE.md` records CLI-first + long-lived JSONL stdio.
- **REC-10-04:** `ARCHITECTURE.md` records MCP deferred and future thin-adapter constraint.
- **REC-10-05:** architecture records durable save/reopen idempotency and project-owned ledger semantics.
- **REC-10-06:** ledger is clearly canonical persistence state, not session cache.
- **REC-10-07:** Phase 11 durable state includes ledger growth/retention research.
- **REC-10-08:** Phase 11 durable state includes fingerprint threat-model review.
- **REC-10-09:** no pruning or hash replacement implemented.
- **REC-10-10:** no unrelated phase scope changed.
- **REC-10-11:** no production code changed unless reconciliation finds a blocking contradiction requiring escalation.
- **REC-10-12:** no `plan_phase11.md` created.
- **REC-10-13:** final lifecycle state is:
```text
Phase 10 COMPLETE
-> reconciliation complete
-> Phase 11 research gate next
```

## 12. Codex Completion Report Requirements

Report:

### Summary
- docs updated;
- any implementation discrepancy found.

### Repo verification
Evidence for:
- CLI-first / JSONL stdio;
- persisted command ledger;
- save/reopen replay semantics;
- FNV-1a 64-bit fingerprint.

### Files changed
Expected primarily:
- `ARCHITECTURE.md`;
- `PLAN.md`;
- `PROJECT_STATUS.md`;
- ADR only if justified.

### Reconciliation details
State exactly:
- Phase 10 architecture reality added;
- durable idempotency documentation added;
- ledger-growth Phase 11 item added;
- fingerprint-review Phase 11 item added.

### Validation
List checks run.

### Scope confirmation
Explicitly confirm:
- no Phase 10 production code changed;
- no Phase 11 implementation started;
- no `plan_phase11.md` created;
- no MCP implementation added.

### Architecture impact
State:
- `ARCHITECTURE.md updated; no ADR required`, or
- ADR created/updated with justification.

### Final state
Recommend:
```text
Phase 10 reconciliation: COMPLETE
Next action: Phase 11 repo-first hardening research
```

## 13. Stop / Escalation Conditions

Stop and report instead of silently editing around the issue if:
- Phase 10 implementation materially contradicts the accepted report;
- ledger is not actually persisted across canonical save/reopen;
- FNV-1a statement is false/materially different;
- CLI bypasses authoritative `CommandService`;
- automation bypasses canonical persistence;
- architecture update would require changing source-of-truth or mutation-path invariants;
- truthful reconciliation would require reopening Phase 10 implementation.

## 14. Next Lifecycle Step After Acceptance

After ChatGPT reviews and accepts this reconciliation:
1. mark Phase 10 durable state reconciled;
2. move to `01 — Architecture & Repo Research`;
3. perform Phase 11 repo-first hardening research;
4. define measurable reliability/security/performance findings and thresholds;
5. only then create `plan_phase11.md`.

Do not skip the Phase 11 research gate.
