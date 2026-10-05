# Phase 12 Plan — FFmpeg Distribution, Packaging, and Release Qualification

**Project:** TinixCut / Automation-First Desktop Video Editor  
**Phase:** 12 — FFmpeg Distribution, Packaging, and Release Qualification  
**Plan status:** READY FOR CODEX IMPLEMENTATION  
**Research gate:** COMPLETE — READY FOR PLANNING  
**Architecture authority:** `ADR-002` for Phase 12 distribution/packaging decisions  
**Gate:** Gate F / POC 5 — **PENDING EXECUTION**  
**Primary target:** personal-use Windows build  

---

## 1. Objective

Turn the current development application into a reproducible, installable Windows release candidate whose media behavior is independent of the developer machine.

Phase 12 must prove all of the following together:

1. the packaged application owns and uses the pinned FFmpeg/ffprobe binaries required by the release;
2. packaged execution cannot silently depend on system `PATH`;
3. all media operations resolve binaries through one centralized `MediaBinaryResolver` policy;
4. the hardware encode ladder behaves deterministically as `NVENC -> QSV -> AMF -> libx264`;
5. mandatory workflows continue to work when hardware acceleration is absent;
6. the NSIS installer correctly handles WebView2 through `downloadBootstrapper`;
7. a clean Windows machine can install, launch, import/probe media, generate derivatives, and export without a development toolchain or preinstalled FFmpeg;
8. release packaging preserves the reliability, security, preview/export parity, persistence, and automation guarantees established in earlier phases;
9. Gate F / POC 5 is executed with durable evidence rather than inferred from unit tests.

This phase is release qualification, not a feature-expansion phase.

---

## 2. Authoritative Inputs and Precedence

Codex must read and reconcile these sources before implementation:

1. `ADR-002` — authoritative Phase 12 architecture;
2. `ARCHITECTURE.md` — durable system invariants;
3. `RESEARCH.md` — POC 5 research conclusions and limitations;
4. `PLAN.md` — Phase 12 roadmap scope and release gate;
5. `PROJECT_STATUS.md` — current accepted implementation state;
6. `DEVELOPMENT_WORKFLOW.md` and `AGENTS.md` — actual repository validation and agent rules;
7. the current repository implementation.

If an older document conflicts with `ADR-002` on Phase 12 packaging/distribution behavior, `ADR-002` wins. Do not silently rewrite architecture around implementation convenience.

### 2.1 ADR-002 decisions that this plan must preserve

The implementation must preserve these approved decisions exactly:

- release scope is a **personal-use Windows build**;
- pinned media distribution is **FFmpeg 9.0.2 Gyan Release Essentials**;
- FFmpeg/ffprobe are packaged through Tauri **`bundle.resources`**;
- binary discovery is centralized behind **`MediaBinaryResolver`**;
- a packaged build must use **app-owned pinned binaries**;
- system `PATH` is permitted only through an **explicit dev/test mode**;
- Windows packaging uses **NSIS**;
- WebView2 install mode is **`downloadBootstrapper`**;
- encoder preference ladder is **NVENC -> QSV -> AMF -> libx264**;
- Gate F / POC 5 remains **PENDING EXECUTION** until this phase performs the real qualification.

---

## 3. Existing-State Assumptions to Verify, Not Re-Implement

The accepted project state says Phases 0–11 are complete and healthy. Phase 12 must treat these as regression constraints rather than redesign targets:

- Tauri + React/TypeScript + Rust modular-monolith application exists;
- media services already use controlled FFmpeg/ffprobe process boundaries;
- export already has snapshot isolation, progress/cancellation, temporary output, and safe final promotion;
- preview/export parity is already gated and passing for the supported semantic subset;
- automation inspect/diff/apply/render uses the same domain/JobManager paths;
- reliability/security/performance hardening from Phase 11 is accepted;
- existing hardware/software fallback logic may exist, but Gate F has not yet qualified packaged binaries, installer behavior, clean-machine execution, or the complete fallback matrix.

Codex must inspect the actual repository first and map these assumptions to concrete modules, tests, commands, and configuration files. Do not create parallel replacements if equivalent infrastructure already exists.

---

## 4. In Scope

Phase 12 includes:

- repository preflight and Phase 12 implementation mapping;
- pinned FFmpeg/ffprobe resource staging and integrity inventory;
- centralized `MediaBinaryResolver` implementation/refactor as required by ADR-002;
- migration of every production FFmpeg/ffprobe caller to the resolver;
- packaged-vs-dev/test binary-resolution policy;
- encoder capability selection and hardware/software fallback hardening;
- deterministic POC 5 fixture harness and evidence capture;
- Tauri `bundle.resources` configuration;
- NSIS installer configuration;
- WebView2 `downloadBootstrapper` verification;
- installer/package content verification;
- clean-install, upgrade, uninstall, and prerequisite-failure qualification;
- clean-machine runtime qualification on retained Windows targets;
- release regression suite;
- third-party/license/build inventory required for this personal-use package;
- release evidence and project-state documentation updates after successful implementation.

---

## 5. Out of Scope / Do Not Add

Do **not** expand Phase 12 into:

- new timeline/editor features;
- new transitions/effects;
- H.265/HEVC or AV1 launch support;
- HDR or advanced color management;
- new 60 fps product commitment;
- guaranteed real-time 4K multi-layer playback;
- macOS/Linux packaging;
- ARM64 Windows qualification unless already an approved target;
- MSI/MSIX as a second installer path;
- public auto-update infrastructure;
- code signing/certificate procurement or SmartScreen reputation work unless already provisioned and explicitly required;
- public/commercial codec patent or legal sign-off;
- bundled/offline WebView2 runtime strategy that contradicts ADR-002 `downloadBootstrapper`;
- downloading arbitrary/latest FFmpeg at runtime;
- system-`PATH` fallback in packaged production;
- direct FFmpeg path construction in UI/automation/export/media modules outside `MediaBinaryResolver`;
- broad refactors unrelated to release qualification.

Any useful but nonessential improvement discovered during this phase must be recorded under Deferred Items rather than silently added.

---

## 6. Implementation Workstreams

### 12.0 — Repository Preflight and Implementation Map

Before changing code:

1. inspect the current FFmpeg/ffprobe invocation sites;
2. identify current media/export/automation modules that spawn binaries;
3. inspect existing hardware fallback logic and error classification;
4. inspect current Tauri config, resource directories, package scripts, installer configuration, and application versioning;
5. identify the current deterministic media fixture corpus and parity fixtures;
6. identify the real commands for test, typecheck, lint, build, Tauri package build, and release execution from repository scripts/docs;
7. identify where release artifacts are emitted;
8. record any discrepancy between actual code and `PLAN.md` / `PROJECT_STATUS.md` / `ADR-002` before implementation.

**Constraint:** do not rename/reorganize broad module trees merely to make the Phase 12 diff aesthetically cleaner.

**Deliverable:** a short implementation map in the Codex report showing existing modules reused and new/changed modules introduced.

---

### 12.1 — Pinned FFmpeg/ffprobe Resource Baseline

Package the exact approved binary distribution:

- FFmpeg version: **9.0.2**;
- distributor/build: **Gyan Release Essentials**;
- binaries required: `ffmpeg.exe` and `ffprobe.exe` plus any distribution files that are actually required for those binaries to run;
- source archive/build identity must be recorded;
- SHA-256 must be recorded for each packaged executable and any required redistributable binary/resource whose integrity matters;
- expected `ffmpeg -version` and `ffprobe -version` identity must be captured in the release inventory.

Use a repository-owned staging/resource layout appropriate to Tauri packaging. Do not make developer-local absolute paths part of configuration.

Tauri packaging must include these assets through **`bundle.resources`**.

#### Required resource inventory

Create or update a durable release inventory containing at least:

- upstream/distributor name;
- exact version;
- build flavor (`Release Essentials`);
- download/source reference used by the project;
- SHA-256 values;
- included executables/resources;
- FFmpeg license/build-configuration output needed for inventory;
- date qualified;
- Phase 12 qualification result.

Do not claim legal/commercial clearance. The inventory is technical provenance; legal review remains separate where applicable.

---

### 12.2 — Centralized `MediaBinaryResolver`

Implement or consolidate one application-owned resolver responsible for locating FFmpeg and ffprobe.

#### Required resolver contract

The resolver must distinguish at least:

- binary kind: FFmpeg vs ffprobe;
- runtime mode: packaged production vs explicit dev/test system-binary mode;
- resolved origin: app-owned resource vs explicitly permitted system path;
- resolution failure reason.

#### Packaged production policy

In a packaged build:

- resolve FFmpeg/ffprobe only from the application-owned packaged resource location;
- do not search system `PATH`;
- do not use developer environment variables as an implicit fallback;
- do not download replacement binaries at runtime;
- missing/unreadable packaged binaries must fail explicitly;
- a fake or older `ffmpeg.exe` earlier on `PATH` must not influence the chosen executable.

#### Explicit dev/test policy

System `PATH` may be used only when an explicit dev/test mode is deliberately selected.

The exact flag/environment/config name may be chosen to fit repository conventions, but it must be:

- disabled by default for packaged release;
- obvious in diagnostics;
- covered by tests;
- documented in `DEVELOPMENT_WORKFLOW.md`;
- impossible to activate accidentally merely because a system FFmpeg exists.

#### Caller migration

Every production process-spawn path for FFmpeg/ffprobe must consume the resolver result rather than reimplement discovery. This includes, as applicable to the actual repo:

- probe/import metadata;
- thumbnails/filmstrips;
- proxies;
- waveform/audio extraction;
- export/render;
- automation-triggered render/export;
- diagnostic/version checks.

A repository search at phase completion must show no independent production `ffmpeg`/`ffprobe` path lookup or raw command-name spawning outside the approved resolver/test fixtures.

---

### 12.3 — Binary Validation and Failure Semantics

At runtime or through a release diagnostic path, validate enough binary identity to distinguish a valid packaged baseline from a missing/mispackaged resource.

Required behavior:

- packaged binary missing -> explicit structured media-binary resolution/startup failure;
- packaged binary unreadable -> explicit failure;
- packaged binary cannot execute -> explicit failure;
- system FFmpeg present while packaged binary is broken -> **must not silently fall back to system PATH**;
- process crash/abnormal exit -> existing job failure semantics apply; project state remains untouched;
- failure details must identify the attempted binary origin/path category without exposing unrestricted shell capability to the frontend.

A hash mismatch check may be performed by build/package verification tooling rather than on every application launch if runtime hashing would add unnecessary cost. However, the shipped artifact must be verified against the recorded SHA-256 inventory before release qualification passes.

---

### 12.4 — Hardware Encoder Ladder and Software Fallback

Preserve the ADR-002 encoder order exactly:

```text
NVENC -> QSV -> AMF -> libx264
```

The selection layer must distinguish:

1. encoder not compiled/present in the pinned FFmpeg build;
2. encoder present but required hardware/device is unavailable;
3. hardware encoder initialization failure before useful render progress;
4. non-hardware failures such as invalid filtergraph, missing input, permission error, low disk, cancellation, or unrelated process failure.

#### Fallback rule

Fallback to the next encoder is permitted only for errors classified as encoder/capability/hardware-initialization unavailability.

Do **not** hide unrelated render errors by repeatedly changing encoders.

#### Mid-render failure

If a hardware encoder starts successfully and later fails after meaningful render progress, Phase 12 must default to the existing safe export failure behavior unless the current backend already has a proven restart-from-zero mechanism. Do not add a large transparent restart engine merely for this phase.

In all cases:

- temporary/incomplete output is not promoted;
- an existing valid destination is preserved;
- the export job reaches a clear terminal state;
- project state is unchanged;
- cancellation remains cancellation, not a trigger for fallback;
- logs/evidence identify attempted encoders and the final selected encoder.

#### Mandatory software path

`libx264` is the mandatory final fallback for required H.264 export. A machine with no usable NVENC/QSV/AMF path must still export the required H.264/AAC MP4 profile through the packaged FFmpeg build.

---

### 12.5 — Phase 12 Fixture Harness

Build a deterministic Phase 12 qualification harness around the existing media fixture corpus. Reuse existing fixtures; add only missing cases.

The harness must be runnable without requiring the editor UI to be manually driven for every assertion.

#### Minimum fixture set

Use existing equivalents where they already exist:

- **P12-F01 — CFR 1080p30 H.264/AAC:** baseline probe, thumbnail, proxy, and export input;
- **P12-F02 — 4K H.264 source:** proxy/downscale and probe path;
- **P12-F03 — VFR H.264/AAC:** probe/timing robustness;
- **P12-F04 — 48 kHz stereo audio:** waveform/audio extraction path;
- **P12-F05 — rotated-metadata media:** probe/orientation metadata path;
- **P12-F06 — corrupt/truncated media:** safe negative process failure;
- **P12-F07 — path stress case:** copy/use media from a path containing spaces and non-ASCII Unicode characters;
- **P12-F08 — representative `.tcproj`:** supported production semantics only, including representative video/image/audio, transform/crop/opacity, bundled deterministic text/font, supported fade/transition behavior, and supported keyframe/audio behavior. Do not include deferred `CrossDissolve` or other unsupported features.

#### Harness outputs

Each qualification run must emit a machine-readable result artifact, preferably JSON, containing at least:

- application/package version;
- OS version/build;
- machine architecture;
- GPU adapter(s) and driver information obtainable without adding broad new dependencies;
- FFmpeg and ffprobe resolved origin;
- executable version output;
- recorded executable hash or package-verification hash reference;
- operation executed;
- encoder candidates attempted;
- selected encoder;
- exit/result status;
- relevant structured error category;
- output file path/name for test artifact;
- output probe summary: container, video codec, audio codec, resolution, frame-rate metadata, duration;
- timestamp and test case identifier.

Do not make test evidence dependent on screenshots alone.

---

### 12.6 — Gate F / POC 5 Execution Matrix

POC 5 answers:

> Can the pinned FFmpeg/ffprobe distribution be packaged and executed reliably on the target Windows release environment, including deterministic hardware acceleration fallback and mandatory software fallback?

POC 5 must execute against the actual packaged/installed release candidate for package-sensitive cases. Dev-mode success is insufficient.

#### 12.6.1 Functional matrix

| ID | Environment / condition | Required operation | Expected result |
|---|---|---|---|
| P5-M01 | Installed package, no system FFmpeg required | resolve `ffmpeg` + `ffprobe`, capture versions | app-owned 9.0.2 Gyan Essentials binaries resolved |
| P5-M02 | Installed package | probe P12-F01 | success; metadata sane |
| P5-M03 | Installed package | thumbnail P12-F01 | success; artifact produced |
| P5-M04 | Installed package | proxy P12-F02 | success; artifact produced and probeable |
| P5-M05 | Installed package | waveform/audio extraction P12-F04 | success; artifact/data produced |
| P5-M06 | Installed package | probe P12-F03/P12-F05 | success; timing/orientation path remains valid |
| P5-M07 | Installed package | process P12-F06 | explicit safe failure; no crash/corruption |
| P5-M08 | Installed package from Unicode/space media path | probe + one derivative + export | success; no quoting/path corruption |
| P5-M09 | Supported hardware encoder available | export P12-F08 using automatic ladder | first usable candidate selected; valid H.264/AAC MP4 |
| P5-M10 | No supported hardware encoder available | export P12-F08 | `libx264` selected; valid H.264/AAC MP4 |
| P5-M11 | Earlier ladder candidate unavailable/initialization rejected | export via testable selection scenario | next candidate attempted in exact ADR order |
| P5-M12 | All hardware candidates unavailable | export | `libx264` succeeds; no hardware requirement leaks into project/output semantics |
| P5-M13 | Packaged binary missing/renamed after install | probe/export attempt | explicit binary error; **no PATH fallback** |
| P5-M14 | Fake/older `ffmpeg.exe` prepended to system PATH | packaged probe/export | fake binary is ignored; app-owned binary still used |
| P5-M15 | FFmpeg child killed/fails during export | export | job fails safely; incomplete output not promoted; project unchanged |
| P5-M16 | Export cancelled | export | terminal `Cancelled`; no fallback retry; incomplete output cleaned/not promoted |

#### 12.6.2 Encoder branch qualification

Automated tests must exercise ordering/error-classification branches for all four candidates:

- NVENC;
- QSV;
- AMF;
- libx264.

Real-hardware qualification is recorded separately:

- a hardware path may be marked **QUALIFIED** only when an actual encode succeeds on real hardware using the installed/package candidate;
- if NVENC/QSV/AMF hardware is not present on available qualification machines, record **NOT PRESENT — BRANCH TESTED, HARDWARE NOT QUALIFIED**;
- absence of a vendor GPU is not itself a failure for this **personal-use** release, provided the actual intended personal target machine(s) and mandatory `libx264` fallback are qualified;
- do not advertise an untested vendor path as release-qualified support;
- if project scope later changes to general/public Windows support, real-hardware qualification for each claimed vendor path becomes a separate release requirement.

#### 12.6.3 Gate F pass conditions

Gate F passes only if:

1. the installed application uses the pinned app-owned binaries without system FFmpeg;
2. packaged FFmpeg/ffprobe perform required probe/derivative/export operations;
3. the actual target machine's available hardware path, if any, succeeds or cleanly falls through;
4. software fallback with `libx264` succeeds on a machine/environment with no usable hardware encoder;
5. failure classification prevents unrelated render errors from being hidden by fallback;
6. binary-missing and PATH-poisoning negative controls behave correctly;
7. package/license/build inventory is complete;
8. no release-blocking regression is found by the Phase 12 regression suite.

A unit test proving `libx264` is the last enum/candidate is not sufficient to pass Gate F.

---

### 12.7 — Tauri Resource Packaging and Installer Configuration

Configure the Windows release bundle according to ADR-002:

- Tauri `bundle.resources` contains the pinned FFmpeg/ffprobe resource set;
- Windows installer target is NSIS;
- WebView2 install mode is `downloadBootstrapper`;
- production package contains only the intended runtime assets;
- installer/build configuration uses repository-relative deterministic paths;
- bundled deterministic fonts/assets required by existing render parity remain included.

Do not add a second MSI/MSIX packaging route in this phase.

#### Package inspection checklist

For each release candidate artifact:

1. record installer filename, application version, size, and SHA-256;
2. install to the expected per-user/system location according to existing Tauri/NSIS policy;
3. verify the installed app contains the FFmpeg/ffprobe resources in the expected app-owned resource location;
4. verify installed binary hashes match the release inventory;
5. verify app launch resolves those exact installed resources;
6. verify no repo checkout, Node.js, Rust toolchain, package manager, or developer-local FFmpeg is required at runtime;
7. verify bundled fonts/assets used by supported rendering remain available;
8. verify no obvious dev-only fixture corpus, secrets, `.env` files, debug dumps, or unrelated build outputs are accidentally shipped.

---

### 12.8 — Clean-Machine Qualification

A development workstation does not qualify the installer.

Use clean Windows VMs or clean physical machines that do not have the TinixCut repo/toolchain installed.

#### Baseline clean-machine conditions

Unless a test row explicitly says otherwise:

- no TinixCut prior install;
- no repository checkout;
- no Node.js requirement;
- no Rust/Cargo requirement;
- no system FFmpeg requirement;
- no FFmpeg directory intentionally added to `PATH`;
- normal non-developer user environment;
- media fixtures transferred separately as test data;
- installer under test is the exact candidate whose hash is recorded.

#### Qualification matrix

| ID | Windows state | WebView2 state | GPU state | Network | Purpose |
|---|---|---|---|---|---|
| CM-01 | Windows 11 x64 clean | absent if practical | intended personal target hardware | available | clean install + `downloadBootstrapper` + normal packaged workflow |
| CM-02 | Windows 11 x64 clean | present | no supported vendor GPU / VM software-only path | available | mandatory `libx264` fallback |
| CM-03 | Windows 10 x64 clean, if Windows 10 remains a claimed target | absent or clean baseline | any | available | retained Windows 10 install/runtime support |
| CM-04 | clean Windows | absent | any | **unavailable** | negative WebView2 bootstrap failure behavior |
| CM-05 | prior candidate installed | present/normal | any | available | upgrade rehearsal and user-data preservation |

If Windows 10 cannot be practically qualified, Codex must not silently mark the phase fully release-qualified while `PLAN.md`/architecture still claims Windows 10/11 support. Report the gap for an explicit completion decision or scope/documentation change.

#### Clean-machine smoke flow

On each applicable positive machine:

1. install from the NSIS candidate;
2. launch the app normally;
3. create/open a project;
4. import/probe P12-F01;
5. generate at least one thumbnail;
6. generate a proxy from P12-F02;
7. exercise waveform/audio extraction from P12-F04 if exposed by current workflow;
8. open the representative P12-F08 project or construct the equivalent through normal supported paths;
9. export H.264/AAC MP4;
10. probe the output and record codec/resolution/duration;
11. close/reopen the app/project;
12. verify project semantics remain intact;
13. run the automation render smoke path if automation is included in the packaged executable/release surface;
14. collect the POC 5 evidence artifact.

---

### 12.9 — Install / Upgrade / Uninstall / Rollback and Failure Behavior

#### Clean install

Must verify:

- installer completes on supported clean machine with network available when WebView2 must be bootstrapped;
- app launches from installed location;
- installed runtime does not rely on developer tools;
- app-owned media binaries are used.

#### WebView2 download failure

Test the `downloadBootstrapper` prerequisite path with WebView2 absent and network unavailable or blocked.

Expected behavior:

- installation/prerequisite failure is visible and actionable;
- installer must not report a misleading successful ready-to-run state when the required WebView2 runtime was not obtained;
- any partial-install residue must be documented and cleanly removable;
- no project/source media is touched by the failed install.

Do not change ADR-002 to an offline/bundled WebView2 strategy merely to make this negative test disappear.

#### Upgrade rehearsal

Perform at least one installer upgrade rehearsal using two package versions/candidates produced through the same packaging path.

Verify:

- upgrade replaces application-owned executable/resources with the new candidate;
- FFmpeg/ffprobe hashes after upgrade match the new pinned inventory;
- user project files remain readable and untouched;
- external source media is untouched;
- cache/recovery directories are not treated as canonical project data;
- existing project save/reopen and export still pass after upgrade.

If no historical Phase 12 installer exists, it is acceptable to create two controlled packaging candidates solely for the upgrade rehearsal; do not fabricate a product feature migration to create work.

#### Uninstall

Verify the uninstaller:

- removes application-installed files as expected;
- does **not** delete user `.tcproj` files in arbitrary user-selected locations;
- does **not** delete external source media or exports;
- does not traverse/delete unrelated user directories;
- leaves any user-local cache/recovery behavior exactly as documented by existing policy unless explicit cleanup was already part of the product design.

Do not add risky recursive user-data deletion logic merely for a “clean uninstall.”

#### Runtime rollback/failure safety

For packaged media failures:

- binary resolution failure -> operation fails explicitly, no system-PATH rescue;
- hardware unavailability -> proceed to next approved encoder;
- all hardware unavailable -> `libx264`;
- invalid input/filter/output permission/low disk/cancellation -> do not masquerade as hardware fallback;
- sidecar/export crash -> no project corruption;
- failed export -> no misleading final output;
- existing valid output remains preserved according to the Phase 8 safe-promotion contract.

---

### 12.10 — Release Regression Suite

Phase 12 must rerun the complete risk-relevant suite after packaging changes.

#### Mandatory automated validation

Run the repository's canonical commands for:

- Rust tests;
- frontend/Vitest tests;
- TypeScript typecheck;
- Rust formatting/lint/Clippy according to repository policy;
- frontend production build;
- Rust/Tauri release build;
- package/installer build;
- new resolver tests;
- new resource/package verification tests;
- new encoder selection/fallback tests;
- new POC 5 fixture-harness tests.

Do not hardcode an old expected test count; all tests present at the Phase 12 commit must pass.

#### Mandatory integration/regression coverage

At minimum rerun or verify:

- project save/reopen;
- atomic save/recovery failure cases;
- media offline/relink behavior;
- cache deletion/regeneration;
- corrupt media safe failure;
- JobManager cancellation/backpressure tests affected by sidecar changes;
- preview/export parity suite;
- deterministic export fixtures;
- automation inspect/dry-run/apply/save-reopen/idempotency/stale-revision/export E2E;
- Phase 11 security tests for shell/path injection boundaries;
- Phase 11 long-project/performance checks if resolver/package changes touch hot paths;
- packaged export cancellation and sidecar crash behavior;
- clean-machine POC 5 qualification.

#### Security regression requirements

Packaging work must not introduce:

- unrestricted shell capability to WebView/UI;
- arbitrary executable selection from project/user input;
- raw shell-string concatenation;
- environment-controlled production binary substitution;
- project-file fields that can redefine executable paths;
- automation ability to select arbitrary native executables.

---

### 12.11 — Release Inventory, Evidence, and Documentation

Create/update durable documentation for the Phase 12 candidate:

- exact app version;
- exact Tauri/Rust/Node/package-manager policy actually used by the repo;
- FFmpeg/ffprobe 9.0.2 Gyan Release Essentials provenance;
- binary SHA-256 values;
- FFmpeg build configuration/version output;
- bundled font/resource inventory relevant to deterministic rendering;
- NSIS artifact identity/hash;
- WebView2 mode (`downloadBootstrapper`);
- POC 5 matrix result;
- clean-machine matrix result;
- qualified hardware paths vs not-present/unqualified vendor paths;
- software fallback result;
- known packaging limitations;
- third-party notices/source-obligation inventory as applicable;
- deferred legal/commercial review statement.

Do not call an item “qualified” when it was only inferred or simulated.

---

## 7. Expected Files / Modules Affected

Codex must map these to actual repository paths during Workstream 12.0. Likely affected areas include:

- Tauri configuration (`tauri.conf.*` or equivalent);
- Windows/NSIS bundle configuration;
- media service/process-spawn module;
- export encoder selection/fallback module;
- a new or existing centralized `MediaBinaryResolver` module;
- automation render path only if it currently resolves binaries independently;
- release-resource staging directory for FFmpeg/ffprobe;
- media fixture/qualification harness;
- integration tests for packaged resource resolution;
- package verification scripts/tests;
- `DEVELOPMENT_WORKFLOW.md`;
- release/license inventory documentation;
- `PLAN.md` / `PROJECT_STATUS.md` only at the required implementation/reconciliation checkpoint.

Do not assume new filenames when an existing module already owns the responsibility.

---

## 8. Data / State / API Contracts

Phase 12 should avoid project-schema changes.

### 8.1 Project state

- `.tcproj` must not store FFmpeg/ffprobe executable paths;
- `.tcproj` must not store GPU encoder availability as durable project truth;
- projects created/exported on one GPU vendor must remain openable/exportable on another machine using the available ladder/fallback;
- cache remains derived and rebuildable.

### 8.2 Runtime media-binary state

Binary path/origin/version is runtime/release metadata, not canonical editorial state.

### 8.3 Encoder selection

The selected encoder is an execution detail of an export job. It may be reported in diagnostics/evidence but must not alter canonical project semantics.

### 8.4 Errors

Prefer existing structured error categories. Add a new stable category only if the current model cannot distinguish a release-critical case such as packaged media binary missing/unavailable.

Do not leak raw unrestricted process control into frontend APIs merely to expose diagnostics.

---

## 9. Exact Acceptance Criteria

Phase 12 may be classified `COMPLETE` only when every mandatory criterion below is satisfied or an explicit accepted scope change removes the criterion.

### Binary distribution and resolver

- **P12-AC-01 — Source reconciliation:** Codex documents the actual repo modules/configuration used for Phase 12 and reports any discrepancy with `ADR-002`, `PLAN.md`, `ARCHITECTURE.md`, or `PROJECT_STATUS.md` before claiming completion.
- **P12-AC-02 — Exact binary baseline:** the release resource set is FFmpeg/ffprobe **9.0.2 Gyan Release Essentials**, with provenance and SHA-256 inventory recorded.
- **P12-AC-03 — Tauri resource bundling:** the NSIS-installed application contains the required pinned binaries as app-owned Tauri resources included through `bundle.resources`.
- **P12-AC-04 — Central resolver:** all production FFmpeg/ffprobe callers route through `MediaBinaryResolver` (or the existing module renamed/extended to fulfill exactly that role); no independent production PATH lookup remains.
- **P12-AC-05 — Packaged PATH isolation:** with a fake/older FFmpeg first on system `PATH`, packaged TinixCut still resolves and executes the app-owned pinned binary.
- **P12-AC-06 — Explicit dev/test escape hatch only:** system PATH use is possible only through an explicit documented dev/test mode and is disabled in normal packaged release behavior.
- **P12-AC-07 — No silent rescue:** when the packaged FFmpeg/ffprobe resource is removed/broken, the media operation fails explicitly and does not use a system FFmpeg fallback.
- **P12-AC-08 — Binary identity evidence:** qualification evidence records resolved origin plus version output for both FFmpeg and ffprobe, and installed hashes match the pinned inventory.

### Hardware/software fallback

- **P12-AC-09 — Exact ladder order:** automated tests prove candidate order is `NVENC -> QSV -> AMF -> libx264`.
- **P12-AC-10 — Fallback classification:** only encoder/capability/hardware-initialization unavailability advances to the next encoder; unrelated render/input/output/cancellation errors do not.
- **P12-AC-11 — Mandatory software fallback:** an installed candidate on a machine/environment without usable NVENC/QSV/AMF successfully exports the required H.264/AAC MP4 workflow via `libx264`.
- **P12-AC-12 — Hardware evidence honesty:** every vendor hardware path labeled `QUALIFIED` has at least one successful real-hardware installed-package encode result; absent vendor hardware is recorded as not present/unqualified rather than assumed passing.
- **P12-AC-13 — Safe hardware failure:** a failed/unsupported hardware attempt never promotes a partial output, never mutates project state, and either advances according to the approved classification or terminates with a clear failure.

### POC 5 and fixture harness

- **P12-AC-14 — Fixture harness:** P12-F01 through P12-F08 or equivalent existing fixtures are covered by a deterministic harness with machine-readable result artifacts.
- **P12-AC-15 — POC 5 functional matrix:** P5-M01 through P5-M16 are executed or, where a row depends on unavailable vendor hardware, explicitly marked according to the branch-vs-real-hardware qualification rule; all mandatory rows pass.
- **P12-AC-16 — Gate F:** Gate F / POC 5 is marked PASS only after installed-package media operations, software fallback, binary-origin negative controls, package inventory, and regression requirements pass.

### Installer/package qualification

- **P12-AC-17 — NSIS artifact:** the canonical Windows release packaging path produces an installable NSIS artifact whose filename/version/hash are recorded.
- **P12-AC-18 — WebView2 bootstrap:** on a clean supported Windows machine without WebView2 and with network available, the NSIS `downloadBootstrapper` path installs/obtains WebView2 as needed and TinixCut launches successfully.
- **P12-AC-19 — Existing WebView2:** on a clean supported Windows machine with WebView2 already present, installation does not break or require unnecessary developer components and the app launches successfully.
- **P12-AC-20 — Bootstrap failure behavior:** with WebView2 absent and network unavailable/blocked, installation fails visibly/actionably rather than presenting a misleading ready-to-run success; any residue is removable and no user project/media is touched.
- **P12-AC-21 — Clean-machine independence:** on the clean-machine positive cases, TinixCut completes probe, derivative generation, and H.264/AAC export without Node, Rust, repo checkout, or system FFmpeg.
- **P12-AC-22 — Windows target coverage:** Windows 11 qualification passes; Windows 10 qualification also passes if Windows 10 remains a claimed target. If Windows 10 is not qualified, the phase cannot silently claim full Windows 10/11 release qualification.
- **P12-AC-23 — Upgrade rehearsal:** installing a newer controlled candidate over a prior candidate replaces app-owned resources while preserving user projects/external media and leaves save/reopen/export functional.
- **P12-AC-24 — Uninstall safety:** uninstall removes application-installed files without deleting arbitrary user projects, source media, or exports.
- **P12-AC-25 — Package-content audit:** the installed package contains required runtime resources and does not accidentally ship obvious secrets, `.env` files, unrelated debug artifacts, or developer-only fixture corpora.

### Regression and release evidence

- **P12-AC-26 — Full repository health:** canonical tests, typecheck, lint/Clippy, frontend build, Rust/Tauri release build, and installer build all pass at the Phase 12 candidate commit.
- **P12-AC-27 — Cross-phase regressions:** persistence/recovery/relink, cache independence, preview/export parity, safe export promotion/cancellation, automation E2E/idempotency/stale revision, security injection boundaries, and relevant Phase 11 hardening tests remain passing.
- **P12-AC-28 — Release inventory:** durable Phase 12 documentation records the package hash, app/toolchain versions/policies, FFmpeg provenance/hash/build identity, WebView2 strategy, POC 5 results, clean-machine results, qualified/unqualified GPU paths, known limitations, and applicable third-party notices/source obligations.
- **P12-AC-29 — No architecture bypass:** no project schema, UI, automation, or user-controlled field can choose an arbitrary executable or bypass `MediaBinaryResolver` to spawn FFmpeg/ffprobe.
- **P12-AC-30 — State reconciliation ready:** the Codex completion report contains enough evidence for ChatGPT to update `PLAN.md`/`PROJECT_STATUS.md`, record Gate F outcome, deferred items, known issues, and any architecture/ADR implications before the project is declared release-candidate complete.

---

## 10. Release-Blocking Failure Conditions

Phase 12 is `INCOMPLETE` or `BLOCKED` rather than complete if any of these remain:

- packaged application needs system FFmpeg to perform a mandatory workflow;
- packaged application can silently choose system FFmpeg when app-owned binaries are missing;
- pinned binary identity cannot be reproduced/verified;
- `libx264` software fallback cannot complete the required export on a no-hardware environment;
- fallback masks unrelated render errors;
- clean install cannot launch/perform the core media workflow on the claimed Windows target;
- WebView2 bootstrap behavior is unverified for the chosen NSIS strategy;
- failed export can overwrite/promote an invalid final output;
- packaged sidecar failure can corrupt project state;
- package introduces a new unrestricted shell/arbitrary-executable path;
- preview/export parity regresses for a production-supported feature;
- automation idempotency/revision guarantees regress;
- Gate F evidence is only unit-test or developer-machine evidence;
- an untested Windows/GPU configuration is claimed as qualified support.

---

## 11. Rollback Strategy for Implementation Changes

Phase 12 changes touch shared process-launch and packaging infrastructure. Keep changes decomposable.

Recommended commit/work-unit boundaries:

1. resolver contract/tests;
2. caller migration;
3. pinned resource staging/inventory;
4. Tauri resource bundle configuration;
5. encoder-selection/error-classification hardening;
6. POC 5 harness;
7. NSIS/WebView2 configuration;
8. package/clean-machine qualification scripts/docs;
9. final regression/documentation reconciliation.

If a workstream destabilizes production behavior:

- revert the smallest work unit rather than retaining a dual path;
- do not temporarily keep both legacy PATH discovery and the new production resolver active;
- do not bypass a failing resolver by hardcoding absolute executable paths;
- do not weaken fallback/error tests to make packaging pass;
- report an architecture contradiction if ADR-002 cannot be implemented cleanly in the actual repository.

---

## 12. Deferred Items

The following are intentionally outside Phase 12 unless separately approved:

- public/commercial distribution legal opinion, codec patent analysis, or formal license counsel;
- Windows code signing certificate procurement, signing pipeline, and SmartScreen reputation program;
- public auto-updater/update service;
- MSI/MSIX installer path;
- offline/bundled WebView2 runtime alternative;
- macOS/Linux packaging and release qualification;
- Windows ARM64 packaging;
- FFmpeg runtime auto-update or “latest” resolver;
- H.265/HEVC, AV1, HDR, advanced color pipeline;
- 60 fps launch guarantee;
- guaranteed 4K real-time multi-layer playback;
- exhaustive performance certification across all NVIDIA/Intel/AMD generations;
- vendor hardware paths that are not physically available for real-hardware qualification in the personal-use environment — keep branch coverage, mark hardware as unqualified;
- deferred Phase 9 adjacent-cut/overlap `CrossDissolve`;
- non-linear Bezier/custom easing keyframes;
- multi-channel/spectral waveform visualization;
- telemetry/crash-upload infrastructure;
- public release support/QA matrix beyond the approved personal-use scope.

Deferred items must not be accidentally represented as Phase 12 acceptance claims.

---

## 13. Execution Order

Codex should implement in this order:

```text
12.0 repo preflight
  -> 12.1 pinned resource baseline
  -> 12.2 MediaBinaryResolver
  -> 12.3 failure semantics
  -> 12.4 encoder ladder/fallback
  -> 12.5 fixture harness
  -> 12.7 Tauri resources + NSIS/WebView2 package
  -> package-content verification
  -> 12.6 Gate F / POC 5 installed-package matrix
  -> 12.8 clean-machine qualification
  -> 12.9 install/upgrade/uninstall/failure rehearsal
  -> 12.10 full regression suite
  -> 12.11 durable release evidence/docs
  -> Codex completion report
```

Do not mark Gate F passed before the installed-package and clean-machine stages.

---

## 14. Validation Evidence Required in the Codex Completion Report

The completion report must include, not merely state “passed”:

1. summary of implementation;
2. exact files/modules/configuration changed;
3. mapping from each workstream to implementation;
4. exact pinned FFmpeg/ffprobe provenance and SHA-256 values;
5. installer artifact filename/version/SHA-256;
6. evidence that installed app resolves app-owned binaries;
7. PATH-poisoning negative-control result;
8. missing-packaged-binary negative-control result;
9. encoder ladder branch test results;
10. real-hardware results, explicitly separated from simulated/branch tests;
11. mandatory `libx264` clean-machine fallback result;
12. P5-M01..P5-M16 result table;
13. CM-01..CM-05 result table with any not-applicable rows explained;
14. WebView2 `downloadBootstrapper` positive and negative results;
15. clean install result;
16. upgrade rehearsal result;
17. uninstall safety result;
18. package-content audit result;
19. full test/typecheck/lint/build/package command results;
20. preview/export parity regression result;
21. persistence/recovery/security/automation regression result;
22. known issues;
23. deferred items;
24. technical debt introduced, if any;
25. architecture implications / whether ADR-002 needs amendment;
26. explicit acceptance-criteria table for `P12-AC-01` through `P12-AC-30` with PASS/FAIL/BLOCKED and evidence;
27. proposed Gate F decision: PASS / FAIL / BLOCKED;
28. proposed phase classification: COMPLETE / COMPLETE WITH DEFERRED ITEMS / INCOMPLETE / BLOCKED.

Codex must not self-authorize a roadmap transition merely because the report recommends `COMPLETE`. ChatGPT review and project-state reconciliation remain mandatory.

---

## 15. Phase Completion and Project-State Reconciliation

After Codex reports completion, ChatGPT must review implementation evidence against this plan.

If accepted, reconcile durable project state before declaring the release candidate milestone complete:

- update Phase 12 status in `PLAN.md`;
- record Gate F / POC 5 outcome;
- update `PROJECT_STATUS.md` from Phase 11 complete / Phase 12 pending to the accepted Phase 12 state;
- record exact completed packaging capabilities;
- record qualified hardware paths and unqualified/not-present paths;
- record clean-machine coverage;
- record deferred items and technical debt;
- record known installer/runtime limitations;
- update `ARCHITECTURE.md` only if implementation exposed a real durable architecture change;
- update `ADR-002` only if the approved architecture itself must change, not for routine implementation notes;
- reconcile any disagreement between report, repository, plan, architecture, and POC evidence.

Only after this reconciliation can the project be considered to have reached the **Release Candidate milestone** described in `PLAN.md`.

---

## 16. Phase 12 Definition of Done

Phase 12 is done only when the release candidate is not merely buildable, but demonstrably installable and self-contained for the approved personal-use Windows scope:

- the exact pinned FFmpeg/ffprobe distribution is inside the installed app;
- production media execution is app-owned and independent of system PATH;
- the centralized resolver is the only production binary-resolution path;
- hardware acceleration is optional, deterministic, and safely falls back to `libx264`;
- POC 5 / Gate F passes with installed-package evidence;
- NSIS + WebView2 `downloadBootstrapper` behavior is qualified;
- clean-machine workflows pass;
- upgrade/uninstall/failure behavior is safe;
- all prior reliability/security/parity/automation guarantees remain green;
- release inventory and known limitations are durable and reproducible;
- project state is ready for final ChatGPT review and reconciliation.

