# plan_phase17.md — Phase 17: Performance Requalification & Release Foundation

**Project:** TinixCut  
**Phase:** 17  
**Title:** Performance Requalification & Release Foundation  
**Plan status:** AMENDED — SELF-REVIEWED AND LOCKED FOR IMPLEMENTATION
**Implementation status:** QUALIFICATION RUN COMPLETE — PHASE NOT COMPLETE
**Research gate:** COMPLETE — PERFORMANCE CONTRACT AMENDED
**Research reconciliation baseline:** `8b9f752` (`docs: correct phase 17 research reconciliation`)  
**Schema:** v3 — invariant for this phase  
**ADR-006:** NOT REQUIRED  
**Primary platform:** Windows 10/11 x86_64  
**Phase character:** Qualification-first; no speculative production optimization

---

## 1. Execution Gate

This plan is the authoritative Phase-17 implementation plan. The user has delegated planning, research, implementation, review, and reconciliation to the current agent; completion is evidence-based and requires independent self-review.

Required lifecycle:

```text
Research and calibration evidence
→ durable research/plan amendment
→ planning checkpoint commit
→ Codex implementation
→ implementation report
→ independent self-review
→ phase completion decision
→ durable project-state reconciliation
```

Before implementation begins, Codex must verify that the repository HEAD contains the accepted research reconciliation (`8b9f752` or a descendant containing it) and that the working tree has no unexplained changes.

---

## 2. Objective

Phase 17 establishes a reproducible release-qualification foundation for the current Schema-v3 product without changing creator semantics.

The phase must:

1. turn the accepted Phase-17 research workloads into deterministic repository-owned fixtures;
2. add an explicit, opt-in release performance qualification harness;
3. requalify current Schema-v3 scale, transport, and CPU export performance using the accepted methodology and budgets;
4. synchronize the existing Phase-12 qualification harness with current Schema-v3 semantics;
5. execute the accepted Host-Isolated Workstation qualification contract (`CM-01..CM-07`);
6. rebuild and validate the current NSIS package using the actual Phase-17 codebase;
7. produce durable release evidence containing hashes, environment metadata, benchmark results, and qualification outcomes;
8. keep external release gates explicit rather than pretending they passed.

This is **not** an optimization phase. A faster implementation is not a goal by itself.

---

## 3. Durable Inputs / Source of Truth

Implementation must conform to the latest reconciled state in:

- `RESEARCH.md` — Phase-17 accepted `PR-01..PR-18`, benchmark baselines, Fixture A/B, MCP correction, packaging policy, CM contract;
- `PLAN.md` — Phase-17 roadmap-level scope and status;
- `PROJECT_STATUS.md` — Phase-17 research complete, implementation not started;
- `ARCHITECTURE.md`;
- ADR-001 through ADR-005;
- `src-tauri/tauri.conf.json`;
- current repository code.

If this plan conflicts with repository reality, Codex must stop and report the discrepancy rather than silently changing the research contract.

---

## 4. Accepted Baseline

At plan authoring time:

```text
Phases 0–15: COMPLETE
Phase 16: COMPLETE WITH DEFERRED ITEMS
P16-AC-56: DEFERRED / NOT PASS
Phase 17 Research Gate: COMPLETE — ACCEPTED; transport calibration amendment recorded 2026-10-07
Phase 17 Implementation: NOT STARTED
Current Active Implementation Phase: Phase 17
Schema: v3
ADR-006: NOT REQUIRED
Accepted reconciliation commit: 8b9f752
```

The previously accepted automated Phase-16 baseline is:

```text
Rust: 198 passed
Frontend: 37 passed
cargo clippy -- -D warnings: PASS
cargo fmt -- --check: PASS
cargo build: PASS
TypeScript typecheck: PASS
Frontend production build: PASS
```

Phase 17 must preserve all existing feature behavior unless a change is strictly required to keep qualification infrastructure correct.

---

## 5. Normative Research Contracts

The implementation is governed by the reconciled Phase-17 research contract.

### PR-01 — Dual deterministic Schema-v3 workloads

Use two deterministic workloads:

- Fixture A: semantic-rich Schema-v3 workload;
- Fixture B: 20-track / 200-item Schema-v3 scale workload.

### PR-02 — Benchmark methodology

Performance measurements use release builds, warmups, repeated samples, and median/P95/aggregate reporting where applicable.

### PR-03 — No wall-clock gating in ordinary CI

Normal functional CI must never fail because of machine-dependent wall-clock latency or export RTF.

### PR-04 — Explicit qualification suite

Performance budgets execute only through an explicit opt-in release qualification invocation.

Accepted form:

```text
cargo test --release --test phase17_performance_benchmarks -- --ignored --nocapture
```

### PR-05 — Performance classification

Every timing result must be classified as one of:

- Qualification Ceiling;
- Reference-Host Regression Budget;
- Informational Baseline.

### PR-06 — Preview terminology

`sample_preview_at` measures **preview semantic sampling latency** only.

It does not qualify:

- physical display frame pacing;
- WebView compositor presentation;
- visible dropped frames;
- physical A/V sync;
- tearing.

### PR-07 — CPU export authority

CPU / bundled `libx264` is the sole required authoritative export baseline.

Hardware encoders remain best-effort paths with required software fallback.

### PR-08 — Reference-host CPU export budgets

Using `libx264`, preset `fast`, CRF 23:

```text
720p30: RTF >= 2.20x
1080p30: RTF >= 1.00x
```

These are reference-host regression budgets, not universal minimum-hardware guarantees.

### PR-09 — Export validity

Qualified output must satisfy the accepted H.264/AAC container/stream contract and timeline duration invariance.

Do not claim physical A/V sync or live playback frame behavior from file probing.

### PR-10 — Phase-16 scale command budget

On Fixture B:

```text
Reference-host median < 200 µs
```

for the accepted Phase-16 command workloads.

### PR-11A — JSONL steady-state budget

Real OS-stdio, long-lived JSONL session, in-memory operations with `saveAfter:false`:

```text
Fixture-B reference-host median < 150 µs
```

This is not durable-save latency.

### PR-11B — MCP inspect/dry-run

Real OS-stdio:

```text
Fixture-B reference-host median < 5.0 ms
Explicit qualification ceiling < 20 ms
```

### PR-11C — MCP persistent apply

Real OS-stdio, including project reload + CommandService mutation + atomic save:

```text
Reference-host median < 20 ms
Explicit qualification ceiling < 100 ms
```

### PR-12 — True Clean VM

True clean Windows 10/11 VM qualification remains:

```text
DEFERRED / EXTERNAL RELEASE GATE
```

### PR-13 — Host-isolated qualification

`CM-01..CM-07` complements but does not replace True Clean VM qualification.

### PR-14 — P16-AC-56

P16-AC-56 remains:

```text
DEFERRED / NOT PASS
```

It is an external human pre-release GUI qualification.

### PR-15 — Installer artifact policy

Generated installer/release executables are transient build artifacts and must not be committed to Git.

### PR-16 — Durable evidence

Durable qualification evidence records:

- SHA-256 hashes;
- product/version metadata;
- Git provenance;
- environment metadata;
- benchmark results;
- qualification results/logs.

### PR-17 — Schema/scope invariant

Schema v3 remains unchanged.

No Schema v4 or new creator features are authorized.

### PR-18 — Qualification-first

Production optimization/refactoring is unauthorized unless an accepted ceiling is violated and the remediation scope is explicitly approved.

---

## 6. In Scope

Phase 17 includes exactly these workstreams.

### 6.1 Deterministic Fixture A

Implement a repository-owned deterministic builder for the accepted semantic-rich Schema-v3 workload.

Required semantic topology:

```text
Schema: v3
Duration: 6.0 seconds
Frame rate: 30 fps
Variants:
- 1280x720
- 1920x1080

Tracks:
- 2 video
- 1 audio

Content:
- 2 sequential primary video clips
- 1 title/text overlay item
- 1 audio bed
- 1 cut-owned CrossDissolve
- nonlinear CubicBezier visual keyframes
- audio volume automation
```

The fixture must use stable deterministic IDs and exact RationalTime values.

Synthetic media required for export qualification should be reproducibly generated from app-owned/bundled FFmpeg or the existing qualification infrastructure. Large generated media must not be committed.

### 6.2 Deterministic Fixture B

Implement a repository-owned deterministic builder for the accepted scale workload.

Exact topology:

```text
Schema: v3
Duration: 10.0 seconds
Frame rate: 30 fps
Tracks: 20
- 14 video
- 4 audio
- 2 text/overlay

Items: exactly 200
- 10 sequential items per track
- 30 frames per item

CrossDissolves: exactly 20
- Video Tracks 0..3
- 5 pairwise adjacent transitions per track

Visual nonlinear keyframes: 60+
Audio volume keyframes: 120
```

Fixture B must exercise modern Schema-v3 structures and must not fall back to Phase-11/12 Schema-v1 mock semantics.

### 6.3 Performance qualification harness

Add a dedicated performance qualification suite, preferably:

```text
src-tauri/tests/phase17_performance_benchmarks.rs
```

The suite must:

- be opt-in / ignored for ordinary `cargo test`;
- compile and run under release profile;
- record environment metadata;
- distinguish Qualification Ceilings from Reference-Host Budgets;
- emit enough diagnostics to review failures;
- validate benchmark operations actually succeeded before recording timing.

### 6.4 Schema-v3 persistence/compile/sample qualification

Using Fixture B, measure:

- `save_project_atomic`;
- `load_project`;
- `CompositionPlan::from_project`;
- `sample_preview_at`.

Sampling cases must cover, where present:

- ordinary frame;
- dense active layers;
- CrossDissolve;
- nonlinear Bézier evaluation;
- combined transition + automation.

All preview timing must be labeled `preview semantic sampling latency`.

### 6.5 Phase-16 command scale qualification

Using Fixture B, measure representative:

- `ClipGroupMove` dry-run;
- `ClipGroupMove` execute;
- `ClipRippleDelete` dry-run;
- batched multi-delete dry-run.

The group workload must be nontrivial and exercise multiple tracks/items.

### 6.6 Real JSONL OS-stdio qualification

Use the real release executable and actual OS pipes.

Required executable mode:

```text
tinixcut.exe session
```

Measure steady-state:

- inspect;
- dry-run;
- apply with `saveAfter:false`.

Process startup must be recorded separately or excluded explicitly.

Responses must be parsed and validated; errors must never be timed as successful operations.

### 6.7 Real MCP OS-stdio qualification

Use the real release executable:

```text
tinixcut.exe mcp
```

Required:

- real Windows stdin/stdout pipes;
- correct MCP initialization;
- canonical required `_meta`;
- unique JSON-RPC request IDs;
- matching response IDs;
- zero pipelining during latency samples;
- response parsing;
- assert no JSON-RPC error;
- assert tool `isError == false`.

Measure:

- `tinixcut_inspect`;
- `tinixcut_dry_run`;
- `tinixcut_apply`.

For every measured apply:

- unique ADR-001 operation identity;
- current correct base revision;
- actual successful mutation;
- exactly one revision increment;
- actual atomic persistence;
- independent reload confirms mutation.

The invalid historical ~0.06 ms MCP value must never be reused as a baseline.

### 6.8 CPU/libx264 export qualification

Fixture A must qualify 720p30 and 1080p30 CPU export.

Profile:

```text
video codec: libx264
preset: fast
CRF: 23
audio: AAC
```

For each profile:

- 1 warmup run minimum;
- 5 measured runs;
- capture every wall time;
- compute median wall time;
- compute observed RTFs;
- probe output;
- validate stream/container contract.

Reference-host budgets:

```text
720p30: median/reference result must satisfy RTF >= 2.20x
1080p30: median/reference result must satisfy RTF >= 1.00x
```

Do not call these universal product guarantees.

### 6.9 Phase-12 qualification harness synchronization

Synchronize the actual existing qualification implementation:

```text
src-tauri/src/modules/media/qualification.rs
tests/clean_machine_qualification.ps1
```

The obsolete representative Schema-v1 transition/keyframe mock must be updated to current Schema-v3 structures.

Required modern semantics include:

- track-owned `CutTransition`;
- `CrossDissolve`;
- nonlinear keyframe representation where qualification needs modern semantics;
- current project schema;
- current bundled binary resolver behavior.

Do not create or reference nonexistent `src-tauri/tests/qualification.rs`.

### 6.10 Host-Isolated Workstation qualification

Implement/execute the accepted `CM-01..CM-07`.

#### CM-01 — Bundled Binary Self-Sufficiency

Run qualification with developer FFmpeg/toolchain paths stripped from the relevant runtime `PATH`.

Packaged application/media operations must resolve bundled FFmpeg/ffprobe through `MediaBinaryResolver`.

#### CM-02 — Mandatory Software Fallback

When hardware acceleration is unavailable or fails capability/driver requirements, export must fall back to bundled CPU `libx264` without process crash or project corruption.

This does not qualify NVENC/QSV/AMF output.

#### CM-03 — Spaces and Unicode Path Robustness

Use projects/media under paths containing:

- spaces;
- non-ASCII Unicode.

Verify required operations such as:

- open/load;
- save/reopen;
- probe/import;
- export.

#### CM-04 — Read-Only Media Protection

External read-only source media must be probed/read/rendered without mutation attempts against the source.

#### CM-05 — Atomic Persistence & Recovery

Exercise the existing save/recovery safety path such that interrupted/failed persistence cannot truncate the last valid canonical project.

Do not introduce destructive crash simulation outside existing safe test mechanisms.

#### CM-06 — Clean User-Scope Installation

Generate and validate the current NSIS installer with:

```text
installMode = currentUser
WebView2 mode = downloadBootstrapper
```

No admin elevation is required by the intended install mode.

Verify expected installed executable/shortcut state using the current automation capabilities of the environment.

#### CM-07 — User Data Preservation on Uninstall

Uninstall must remove application-owned installation files while preserving:

- `.tcproj` project files;
- external source media;
- rendered export files.

Caches/rebuildable derived data may be removed according to existing product policy.

### 6.11 Package synchronization

Build current production assets and the NSIS bundle from the accepted Phase-17 codebase.

Authoritative current metadata:

```text
productName: TinixCut
version: 0.1.0
identifier: com.tinixcut.editor
WebView2 install mode: downloadBootstrapper
NSIS installMode: currentUser
languages: English
```

Expected installer location/name:

```text
src-tauri/target/release/bundle/nsis/TinixCut_0.1.0_x64-setup.exe
```

If Tauri derives a materially different filename from actual current config/tooling, report it; do not rename binaries solely to satisfy this text.

Installer and release binaries remain untracked transient artifacts.

### 6.12 Durable release evidence

Create durable machine-readable qualification evidence.

Preferred location:

```text
docs/qualification/phase17_release_qualification.json
```

A concise human-readable companion may also be created if useful.

At minimum evidence must contain:

- Phase number/title;
- Git commit under qualification;
- clean/dirty working-tree state;
- TinixCut version;
- Schema version;
- OS/build;
- CPU;
- RAM;
- storage class/model when available;
- GPU/driver;
- bundled FFmpeg version;
- SHA-256 of FFmpeg;
- SHA-256 of ffprobe;
- SHA-256 of generated installer;
- fixture definitions/version identifiers;
- benchmark methodology;
- timing classifications;
- measured results;
- budget pass/fail;
- CM-01..CM-07 results;
- P16-AC-56 still deferred;
- True Clean VM still deferred;
- generated artifact paths;
- timestamp of qualification.

Do not embed generated installer binaries in Git.

---

## 7. Explicitly Out of Scope

The following are prohibited in Phase 17 unless the phase is formally revised:

- qualifying NVENC output;
- qualifying QSV output;
- qualifying AMF output;
- upgrading GPU drivers to make hardware qualification possible;
- changing the hardware fallback architecture;
- hardware decode implementation;
- True Clean Windows VM provisioning/automation;
- marking True Clean VM PASS;
- automated closure of P16-AC-56;
- claiming P16-AC-56 PASS;
- GUI automation framework introduction;
- Playwright/CDP desktop automation;
- audio crossfade;
- waveform/spectral feature expansion;
- graph/curve editor;
- new transition kinds;
- text/caption feature expansion;
- ripple insert;
- ripple trim/Q-W editing;
- magnetic timeline;
- track targeting;
- persistent marker features;
- Schema v4;
- GPU compositor redesign;
- preview architecture redesign;
- renderer/filtergraph redesign;
- CommandService redesign;
- MCP protocol redesign;
- dependency/framework upgrades unrelated to qualification;
- speculative performance optimization when budgets already pass.

---

## 8. Architecture Constraints

### 8.1 Schema invariant

No project schema migration is allowed.

`CURRENT_SCHEMA_VERSION` remains 3.

### 8.2 Command authority

No direct mutation bypass may be introduced.

Existing `CommandService` remains authoritative.

### 8.3 Media binary resolution

Packaged runtime must use the app-owned `MediaBinaryResolver` policy established by ADR-002.

Do not introduce system-PATH dependency in packaged mode.

### 8.4 Export baseline

CPU `libx264` remains the authoritative required baseline.

Hardware paths must not become required for project correctness or qualification completion.

### 8.5 Transport semantics

JSONL and MCP semantics remain unchanged.

The benchmark harness must measure current behavior; it must not add transport-specific edit semantics.

### 8.6 Preview semantics

No preview/backend architecture change is authorized.

`sample_preview_at` qualification covers semantic evaluation only.

### 8.7 ADR policy

ADR-001 through ADR-005 remain valid.

ADR-006 is not required unless implementation discovers a truly new cross-cutting architectural decision.

If such a decision becomes necessary, STOP before implementing it.

---

## 9. Benchmark Methodology Contract

### 9.1 Reference host

The accepted research reference host is:

```text
CPU: Intel Core i5-13400F
RAM: 64 GB
OS: Windows 11 Pro 64-bit
Storage: Samsung 990 PRO 2 TB NVMe
GPU: NVIDIA RTX 3090
Driver: 595.79
QSV: unavailable
AMF: unavailable
FFmpeg: bundled Gyan 9.0.2 Essentials
```

If the implementation/qualification host materially differs, the report must say so.

Reference-host budgets must not be silently applied to a different host as universal hardware requirements.

### 9.2 Micro/medium operations

Default accepted methodology:

```text
warmups: 5
measured samples: >= 50 where practical
statistics:
- min
- median
- P95
- max
```

For operations intentionally measured at N=30 in accepted research, N>=30 is permitted if the plan/report identifies it.

### 9.3 Transports

For JSONL/MCP:

- launch subprocess once;
- complete protocol/session initialization once;
- discard warmups;
- sequential measured requests;
- no pipelining;
- validate matching responses;
- record process startup separately;
- record handshake separately where applicable.

### 9.4 Export

For each 720p/1080p profile:

```text
warmup: >= 1
measured runs: 5
statistics:
- every wall time
- min/median/max wall time
- every RTF
- min/median/max RTF
```

Do not manufacture P95 from five export samples.

### 9.5 Ordinary CI isolation

The ordinary functional suite must not fail because a machine is slower.

Performance tests must be marked/structured so normal:

```text
cargo test
```

does not enforce wall-clock budgets.

---

## 10. Qualification Ceilings and Reference Budgets

### 10.1 Fixture-B qualification ceilings

For the explicit opt-in qualification job only:

```text
save_project_atomic: < 200 ms
load_project: < 200 ms
CompositionPlan::from_project: < 20 ms
preview semantic sampling: < 10 ms
Phase-16 command workloads: < 10 ms
JSONL steady-state roundtrip: < 10 ms
MCP inspect/dry-run: < 20 ms
MCP persistent apply: < 100 ms
```

These are catastrophic-regression guardrails, not ordinary-CI timing assertions.

### 10.2 Reference-host regression budgets

On the accepted reference host:

```text
Fixture B save median: < 10 ms
Fixture B load median: < 10 ms
Fixture B compile median: < 1 ms
Fixture B semantic sample median: < 500 µs
Phase-16 command median: < 200 µs
Fixture-B JSONL OS-stdio in-memory median: < 150 µs
Fixture-B MCP inspect median: < 5 ms
Fixture-B MCP dry-run median: < 5 ms
MCP persistent apply median: < 20 ms
720p30 CPU export: RTF >= 2.20x
1080p30 CPU export: RTF >= 1.00x
```

### 10.3 Informational measurements

Record without blocking ordinary CI:

- JSONL process startup;
- MCP process startup;
- MCP handshake;
- additional environment diagnostics.

---

## 11. Known Research Baselines

These values are comparison baselines, not hardcoded output expectations.

### Fixture B

```text
save median ≈ 3.890 ms
save P95 ≈ 5.911 ms

load median ≈ 2.985 ms
load P95 ≈ 3.949 ms

compile median ≈ 0.277 ms
compile P95 ≈ 0.407 ms

semantic sampling medians ≈ 0.18–0.25 ms
```

### Phase-16 scale commands

```text
ClipGroupMove dry-run median ≈ 63 µs
ClipGroupMove execute median ≈ 64 µs
ClipRippleDelete dry-run median ≈ 49 µs
multi-delete dry-run median ≈ 53 µs
```

### JSONL real OS-stdio

```text
inspect median ≈ 23 µs
dry-run median ≈ 18 µs
apply saveAfter:false median ≈ 19 µs
```

### MCP real OS-stdio — corrected valid values

```text
inspect median ≈ 0.238 ms
dry-run median ≈ 0.242 ms
persistent apply median ≈ 12.201 ms
persistent apply P95 ≈ 14.311 ms
```

Historical invalid MCP ~0.06 ms measurements must be explicitly ignored.

### CPU export

```text
720p30:
median wall ≈ 2.343 s
median RTF ≈ 2.56x
minimum observed RTF ≈ 2.42x

1080p30:
median wall ≈ 4.791 s
median RTF ≈ 1.25x
```

---

## 12. Implementation Workstreams

### Workstream A — Planning checkpoint

After this planning amendment checkpoint commit and before production/qualification implementation:

1. add the exact approved `Plans/plan_phase17.md`;
2. update durable planning status only as required;
3. create a planning checkpoint commit;
4. confirm no Phase-17 implementation code is included in that checkpoint.

Suggested commit:

```text
docs: approve phase 17 performance release plan
```

Do not begin implementation before this checkpoint exists.

### Workstream B — Fixture infrastructure

Implement deterministic Fixture A and Fixture B generators.

Prefer reusable test/qualification support helpers over duplicated fixture construction.

Do not move canonical product state into test-only alternative domain models.

### Workstream C — Benchmark qualification suite

Create the opt-in release benchmark test.

Ensure timing code:

- validates operation success;
- does not accidentally time error paths;
- does not reuse stale/replayed mutation identities for MCP apply;
- emits clear diagnostics.

### Workstream D — Transport qualification

Implement actual release-process JSONL and MCP OS-stdio benchmark clients in test/qualification code.

Reuse production protocol shapes rather than inventing shadow semantics.

### Workstream E — Export qualification

Generate deterministic Fixture-A media, execute CPU exports, probe outputs, and record RTF.

### Workstream F — Phase-12 harness modernization

Update the existing qualification representative project to Schema v3 semantics.

Preserve Phase-12 binary resolution/fallback behavior.

### Workstream G — Host-isolated qualification

Update/extend `tests/clean_machine_qualification.ps1` or the existing qualification orchestration to implement CM-01..CM-07.

### Workstream H — Packaging/evidence

Produce the current NSIS installer, hash it, run the host-isolated qualification, and persist qualification evidence.

### Workstream I — Documentation

Update durable docs only after actual Phase-17 qualification results exist.

Do not mark external gates PASS.

---

## 13. Expected Files / Modules

Likely affected locations include:

```text
Plans/plan_phase17.md

src-tauri/tests/phase17_performance_benchmarks.rs
src-tauri/tests/... phase17 fixture/support modules as appropriate

src-tauri/src/modules/media/qualification.rs
tests/clean_machine_qualification.ps1

docs/qualification/phase17_release_qualification.json
docs/qualification/... optional human-readable report

PLAN.md
PROJECT_STATUS.md
RESEARCH.md only if execution proves a research assumption inaccurate
DEVELOPMENT_WORKFLOW.md only if a durable qualification command must be documented
```

Production modules outside qualification/test infrastructure should not require semantic changes.

If Codex discovers that a feature/runtime production edit is necessary, use the stop conditions below.

---

## 14. Error and Failure Handling

### 14.1 Benchmark operation failure

If the benchmarked operation returns an error:

- report the operation error;
- do not record its response time as successful performance data;
- classify the relevant acceptance criterion FAIL.

### 14.2 Performance budget failure

If an accepted qualification ceiling or applicable reference-host budget fails:

1. rerun enough to rule out an obvious harness/environment mistake;
2. record exact host/load conditions and variance;
3. identify the bottleneck;
4. STOP before production optimization;
5. report the smallest remediation proposal.

No speculative optimization is authorized by this plan.

### 14.3 Host mismatch

If the host is not materially the accepted reference host:

- still run functional and qualification-ceiling checks where meaningful;
- do not claim reference-host budget PASS/FAIL as if hardware were identical;
- report `REFERENCE-HOST BUDGET NOT QUALIFIED ON THIS HOST`.

Because Phase 17 is specifically requalifying the accepted reference workstation, inability to run the reference-host budget may keep the phase incomplete.

### 14.4 Installer failure

If Tauri/NSIS package generation fails:

- diagnose within packaging/configuration scope;
- do not change product architecture;
- record exact missing runtime/tool dependency.

### 14.5 Hardware accelerator failure

NVENC/QSV/AMF failures are expected/non-blocking if:

- software fallback succeeds;
- the failure is not a regression in the fallback ladder;
- no hardware backend is claimed qualified.

### 14.6 External gates

Do not fail Phase 17 solely because:

- True Clean VM is unavailable;
- P16-AC-56 remains unexecuted.

They remain explicit external/deferred release gates.

---

## 15. Quality Gates

Before Codex proposes Phase-17 completion, all applicable standard gates must pass:

```text
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
cargo build

pnpm test -- --run
pnpm run typecheck
pnpm run build
```

In addition:

```text
cargo test --release --test phase17_performance_benchmarks -- --ignored --nocapture
```

must complete successfully in the declared qualification environment.

The actual repository package/NSIS build command must also complete and be reported exactly.

If the repository's documented commands differ in syntax, use the documented command while preserving the gate intent.

---

## 16. Host-Isolated Qualification Evidence Requirements

Each `CM-01..CM-07` must return exactly one status:

```text
PASS
FAIL
BLOCKED
```

For each item report:

- setup/environment;
- operation performed;
- expected behavior;
- observed behavior;
- artifact/log reference;
- cleanup performed.

A `BLOCKED` CM item that is in Phase-17 scope prevents `COMPLETE`; report the blocker and keep the phase open pending resolution or explicit user disposition.

Do not silently convert a blocked CM item into PASS using unit-test evidence.

---

## 17. Packaging Evidence Requirements

The completion report must include:

- authoritative app version;
- authoritative product/identifier;
- WebView2 mode;
- NSIS install mode;
- exact package command;
- installer path;
- installer size;
- installer SHA-256;
- FFmpeg SHA-256;
- ffprobe SHA-256;
- confirmation generated binaries are ignored/untracked;
- installation qualification result;
- uninstall qualification result;
- final `git status --short`.

If generated installer artifacts appear tracked or staged, Phase 17 cannot be accepted until artifact hygiene is corrected.

---

## 18. Regression Protection

Phase 17 must prove that qualification infrastructure did not regress prior functionality.

Required regression evidence includes:

- all Rust tests;
- all frontend tests;
- P13 transition behavior;
- P14 keyframe/easing behavior;
- P15 MCP behavior;
- P16 group/ripple/selection/snapping logic through existing automated suites;
- Schema v3 load/save compatibility;
- CPU export correctness.

No existing acceptance test may be removed or weakened solely to make Phase 17 pass.

---

## 19. Documentation Rules During Implementation

### Before implementation

Only the self-reviewed plan amendment and its planning checkpoint may be added.

### During implementation

Do not prematurely mark Phase 17 complete in `PLAN.md` or `PROJECT_STATUS.md`.

### On implementation completion report

The agent reports the evidence-backed proposed status and completes an independent self-review against every acceptance criterion.

### Durable completion reconciliation

A separate durable completion reconciliation is mandatory.

That reconciliation must record:

- completed Phase-17 scope;
- actual benchmark baseline;
- actual package hashes;
- CM results;
- any deviations;
- external deferred gates;
- technical debt;
- architecture/ADR implications;
- dependencies affecting later phases.

---

## 20. Explicit Stop Conditions

Codex must STOP and report before continuing if any of these occur:

1. implementation requires Schema v4;
2. a new creator-facing editing feature is required;
3. `CommandService` architecture must change;
4. preview/compositor architecture must change;
5. export semantics/filtergraph must change to meet a performance number;
6. MCP protocol/tool semantics must change;
7. JSONL semantics must change;
8. hardware encoder qualification becomes necessary for Phase-17 completion;
9. a GPU driver upgrade is required as an implementation step;
10. True Clean VM provisioning is required for completion;
11. P16-AC-56 is proposed as PASS without real interactive qualification;
12. a new cross-cutting architectural decision requiring ADR-006 appears;
13. reference-host benchmark fails and remediation requires nontrivial production optimization/refactor;
14. Fixture A/B cannot be implemented without changing canonical project semantics;
15. packaging requires committing generated installer binaries;
16. actual repository configuration contradicts the accepted packaging contract;
17. actual benchmark behavior invalidates an accepted PR contract;
18. implementation requires an unrelated dependency/framework upgrade.

Stop report must include evidence, impact, and smallest available options.

---

## 21. Acceptance Criteria

The following criteria are normative and must not be renumbered or silently rewritten by Codex.

### Planning / governance

**P17-AC-01** — The accepted Phase-17 research reconciliation commit `8b9f752` (or a descendant containing it) is present before implementation.

**P17-AC-02** — The exact user-approved `Plans/plan_phase17.md` is committed in a planning checkpoint before Phase-17 implementation code.

**P17-AC-03** — The planning checkpoint contains no Phase-17 implementation/qualification code.

**P17-AC-04** — Phase 17 remains Schema v3; no migration/schema-version bump occurs.

**P17-AC-05** — ADR-006 remains unnecessary for the approved qualification-only scope; any new durable architecture decision requires evidence and a corresponding plan amendment before implementation.

### Fixture A

**P17-AC-06** — A deterministic repository-owned Fixture-A builder exists.

**P17-AC-07** — Fixture A is 6.0 seconds at 30 fps and has deterministic 720p/1080p variants.

**P17-AC-08** — Fixture A contains the accepted modern semantics: sequential video clips, title/overlay content, audio bed, one valid track-owned CrossDissolve, nonlinear visual easing, and audio volume automation.

**P17-AC-09** — Fixture A uses Schema-v3 canonical structures and stable deterministic IDs/times.

### Fixture B

**P17-AC-10** — A deterministic repository-owned Fixture-B builder exists.

**P17-AC-11** — Fixture B has exactly 20 tracks: 14 video, 4 audio, 2 text/overlay.

**P17-AC-12** — Fixture B has exactly 200 items, 10 per track, with the accepted 10-second / 30-fps topology.

**P17-AC-13** — Fixture B contains exactly 20 CrossDissolves distributed as 5 pairwise transitions on each of Video Tracks 0..3.

**P17-AC-14** — Fixture B includes at least the accepted 60+ nonlinear visual keyframes and exactly 120 audio volume keyframes.

**P17-AC-15** — Fixture B is Schema-v3 native and does not use legacy item-transition mocks.

### Benchmark harness / CI isolation

**P17-AC-16** — `phase17_performance_benchmarks` (or equivalently named dedicated suite) is opt-in and does not enforce timing budgets during ordinary `cargo test`.

**P17-AC-17** — Performance qualification runs under release profile with warmups and repeated samples.

**P17-AC-18** — Latency reporting includes median and P95 where statistically applicable and never times failed responses as successes.

**P17-AC-19** — Timing rules are explicitly categorized as Qualification Ceilings, Reference-Host Regression Budgets, or Informational Baselines.

**P17-AC-20** — Ordinary frontend/Rust functional CI contains no machine-dependent wall-clock/RTF failure gate.

### Fixture-B performance

**P17-AC-21** — Fixture-B save/load/compile/semantic-sampling measurements are produced and recorded.

**P17-AC-22** — Explicit qualification ceilings pass:
- save < 200 ms;
- load < 200 ms;
- compile < 20 ms;
- semantic sampling < 10 ms.

**P17-AC-23** — On the accepted reference host, reference budgets pass:
- save median < 10 ms;
- load median < 10 ms;
- compile median < 1 ms;
- semantic sample median < 500 µs.

**P17-AC-24** — `sample_preview_at` evidence is described only as preview semantic sampling latency and is not presented as physical playback qualification.

### Phase-16 command scale

**P17-AC-25** — Fixture-B benchmarks cover representative ClipGroupMove dry-run + execute, ClipRippleDelete dry-run, and batched multi-delete dry-run.

**P17-AC-26** — The explicit qualification ceiling for these command workloads is < 10 ms.

**P17-AC-27** — On the accepted reference host, their reference-host median budget is < 200 µs.

### JSONL

**P17-AC-28** — JSONL qualification uses the real release `tinixcut session` subprocess over OS stdin/stdout.

**P17-AC-29** — JSONL steady-state inspect/dry-run/apply responses are parsed, validated, and correlated to the intended request.

**P17-AC-30** — JSONL apply performance is explicitly identified as in-memory `saveAfter:false`, not durable-save latency.

**P17-AC-31** — JSONL explicit qualification ceiling < 10 ms and Fixture-B reference-host median < 150 µs are satisfied. This calibrated threshold supersedes the original < 100 µs reference-host budget under the 2026-10-07 amendment recorded in Section 25; the AC identifier and qualification-ceiling meaning are unchanged.

### MCP

**P17-AC-32** — MCP qualification uses the real release `tinixcut mcp` subprocess over OS stdin/stdout.

**P17-AC-33** — MCP benchmark requests use valid initialization/framing, correct `_meta`, unique JSON-RPC IDs, matching responses, no pipelining, and success assertions.

**P17-AC-34** — MCP inspect and dry-run satisfy:
- qualification ceiling < 20 ms;
- Fixture-B reference-host median < 5.0 ms. This calibrated threshold supersedes the original < 1.0 ms reference-host budget under the 2026-10-07 amendment recorded in Section 25; the AC identifier and qualification-ceiling meaning are unchanged.

**P17-AC-35** — Each measured MCP apply uses unique ADR-001 identity, correct base revision, applies one real mutation, increments revision once, and persists to disk.

**P17-AC-36** — Independent reload verifies MCP apply persistence.

**P17-AC-37** — Fixture-B MCP persistent apply satisfies:
- qualification ceiling < 100 ms;
- reference-host median < 20 ms.

**P17-AC-38** — The invalid historical ~0.06 ms MCP measurement is not used as accepted performance evidence and remains documented as superseded/error-path timing.

### Export

**P17-AC-39** — Fixture-A 720p30 CPU export is run with bundled FFmpeg, `libx264`, preset `fast`, CRF 23, with at least 1 warmup + 5 measured runs.

**P17-AC-40** — Fixture-A 1080p30 CPU export uses the same accepted profile methodology with at least 1 warmup + 5 measured runs.

**P17-AC-41** — Every measured export produces a valid probed H.264/AAC MP4 with the accepted timeline duration.

**P17-AC-42** — On the accepted reference host, 720p30 satisfies RTF >= 2.20x.

**P17-AC-43** — On the accepted reference host, 1080p30 satisfies RTF >= 1.00x.

**P17-AC-44** — Completion evidence does not misrepresent export probing as proof of live dropped-frame performance, physical A/V sync, or cross-encoder bitstream identity.

### Qualification harness modernization

**P17-AC-45** — `src-tauri/src/modules/media/qualification.rs` representative project no longer relies on legacy Schema-v1 transition mock semantics for the Phase-17 current qualification path.

**P17-AC-46** — The current qualification path uses Schema-v3 track-owned CutTransition/CrossDissolve semantics and current keyframe representation where applicable.

**P17-AC-47** — `tests/clean_machine_qualification.ps1` (or the authoritative existing orchestration script if renamed in-repo) remains the host-isolated orchestration entry point; nonexistent paths are not introduced.

### CM-01..CM-07

**P17-AC-48** — CM-01 Bundled Binary Self-Sufficiency = PASS.

**P17-AC-49** — CM-02 Mandatory Software Fallback = PASS.

**P17-AC-50** — CM-03 Spaces and Unicode Path Robustness = PASS.

**P17-AC-51** — CM-04 Read-Only Media Protection = PASS.

**P17-AC-52** — CM-05 Atomic Persistence & Recovery = PASS.

**P17-AC-53** — CM-06 Clean User-Scope Installation = PASS.

**P17-AC-54** — CM-07 User Data Preservation on Uninstall = PASS.

### Packaging / artifact hygiene

**P17-AC-55** — Production frontend/release binaries and the current NSIS installer build successfully from current source.

**P17-AC-56** — Generated package metadata matches authoritative configuration:
- product `TinixCut`;
- version `0.1.0`;
- identifier `com.tinixcut.editor`;
- WebView2 `downloadBootstrapper`;
- NSIS `currentUser`.

**P17-AC-57** — Generated installer and release binaries remain untracked transient artifacts and are not committed.

**P17-AC-58** — Installer, FFmpeg, and ffprobe SHA-256 values are captured in durable qualification evidence.

### Release evidence / deferred gates

**P17-AC-59** — Durable Phase-17 qualification evidence records Git SHA, environment metadata, fixture contract/version, benchmark results, budget statuses, package hashes, and CM-01..CM-07 outcomes.

**P17-AC-60** — True Clean Windows 10/11 VM qualification remains explicitly `DEFERRED / EXTERNAL RELEASE GATE`; it is not marked PASS.

**P17-AC-61** — P16-AC-56 remains explicitly `DEFERRED / NOT PASS`; Phase 17 does not claim automated/manual GUI completion.

### Regression / quality

**P17-AC-62** — Full Rust test suite passes without weakening/removing prior tests.

**P17-AC-63** — Full frontend test suite passes without weakening/removing prior tests.

**P17-AC-64** — `cargo clippy -- -D warnings` passes.

**P17-AC-65** — `cargo fmt -- --check` passes.

**P17-AC-66** — `cargo build` passes.

**P17-AC-67** — `pnpm run typecheck` passes.

**P17-AC-68** — `pnpm run build` passes.

**P17-AC-69** — Explicit release performance qualification command passes in the declared qualification environment.

**P17-AC-70** — The actual NSIS/package build invocation succeeds and is reported verbatim.

### Architecture / scope protection

**P17-AC-71** — No Schema-v4, creator-feature expansion, compositor redesign, CommandService redesign, or transport semantic redesign is introduced.

**P17-AC-72** — No NVENC/QSV/AMF backend is falsely labeled qualified; software fallback remains mandatory.

**P17-AC-73** — No generated installer/release binary is staged or committed.

**P17-AC-74** — No production optimization/refactor is introduced solely to improve already-passing benchmarks.

**P17-AC-75** — `ARCHITECTURE.md` / ADR-001..005 remain consistent; ADR-006 remains unnecessary unless new evidence requires a durable architecture decision and plan amendment.

### Completion reporting

**P17-AC-76** — Codex completion report maps every `P17-AC-01..76` to PASS / FAIL / BLOCKED with concrete evidence.

**P17-AC-77** — Codex report identifies all files changed, generated-but-untracked artifacts, benchmark environment, package artifact path/hash, known issues, deviations, and technical debt.

**P17-AC-78** — The agent does not mark Phase 17 COMPLETE in durable roadmap docs before independent self-review and mandatory reconciliation.

### Reconciliation checkpoint — 2026-10-07

The release benchmark suite passed 5/5, host-isolated CM-01..CM-07 passed 7/7, the current NSIS package built, and the required functional and build gates passed. Durable results and package hashes are recorded in `tests/evidence/phase17_release_qualification_report.json`; detailed host-isolated outcomes are in `tests/evidence/clean_machine_qualification_report.json`.

Phase 17 remains **NOT COMPLETE** because P17-AC-03 failed: the planning checkpoint `36e19af` includes Phase-17 transport calibration/qualification code. The commit history is retained as-is. CM-05 in the PowerShell harness is only an interrupted-write simulation; Rust storage tests separately cover application atomic-save/recovery behavior. True clean Windows 10/11 VM qualification and P16-AC-56 interactive GUI QA remain `DEFERRED / NOT PASS`. Do not promote this phase to COMPLETE.

---

## 22. Expected Completion Classification

If all in-scope acceptance criteria pass and only the already-external gates remain deferred:

```text
Expected candidate classification:
COMPLETE WITH DEFERRED ITEMS
```

The expected deferred items would include at least:

```text
P16-AC-56 — interactive Windows GUI qualification
True Clean Windows 10/11 VM qualification
Hardware encoder qualification (NVENC/QSV/AMF)
```

However, Codex must not assume this classification.

The agent determines and documents the final status after independent self-review and reconciliation.

If any in-scope `CM-01..CM-07`, package, functional, or required qualification criterion fails, the phase is not complete.

---

## 23. Codex Completion Report Requirements

Codex must return a single Phase-17 completion report containing:

1. repository HEAD before implementation;
2. planning checkpoint SHA;
3. final implementation HEAD / working-tree state;
4. files changed;
5. Fixture-A exact generated topology;
6. Fixture-B exact generated topology;
7. benchmark methodology;
8. reference-host metadata;
9. Fixture-B save/load/compile/sample results;
10. Phase-16 command results;
11. JSONL subprocess results;
12. MCP subprocess results;
13. MCP apply persistence proof;
14. repeated 720p export results;
15. repeated 1080p export results;
16. qualification-ceiling matrix;
17. reference-host budget matrix;
18. CM-01..CM-07 evidence;
19. package build command;
20. installer artifact path;
21. installer/FFmpeg/ffprobe SHA-256 values;
22. durable evidence-file paths;
23. standard quality-gate results;
24. regression results;
25. acceptance matrix `P17-AC-01..78`;
26. deviations from plan;
27. unresolved issues;
28. deferred/external gates;
29. technical debt;
30. architecture/ADR assessment;
31. proposed completion classification.

The report must distinguish:

```text
implemented
measured
qualified
deferred
blocked
```

Do not use those terms interchangeably.

---

## 24. Implementation Prompt Boundary

After the planning amendment checkpoint commit, implementation proceeds against this plan and its acceptance criteria.

Codex may choose the smallest reasonable internal helper/module organization, but it may not:

- redefine acceptance criteria;
- renumber acceptance criteria;
- broaden product scope;
- replace the accepted research budgets;
- replace CM-01..CM-07;
- reinterpret P16-AC-56;
- treat True Clean VM as completed;
- introduce optimization without the stop/remediation rule.

---

## 25. Transport Performance Contract Amendment — 2026-10-07

### Decision

Ranked options: **C > B > A**. Choose **Option C**: Fixture B is the sole normative workload for Phase-17 transport qualification. W0 and W1 remain measured, reported, and useful as informational longitudinal baselines, but they do not define separate acceptance budgets in this phase.

The Phase-17 harness is explicitly a scale qualification against the accepted 20-track/200-item Fixture B. The previous JSONL and MCP reference-host figures were not consistently calibrated on that workload. Making W0/W1/W2 all normative would add three acceptance surfaces without serving the Phase-17 scale gate; requiring W2 to satisfy W0-scale latency would conflate workload cost with regression.

### Preserved original criteria and amended budgets

| Criterion | Original reference-host budget | Amended normative workload and budget | Qualification ceiling |
|---|---:|---:|---:|
| P17-AC-31 JSONL inspect/dry-run/in-memory apply | < 100 µs | Fixture B median < 150 µs | < 10 ms |
| P17-AC-34 MCP inspect/dry-run | < 1.0 ms | Fixture B median < 5.0 ms | < 20 ms |
| P17-AC-37 MCP persistent apply | < 20 ms | Fixture B median < 20 ms (unchanged) | < 100 ms |

The original thresholds and their historical context are retained here rather than erased. Qualification ceilings are unchanged. W0/W1 and all P95/aggregate values are reported as evidence; only the listed median budgets gate the accepted reference host.

### Evidence and historical provenance

The release binary was `tinixcut.exe` built from Git `92a225899686e0440536b59f30f9fb16d3156511` (SHA-256 and environment/method metadata are in `tests/evidence/phase17_transport_recalibration_2026-10-07.json`). Each transport used 5 warmups; JSONL used N=50 and MCP N=30. The harness ran real Windows OS stdio, excluded process startup and MCP discovery/handshake, correlated every response ID, and rejected error responses. Reported sample-average latency is the arithmetic mean of individually timed round trips; timer includes stdin write/flush and response readline but excludes client-side JSON request construction and response parsing. Wall-clock throughput is separately labeled and is not per-call latency.

| Workload | JSONL inspect median / P95 / avg (ms) | JSONL dry-run median / P95 / avg (ms) | JSONL apply `saveAfter:false` median / P95 / avg (ms) | MCP inspect median / P95 / avg (ms) | MCP dry-run median / P95 / avg (ms) | MCP apply median / P95 / avg (ms) |
|---|---:|---:|---:|---:|---:|---:|
| W0: 1 track / 0 items | 0.0436 / 0.0849 / 0.0515 | 0.0642 / 0.0957 / 0.0655 | 0.0525 / 0.0891 / 0.0573 | 0.2407 / 0.3396 / 0.2500 | 0.2217 / 0.3276 / 0.2270 | 9.703 / 13.736 / 9.633 |
| W1: Fixture A, 3 tracks / 4 items | 0.0577 / 0.1069 / 0.0636 | 0.0673 / 0.0851 / 0.0681 | 0.0758 / 0.0987 / 0.0752 | 0.3231 / 0.4210 / 0.3323 | 0.3368 / 0.4195 / 0.3555 | 10.110 / 14.464 / 10.369 |
| W2: Fixture B, 20 tracks / 200 items | 0.0800 / 0.1452 / 0.0971 | 0.1098 / 0.1449 / 0.1141 | 0.1034 / 0.1369 / 0.1105 | 3.4117 / 3.8771 / 3.4954 | 3.5426 / 4.9322 / 3.6843 | 16.760 / 24.496 / 17.274 |

The W2 results meet the amended JSONL and MCP inspect/dry-run medians; persistent apply remains within its unchanged <20 ms reference-host median budget and <100 ms ceiling. Its observed P95 exceeded 20 ms in this run, which is not the normative median criterion; the <100 ms qualification ceiling remains the tail guardrail.

Supporting direct W2 measurements from the earlier release qualification run were: `load_project` median 2.985 ms/P95 3.951 ms and `save_project_atomic` median 3.890 ms/P95 5.912 ms. A separate direct CommandService run measured dry-run median 0.050 ms/P95 0.069 ms/avg 0.058 ms and apply median 0.075 ms/P95 0.107 ms/avg 0.079 ms. A separate 55-load aggregate was 178.199 ms (3.240 ms/load average, including warmups). A separate release-profile direct run measured 50 atomic saves at median 4.167 ms/P95 6.888 ms/avg 4.547 ms. These are supporting measurements from distinct runs and are not substituted for the real-transport benchmark.

Historical workload classification is evidence-based:

- The historical JSONL medians 23/18/19 µs are classified **W2 (documented)** because the durable research table explicitly identifies Fixture B / 200 items. Original raw sample logs were not retained, so the table's stated classification is known but its underlying raw provenance cannot be independently reconstructed.
- The historical MCP inspect/dry-run medians 0.2381/0.2422 ms are **unknown / conflicting documentation**: the original `RESEARCH.md` table labels the section Fixture B, while the later release-qualification report states that approximately 0.238 ms was measured on W0 (1 track / 0 items). Without the original per-sample artifact, neither label can be established as the actual provenance.
- The historical MCP apply median 12.2009 ms is **W2 (documented)** in the original research section describing full reload, mutation, and atomic save for Fixture B. Its raw per-sample artifact was not retained.
- The earlier ~0.06 ms MCP apply result remains **invalid/error-path timing**, excluded by P17-AC-38.

### Contract review and checkpoint

The amended budgets preserve the existing qualification ceilings, do not loosen MCP persistent-apply median, and distinguish scale cost from regressions using normative Fixture B. W0/W1 remain visible for diagnosis. No architecture or product-semantic choice is introduced; ADR-006 remains unnecessary. This amendment is **SELF-REVIEWED AND LOCKED FOR IMPLEMENTATION**. The dedicated planning-amendment commit is the implementation checkpoint; after it, continue Phase 17 without a separate approval gate.
