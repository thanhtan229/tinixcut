# DEVELOPMENT_WORKFLOW.md

> **Tinix Development, Validation, Reload, Restart & Packaging Standard**
>
> This document defines how a Tinix project should be developed, validated, reloaded, restarted, health-checked, and prepared for user testing.
>
> It is intentionally **framework-agnostic, runtime-agnostic, and platform-aware**. Project-specific commands belong in the **Project Runtime Profile** section of this file.

---

## 0. Purpose

The development workflow should optimize for:

- fast iteration;
- reliable verification;
- minimal disruption;
- clear runtime state;
- safe process handling;
- easy user testing;
- avoiding unnecessary builds and packaging.

The default development loop is:

`Understand → Change → Validate → Build when justified → Apply latest code → Verify runtime → Ready for user test`

The application should be left in a state where the latest relevant changes can be tested immediately.

---

## 1. Scope and Rule Strength

This file governs:

- development mode;
- validation;
- build decisions;
- frontend reload;
- backend/service restart;
- full application restart;
- process ownership;
- runtime health checks;
- development scripts;
- packaging behavior;
- release-related escalation.

Rule strength:

- **MUST / MUST NOT** — required unless a documented project-specific rule overrides it.
- **SHOULD / SHOULD NOT** — default behavior; deviate only for a concrete reason.
- **MAY** — optional based on the project.

This file does **not** define:

- product requirements;
- UI design language;
- architecture;
- coding style.

Use the appropriate project documents for those concerns.

---

## 2. Project Runtime Profile

This profile is initialized from `ARCHITECTURE.md` and `PLAN.md`.

Commands that do not exist yet MUST remain explicitly unresolved until Phase 0 creates the actual repository workflow. Agents MUST NOT invent commands merely to fill this document.

```text
Product name:
- Automation-First Desktop Video Editor (working title)

Project type:
- Desktop app
- Local native core
- Future local CLI/MCP adapters after the Command Service stabilizes

Primary platform:
- Windows 10/11

Frontend runtime:
- React + TypeScript in the Tauri WebView

Backend runtime:
- N/A as a standalone server in v1
- Application-core services run locally in Rust through controlled Tauri IPC/commands

Desktop/native runtime:
- Tauri 2.x + Rust

Media runtime:
- FFmpeg / ffprobe through a controlled abstraction/sidecar boundary

Database/runtime dependency:
- No server database required initially
- Canonical state: versioned project files
- Local derived cache/recovery storage
- If persistent DB storage is later introduced, update ARCHITECTURE.md first when it changes durable ownership

Development start command:
- pnpm tauri dev

Background caption-merge integration check (Windows, no desktop input):
- `pnpm run test:cdp` runs the real-WebView suite sequentially: caption/Source Viewer, recovery, saved recovery, pointer, controls, paired G51 audio, the 60-second workflow/export and production output PCM/gain qualification plus extended caption/transcript, relink/collect and history-branch qualification (eleven cases). Build the qualification executable first (command below). It does not rebuild automatically; reports record its SHA-256 and modification time, Git revision/dirty state and harness hashes. Keep `pnpm run test:background` for fast headless/native/MockRuntime regression checks.
- Select cases with `pnpm run test:cdp -- controls audio` (valid names: `caption-source`, `recovery`, `saved-recovery`, `pointer`, `controls`, `audio`, `workflow`, `gain-preview`, `captions`, `media`, `history-branch`). Each case gets a new isolated desktop/profile. The suite stops on functional/provenance failure, unsafe/unknown runtime state or a retention-budget failure. A cleanup-only failure may continue collecting evidence after the independent post-parent inspection proves safety; overall status remains FAILED. It retains logs/session receipts and copies fresh JSON evidence into `artifacts/background/cdp-runs/<run-id>/`; `cdp-latest-report.json` points to the latest run. PASS requires matching binary hashes, a successful parent session and verified cleanup; stale case JSON is rejected.
- The parent accepts one mode and `-TimeoutSeconds` (default 300 per stage). It launches Node without a visible console, drains stdout/stderr asynchronously, kills only the timed-out Node tree, then runs its normal app/profile cleanup. `-NonInteractive` and closed stdin prevent a background PowerShell prompt. Receipts include timeout/exit code, input desktop, app-window/listener checks and cleanup outcome. Startup/listener retries and cleanup retries are bounded. The expected-failure timeout check is recorded in `artifacts/background/cdp-timeout-check.json` (status `failed` and `timedOut: true` are intentional; cleanup must pass). Do not run two suites simultaneously on port 9444; the parent refuses an occupied port/qualification executable.
- `-Audio` generates two 90-second 48 kHz PCM tones, checks one/two sources, mute-all, multiple solo, −6/0/+6 dB, relative/summed meter response, command-backed fade-in/out at paused seek positions, paused-clock stability and seek/resume, then records 60 paired DOM/native samples over at least 60 seconds. Evidence: `artifacts/background/cdp-audio.json`. Clock error is media time minus DOM playhead/30 in the same DOM read; the native sample is requested at that captured frame and IPC duration is retained. No calibrated audible clock budget is asserted. The offline FFmpeg aligned-mix reference is separate from live captured elements. A 3-second native range export isolates Tone B at +6 dB, decodes it and compares its peak with a channel-matched FFmpeg reference (locked ±0.5 dB). `positiveGain` now compares the normalized native gain with the actual production GainNode through test-only Web Audio instrumentation installed before page load; it requires a +6 dB signal increase. Media-element volume stays unity by design. Paused fade checks compare the GainNode with sampled native gain. Export applies track gain without keyframes and sums sources with `amix normalize=0`; native regressions retain −6/+6 dB and coherent-sum checks.
- `gain-preview` / `-GainPreview` records the production `PreviewAudioOutput` graph PCM in the real qualification WebView, via a ScriptProcessor observer whose destination gain is zero. This test-only observer uses the browser initialization script mechanism; application CSP is unchanged. It compares 24,000 decoded channel samples from each one-second native −6/0/+6 dB export against FFmpeg PCM and the live graph (RMS ±0.5 dB, measured away from startup). It checks pause/mute silence, seek/resume, source reuse, replacement/disconnect and multiple-source/solo/mute. One audible edge per media source prevents a parallel native/Web Audio path by construction; speaker-level double-output/calibration is not measured. Evidence: `artifacts/background/cdp-gain-preview.json`. The runner pins the production output helper SHA-256.

- `captions` / `-Captions` exercises real UI multiline text edits, reorder into an empty time range, split/merge and Undo/Redo, native save/reopen with UI row readback, plus scoped SRT/WebVTT export/import and UI Undo. Native file pickers are not driven or stubbed. It copies the local JFK source from `artifacts/phase26-whisper/test-input-jfk.wav` into the scoped fixture, uses the real `whisper.cpp-v1.9.2-x64/Release/whisper-cli.exe` and `models/ggml-base.bin` under that directory, and records source/CLI/model SHA-256. These prerequisites must exist; no download or mocked transcript is substituted. It temporarily sets qualification-app transcription settings and restores them in finally, tests UI analysis cancel, real source transcript analysis/repeat cache identity, selected-range rough cut and history/persistence. The 180-second watchdog bounds the case. Evidence: `cdp-captions.json`; `settingsRestored` must be true. A missing runtime/model or failed analysis remains a failed case.
- `media` / `-Media` generates its own temporary WAV/image fixtures, moves only that owned WAV to create a missing-media state, checks real offline/online/source-preview UI, rejects wrong-kind/short/missing relink paths without revision/data changes, applies a scoped native relink and tests UI Undo/Redo. Scoped collect compares copied bytes and SHA-256 manifest, reopens the collected project and reads its online/source-preview UI; it checks the linked source-project file and source bytes remain unchanged. Evidence: `cdp-media.json`. Relink/collect commands bypass only the native picker entry point; picker/batch-cancel/full installed workflow acceptance remains OPEN. Poll the UI offline indicator after native revision changes; the offline refresh is a separate async request.
- Transcript regression: source analysis previously held the EditorEngine guard while attempting to lock it again for cache persistence, deadlocking uncached Whisper completion and subsequent IPC. The worker now releases its validation guard once the returned entries own their data. Keep the real `captions` analysis/repeat/rough-cut sequence in the CDP suite to catch this regression. Never mark a hung job as completed or stub its native report.
- Shutdown/continuation: Tauri window close alone can leave the embedded browser running and holding its profile. `cdp-close.mjs` checks the qualification title, closes the Tauri window, then sends the browser-level CDP `Browser.close` command and observes disconnection. Native `OpenProcess/GetExitCodeProcess` verifies actual app/WebView state; `Get-Process` alone was found insufficient for these exited/lingering processes. The parent retains a single owned-process fallback and three bounded in-process removal attempts; it does not retry force-kills on a file-lock error.
- Read-only `cdp-diagnostics.ps1` captures processes, CDP listeners, input desktop and Restart Manager owners of `lockfile`/`000003.log` before close, after app-exit wait, after browser-exit wait, before removal, and on cleanup failure. The runner independently captures the same state after the PowerShell parent exits. This latter inspection never deletes files or stops processes; the previously blocked after-parent deletion workflow remains unapplied. Inspect an existing receipt with `powershell -NoProfile -NonInteractive -File scripts/qualification/background/cdp-diagnostics.ps1 -SessionPath <receipt.json> -OutputPath <snapshot.json>`.
- The report exposes independent `functionalStatus`, `cleanupStatus` and per-case continuation reasons. A case requires fresh evidence, matching binary provenance and all successful isolated stages to qualify functionally. Before every case and after each parent, the runner requires exited app/test WebViews, no port 9444 listener, unchanged input desktop, no live/unknown lock owners, and a complete temporary-data inventory. Limits: 1 GiB across known `TinixCutCDP-<guid>` profiles/fixtures, fewer than 64 retained run IDs, with 128 MiB headroom for the next bounded fixture. Headroom is a preflight check, not an OS disk quota. Old blocked profiles are inventoried, never deleted by diagnostics.
- `node --test scripts/qualification/background/cdp-report-policy.node-test.mjs` verifies cleanup-only continuation retains FAILED status, live/unknown processes/locks/ports stop the suite, and stale evidence, changed input desktop, quota overflow and incomplete inspection cannot continue. The test is included in `test:background`. All three status fields must be read; functional PASS alone does not close a gate.
- Master meter now observes the production output graph after normalized gain/mute/solo, sharing the exact node that drives the AudioContext destination. Its extra observation output remains silent. Track peaks still aggregate active item peaks within a track; full stereo/transition track-bus metering, physical output and media-clock calibration remain open. Do not promote `test:cdp` PASS into whole-gate closure.
- `node scripts/qualification/background/headless-audio-meter.mjs` uses the production graph in real headless Chromium OfflineAudioContext. Seven cases compare rendered samples (error <1e-6) and FFmpeg peak (<0.15 dB, silence floored): one source, two tones, coherent sum, phase cancellation, mute, −6 dB and a silent destination. `test:background` includes this graph step and the native `phase51_audio_mix_acceptance` suite; no additional dependencies are required. Evidence: `artifacts/background/audio-meter-graph.json`.
- `powershell -NoProfile -File scripts/qualification/background/cdp-desktop-smoke.ps1 -SavedRecovery` verifies UI edits/autosave of a saved project, process restart, native recovery check/stale-fingerprint rejection/restore, then UI Save and native reopen. It compares project/recovery bytes before restore. Evidence: `artifacts/background/cdp-saved-recovery.json`. It does not drive the saved-project recovery prompt.
- The native chooser boundary was investigated: CDP controls WebView content but not the Windows file chooser; Tauri's invoke function was immutable in the tested build, so a narrow chooser-result harness could not be installed. No native-input fallback or IPC replacement is used. Saved recovery prompt and G49 batch/cancel through the file picker remain unqualified. Same-profile restart now waits for profile-owned WebView processes to terminate before relaunch.
- Add `-Recovery` to the real-WebView runner to use two process launches on the same isolated desktop/profile. It changes a proxy setting via UI, polls actual native recovery listings until autosave exists, restarts, selects the test project's recovery card, restores it through UI, and saves/reopens via commands. The setting is restored after the resume stage. Evidence: `artifacts/background/cdp-recovery.json`. This tests unsaved-project recovery, not the saved-project recovery chooser. Await native IPC polling explicitly; do not rely on Promise-returning `waitForFunction` as autosave evidence.
- The default CDP run also covers the caption dirty-draft merge guard and Source Viewer In/Out insertion from a generated one-second silent WAV. Import/save/open use native commands; In/Out and insertion use UI. It does not qualify audible playback, batch cancellation or native file-picker behavior.
- Real WebView smoke: build the qualification config as documented below, then run `powershell -NoProfile -File scripts/qualification/background/cdp-desktop-smoke.ps1`. It launches only the qualification binary on a newly created Windows desktop, supplies a unique temporary WebView profile, verifies CDP port 9444 is loopback-only and owned by that profile, checks app-window placement and input desktop, then reads native health/DOM and captures a screenshot through CDP. It sends no native input and never switches desktops. Cleanup terminates the test process/profile-owned WebView processes and removes the temporary profile with bounded retries. Evidence: `artifacts/background/cdp-smoke.json` and `.png`.
- The extended 2026-11-15 smoke creates two Vietnamese text captions through Command Service, then exercises UI merge, Undo/Redo, duration edit and split on the real WebView. It saves/reopens through scoped native commands in a unique temporary `$TEMP/tinixcut-cdp-fixtures` subdirectory. Screenshots confirm visible Vietnamese text on the preview surface. The input desktop remains `Default`; the app window is on the separate desktop. Media playback, native file dialogs and installed acceptance remain unqualified. The qualification config grants only this fixture subtree in addition to inherited settings. Its identifier is `com.tinixcut.qualification`; app-local settings/recovery differ from production but persist across qualification runs. The WebView profile and project subtree are removed after each run.
- `test:background` writes `artifacts/background/latest-report.json` with code revision, tracked working-tree dirty state, caption-fixture SHA-256 and per-step outcomes, including failed steps. This report describes automated evidence only.
- The background runner also executes `headless-draft-stale.mjs` and `headless-planner-stale.mjs`: muted headless React with delayed mocked IPC checks stale draft, trim, reorder, scene suggestion/preview and audio mix results. These use synthetic responses and do not qualify native planner or installed WebView behavior. They require the same Playwright Chromium setup as the other headless checks.
- MockRuntime chains cover stale-fingerprint recovery rejection and restore/save/reopen, relink/Undo/Redo/save/reopen, collection with exact copied-byte comparison and reopen, and Vietnamese multiline subtitle import/export/reimport with history. Collection requires an existing destination folder. Profile restart/settings UI, batch cancel, source scrub, live playback/meter and installed UI remain separate acceptance work.
- Mock IPC coverage also includes reviewed-fingerprint recovery restore, asset relink after a file move plus Undo, and Vietnamese SRT import/WebVTT export timestamp checks. These are scoped temporary-fixture checks in MockRuntime; native UI and playback acceptance remain separate.
- Build the current native CLI first: `cargo build --manifest-path src-tauri/Cargo.toml`.
- Run `pnpm exec playwright install chromium --only-shell` once, then `pnpm run test:background`. The runner checks the CLI exists and runs the caption component, App mocked-IPC checks, mixer calculation tests and a native group-move Undo/Redo test. Steps have a three-minute timeout; on Windows the runner terminates only the timed-out step's process tree. Run against a freshly built CLI; `TINIXCUT_TEST_BINARY` selects a different executable.
  - The App test hydrates a disposable fixture through mocked IPC and exercises caption merge success and simulated command rejection. Successful merge delegates the envelope to the real CLI and reloads the saved file through the native stdio session. A mixed media-import case mocks the file chooser and both per-file import results to check App reporting; it does not verify actual import or native dialogs. CLI/MCP expose save/open but do not currently expose Undo/Redo. A separate native CommandService/HistoryManager test verifies caption merge as one history entry and save/reopen; it does not test the App's actual Tauri IPC history wiring.
  - Native background checks also exercise a group move followed by save/reopen, corrupt recovery handling, and real ffprobe/classification of a generated valid WAV and corrupt MP4. Mock Tauri IPC additionally exercises `media_import` with file scope, one valid asset, corrupt and out-of-scope rejection, and undo/redo; it does not exercise native dialogs or a real WebView.
  - On 2026-11-15 loader tracing identified `TaskDialogIndirect` missing from Common Controls 5.82 as the cause of the MockRuntime test executable's `0xc0000139` startup error. `powershell -NoProfile -File scripts/qualification/background/run-mock-ipc.ps1` builds the opt-in `mock-ipc-tests` feature, copies the test executable to a temporary directory, uses Windows SDK `mt.exe` to embed the v6 manifest on the copy, and runs caption and media import IPC tests. This is a mock Tauri runtime, not a real WebView or installed app; `pnpm run test:background` includes it.
  - On Windows use that runner for `mock-ipc-tests`; directly running `cargo test --all-features` lacks the required v6 test-executable manifest and may fail before the harness starts.
- Install the pinned Playwright browser once with `pnpm exec playwright install chromium --only-shell`, then run `pnpm run test:caption:headless`. This bundles an isolated React Caption Editor harness into a temporary directory and starts a loopback-only server and Chromium in headless mode. The UI-generated merge operations are applied to separate temporary project copies via the real CLI and MCP stdio server and the saved timelines are compared; the temporary server, browser, and project copies are cleaned up. `TINIXCUT_TEST_BINARY` selects another explicitly built executable.
- The headless test covers merge selection, unsaved text/timing guards, reversed selection, gap/locked-track guards, merged text and duration, CLI/MCP parity, two-track timing notices and long-list timing filtering with pending edits, focus, search and invalid timing. The harness directly supplies the merge callback and does not exercise the full `App` wiring, Tauri IPC, real undo/redo, native dialogs, installed UI, playback, or export; it does not close native gate acceptance.
- Run `$env:TINIXCUT_RUN_NATIVE_TESTS='1'; pnpm exec vitest run scripts/qualification/background/captionMerge.integration.test.ts; Remove-Item Env:TINIXCUT_RUN_NATIVE_TESTS`.
- The test copies its synthetic fixture into separate temporary directories and generates the same merge operations as the UI. It tests CLI apply and MCP stdio dry-run/apply through the Command Service, then checks the persisted projects. `TINIXCUT_TEST_BINARY` can select another explicitly built executable. The test does not exercise the Tauri UI, undo/redo, or installer.
- Run `powershell -File scripts/qualification/background/webdriver-preflight.ps1` to inventory Tauri and Edge drivers and WebView2 without launching a GUI. Install a compatible Edge WebDriver and validate its version against the WebView2 runtime before using it.
- A Tauri WebDriver session may open or focus a native window. Do not launch it on the user's active desktop or mark GUI acceptance complete without verified isolation. Native file dialogs and installer checks remain separate acceptance work.
- `powershell -NoProfile -File scripts/qualification/background/isolated-desktop-probe.ps1 -DriverSmoke` creates a temporary separate Windows desktop without switching to it, opens a test form there, checks the child and input desktop names, and checks the local tauri-driver status. It does not perform WebDriver input actions. This probe passed with input desktop `Default`. It expects tauri-driver in Cargo bin and signed Edge WebDriver 157.0.4341.57 under `%LOCALAPPDATA%\TinixCutTestDrivers\157.0.4341.57`, and refuses occupied probe ports. Recheck compatibility if WebView2 updates.
- Build the separate qualification binary with `$env:CARGO_TARGET_DIR='D:\___TOOLS\__TINIX\TinixCut\src-tauri\target-qualification'; pnpm tauri build --debug --no-bundle --config scripts/qualification/background/tauri-isolated.conf.json`. Its configured identifier is `com.tinixcut.qualification`; the smoke runner also assigns a temporary WebView profile. Tauri's app-local data uses the Windows known folder and app identifier, so overriding `LOCALAPPDATA` alone is insufficient. Never use real projects or recovery data in qualification.
- `powershell -NoProfile -File scripts/qualification/background/isolated-desktop-probe.ps1 -AppSmoke` is experimental. On 2026-11-15 the qualification process started but both session-creation attempts timed out after 45 seconds; no app window desktop, health check or WebDriver input was verified. Cleanup left the normal app running and no test driver/app processes. Do not use it for acceptance or claim input isolation until a real session and window verification pass. The separate desktop is not a security boundary.
- Diagnostic follow-up: `-Diagnostic` calls the native Edge driver directly with the same translated WebView2 capabilities and automation environment used by tauri-driver, and writes verbose driver logs and process/window snapshots under `artifacts/phase59/isolated-webdriver/`. The qualification process had a window on the separate desktop but no WebView2 process matching the temporary profile was observed before the 45-second timeout. An independently built minimal Tauri control (`-Control`) showed the same result. This implicates the shared test environment or setup, not specifically TinixCut; the root cause is unproven. Direct Edge-driver diagnostics bypass tauri-driver, so they do not establish its behavior independently. No WebDriver input was sent; preserve these limitations when assessing G53.
- Hyper-V inventory on 2026-11-15 found `TinixCut-ComputerUse-Qualification` with an effectively empty 4 MiB VHDX, no attached installer and no guest heartbeat. It is not a usable guest yet. Provision a Windows guest with a valid installation image/license and guest account, then install the project runtime, WebView2, matching Edge WebDriver, and tauri-driver in that guest. Verify remote execution and that the host's foreground window and input remain untouched before attempting GUI qualification. Do not expose WebDriver beyond the isolated guest.

Development stop command:
- Ctrl+C or kill Tauri dev process

Development restart command:
- Terminate process and rerun `pnpm tauri dev`

Frontend dev command:
- pnpm dev

Backend dev command:
- N/A as a separate service unless implementation later introduces one

Targeted test command:
- Audio silence suggestions: `pnpm test -- src/components/silenceCuts.test.ts src/components/transcriptCut.test.ts`; set `TINIX_SILENCE_EVIDENCE` to the absolute path `artifacts/background/silence-analysis-native.json`, then run `cargo test --manifest-path src-tauri/Cargo.toml --test silence_analysis`. This writes a real FFmpeg-derived report used by `node scripts/qualification/background/headless-silence.mjs` for MediaPanel/dialog integration. It uses mocked IPC lifecycle, not a second detector/reducer or native picker; reports/screenshot are retained under `artifacts/background/silence-review/<timestamp>` with `silence-review-latest.json`. Chromium remains headless/muted and the dialog creates no playback elements. Native stream/probe/decode tests write only disposable source files and never open an audio device.
- `node scripts/qualification/background/silence-chain.mjs` runs muted headless review then its fresh multi-cut command batch against real Command Service via MockRuntime, checks the history/persistence marker and writes `artifacts/background/silence-chain-latest.json` and native log. Failure exits nonzero; only a successful invocation proves current results.
- Native silence job/history/scope checks: `powershell -NoProfile -NonInteractive -File scripts/qualification/background/run-mock-ipc.ps1 -TestFilter silence` uses the existing Common Controls v6 test-copy workflow; the optional filter defaults to `mock_tauri_ipc_`, preserving the existing suite command. The silence filter includes detector/retention and real MockRuntime IPC apply/history/save/reopen tests. Native debug build: `cargo build --manifest-path src-tauri/Cargo.toml --bin tinixcut` after verifying its output is not running. No app launch/installer is needed for these checks.
- Timeline navigation: `pnpm test -- src/components/timelineNavigation.test.ts src/__tests__/timelineShortcuts.test.ts`, then `node scripts/qualification/background/headless-timeline.mjs` for real React/shortcut integration in headless Chromium. This runs asset-free UI fixtures without native input or audio, builds only a disposable test site under `artifacts/background/timeline-navigation/<timestamp>/site`, and writes `timeline-navigation-latest.json` plus archived report/screenshot. Requires the existing Playwright Chromium installation; no Tauri runtime, installer or native picker is used. It verifies Fit/resize/zoom/focus and zero edit calls, not native/installed acceptance.
- Frontend: pnpm test
- Native core: cargo test --manifest-path src-tauri/Cargo.toml

Typecheck command:
- pnpm run typecheck

Lint command:
- cargo clippy --manifest-path src-tauri/Cargo.toml

Build command:
- Frontend: pnpm run build
- Native: cargo build --manifest-path src-tauri/Cargo.toml
- Full desktop app: pnpm tauri build

Full test command:
- pnpm run test:all

Health check:
- Desktop runtime readiness via typed Tauri command `app_health_check`
- App window loads
- Rust core initializes and returns `HealthStatus`
- required local media capability is available when the current phase depends on it
- no fatal startup error
- latest code/revision is active

Package command:
- pnpm tauri build (Generates NSIS installer at src-tauri/target/release/bundle/nsis/TinixCut_0.1.0_x64-setup.exe with pinned Gyan Release Essentials FFmpeg/ffprobe 9.0.2 resources)

Isolated release-candidate preparation (no desktop input):
- `node scripts/qualification/background/prepare-candidate.mjs` copies explicit build inputs into a new `artifacts/candidates/<id>/source`, writes per-file SHA-256 plus Git revision/dirty state, and links the installed node_modules tree for dependency reuse. It does not commit, install dependencies or copy user projects/settings. Both lockfiles and tool versions are recorded; byte-for-byte build reproducibility is not claimed.
- Read `artifacts/candidates/latest-candidate.json`, set `CARGO_TARGET_DIR` to its `targetDirectory`, change to its `source`, then run `pnpm tauri build --bundles nsis --config candidate.conf.json`. Save output as `<candidate.directory>/build.log`. The existing release cache is reused only after verifying its output binary is not running. Frontend build runs inside the frozen source, so the checkout's dist/runtime is not replaced. Only the release cache binary is overwritten; its prior installer remains separate.
- Candidate overrides are limited to a unique product name/identifier and an unfocused window title. Build is release, production CSP/asset scope/resources remain, no debug CDP arguments or qualification-only capabilities are included. This isolated identity intentionally differs from `com.tinixcut.editor`; it cannot qualify production-identity upgrade behavior.
- After successful build, `node scripts/qualification/background/finalize-candidate.mjs` verifies all frozen input hashes and the generated NSIS identity, then copies the installer, release executable, NSIS script and pinned media resources into `<candidate.directory>/package`. It records exact hashes/bytes in `candidate.json` and `latest-candidate.json`. Do not rebuild over or mutate the copied package when performing later acceptance.
- `powershell -NoProfile -NonInteractive -File scripts/qualification/background/install-candidate.ps1 -CandidateReport <candidate.json>` checks the installer hash and unused candidate registry namespace, requires an already registered WebView2 runtime (no bootstrap download), silently installs with `/S /NS /D=<new candidate directory>/installed` and no `/R`, then verifies executable/resources/registration and hidden CLI `--help`. Tauri temporarily patches the unique bundle marker from UNK to NSS for NSIS and restores the standalone executable; finalization records the expected installed-payload SHA-256 separately without modifying the artifact. Production registry/settings snapshots are compared. Installer has a 120-second timeout; no GUI app is launched and no mouse/keyboard events are used. The installed candidate is retained for follow-up testing; old TinixCut installations/settings are not uninstalled or relinked.
- `-VerifyExisting` checks an existing install without reinstalling and writes `install-verification-report.json`, preserving `install-report.json`. It uses the retained first-check-failed receipt when present, otherwise the original successful install receipt; an explicitly annotated recovery branch accepts a matching successful candidate.installation certificate if an earlier verifier overwrote the original report. A missing/mismatched success record is rejected. Use PowerShell 7 for verification because registry snapshots may have empty default-value names (`ConvertFrom-Json -AsHashtable`). The first candidate's marker-check failure remains in `install-first-check-failed.json`; no such file is expected after a normal first-pass install.
- `install-report.json` is installation/resource/CLI readiness evidence only. Exact-candidate GUI, recovery, picker, rendering/audio and upgrade tests are separate; older qualification/installer hashes must not be promoted to current installed acceptance. A unique app identifier reserves its own settings namespace, but native settings/WebView profile are not initialized or qualified by CLI help alone.


Installed candidate acceptance (no desktop input):
- Run `node scripts/qualification/background/installed-acceptance.mjs` after installation. It validates the exact installed payload hash, uses hidden MCP/session processes and disposable candidate-local projects, verifies command retry/stale rejection, save-copy/reopen, offline rejection/relink, export/status/full decode with packaged tools, real local Whisper and recognized-text command correction preserving another confirmed caption. Reports and exports remain under `<candidate.directory>/acceptance`; the latest report is `installed-acceptance-latest.json`. Whisper fixture/model/CLI must already exist at the recorded Phase 26 paths. This does not qualify GUI correction, history/recovery or file pickers.
- For the combined follow-up, set `TINIXCUT_TEST_BINARY` to `<candidate.installation.directory>/tinixcut.exe`, then run `pnpm run test:cdp -- history-branch captions media`. The caption case requires a successful current `installed-acceptance-latest.json` with matching source/model/CLI hashes to independently compare actual transcript endpoints. The media case additionally uses the exact installed CLI to reopen/render/decode the collected copy while the former linked temporary source is absent. Relink/collect and UI actions themselves still run in the qualification WebView. Without the binary environment variable, the media case makes no installed-render claim.
- `history-branch` is included in the default eleven-case CDP suite: saved/unsaved Apply→Undo→Redo→Undo→new edit, monotonic revisions, cleared Redo, recovery snapshot and saved reopen. Snapshot creation is not a claim of restored-chooser UI. Caption/media reloads defer existing recovery prompts with Later; recovery bytes are retained.
- Run `powershell -NoProfile -NonInteractive -File scripts/qualification/background/installed-gui-preflight.ps1 -CandidateReport artifacts/candidates/latest-candidate.json` separately. It launches the exact installed release on its own desktop/profile, verifies the visible main-window identity and profile-owned WebView, reads UIA without input, requests WM_CLOSE on that exact window, and checks app/WebView exit and unchanged input desktop. The temporary profile is retained; no desktop switch, input injection, recursive deletion or force-stop is used. Report: `<candidate.directory>/gui-preflight.json`. PASS means window/profile/cleanup readiness; `readOnlyUIA.health=unverified` must not be promoted to GUI health, rendered preview or editing qualification. `-ResumeOwned` only reattaches a failed preflight process whose path/PID/hash match the prior receipt; prior failures are retained.


Installed GUI diagnostics and silent text-preview acceptance:
- Add `-Mode accessibility` to the installed preflight to request renderer accessibility without CDP, or `-Mode cdp -StandardUser` for the exact installed binary's health/preview test. Flags are process-local; no registry/runtime/package change is made. Run from the workspace. WebView2 may ignore environment browser flags on elevated hosts (observed with Runtime 157.0.4341.57); a requested flag is not proof it applied. CDP mode verifies the owned loopback listener and actual `--mute-audio` on its browser process before testing. See [upstream elevated-host discussion](https://github.com/tauri-apps/wry/pull/1783).
- `-StandardUser` uses the current user's linked non-elevated token if available. On this UAC configuration no linked token exists, so the harness creates a LUA-restricted current-user token at medium integrity. Its default DACL grants its own user and SYSTEM access to objects it creates; omitting that made WebView2 fail to create further sandbox tokens (`GPU process launch failed: error_code=49`, then 0xC0000409). This changes only the temporary child token, never account privileges, global ACLs, registry or sandbox policy. No sandbox-disabling flags are used. Receipts record which token was used.
- `installed-gui-preview.mjs` reads the explicitly selected candidate receipt passed by the parent, validates binary hash/native window title, invokes real native health and reads React's Core Healthy badge. It creates three asset-free caption markers through the installed Command Service, uses a canonical state snapshot as a disposable CLI-export fixture, and compares actual WebView viewport screenshots at frames 0/30/75 with independently decoded export frames. The comparison excludes the diagnostic HUD/border, locks mean absolute RGB error below 3/255 and rejects blank output; marker identity and frame changes are checked. Native save/open and Exact-frame background rendering are not claimed by this snapshot test. Text-only compositions use shared raster overlays and have no non-text Exact-frame background.
- Silent transport checks Play advances and Pause stops the playhead, requires zero video/audio elements, and retains the process-wide mute flag. This does not qualify video decoding, source seek, audio output, native dialogs or whole-gate parity. Release file scopes remain enforced: arbitrary media import/project paths are rejected until granted through the native picker. Do not widen scopes or substitute qualification permissions to make the installed smoke pass.
- Per-run reports: `gui-cdp-<UTC timestamp>.json`, `gui-<mode>-active.json` checkpoints, `gui-preview/<run>/report.json`, `gui-preview-latest.json`, screenshots and decoded reference frames under the candidate directory. Runtime log is `webview-diagnostic.log`; failed runs/profiles remain retained. Parent completion additionally requires graceful app/browser exit, no live profile WebViews and unchanged input desktop. A visible window or open CDP port alone is insufficient. Node stdout/stderr and exit code are recorded. No native input, desktop switch, force-stop or profile deletion is performed by this path.

Release qualification command:
- cargo test --manifest-path src-tauri/Cargo.toml -- test_gate_f_poc5_matrix_execution --nocapture (Runs POC 5 qualification matrix P5-M01..P5-M16 and emits tests/evidence/poc5_qualification_report.json)

HMR / live reload:
- Partial
- Expected for ordinary React/UI development
- Native Rust/Tauri changes may require whole-app restart

Manual UI reload:
- Runtime-specific WebView reload / development action once established
- Do not assume a production-facing reload control
```

Phase 0 MUST replace command-level `TBD` values with the real repository commands once those commands exist. Do not guess them earlier.

---

## 3. Development Mode Is the Default

Unless the task explicitly concerns release, distribution, installer behavior, or production packaging, assume:

`Development / Test Mode`

This means:

- prefer running from source;
- prefer development runtime;
- prefer targeted validation;
- use the least disruptive reload/restart mechanism;
- do not package distribution artifacts by default;
- leave the application ready for testing.

Do not turn routine development into a release pipeline.

---

## 4. Build and Package Are Different

A **build** validates or prepares executable application code.

A **package** creates a distributable artifact.

### 4.1. Build

A build may detect:

- compile errors;
- type errors;
- bundling errors;
- module/import errors;
- dependency incompatibilities;
- configuration problems;
- production-only build failures;
- frontend/backend integration issues.

A build does not necessarily create an installer or executable distribution.

### 4.2. Package

Packaging may create artifacts such as:

- `.exe`;
- `.msi`;
- `.dmg`;
- `.app`;
- `.apk`;
- `.ipa`;
- installer;
- portable build;
- release archive;
- production distribution bundle;
- container image;
- deployable package.

Do not treat `build` and `package` as synonyms.

---

## 5. Do Not Package After Every Coding Task

Routine development MUST NOT automatically trigger packaging.

Do not automatically:

- create installers;
- build release archives;
- create production executables;
- run release pipelines;
- rebuild distributable packages merely to test a normal code change.

Package only when:

- the user explicitly requests it;
- the task concerns packaging or distribution;
- packaged-runtime behavior must be tested;
- installer behavior must be tested;
- auto-update or release behavior changed;
- a release/milestone requires it;
- a documented technical requirement makes packaging necessary.

Otherwise, test from source or the normal development build.

---

## 6. Decide Whether a Build Is Needed

A full build is **not required after every small change**.

Use the smallest validation level that gives sufficient confidence.

### 6.1. Build Often Not Required

A full build MAY be skipped for narrow changes such as:

- copy/text;
- CSS;
- spacing;
- visual styling;
- small icon changes;
- isolated layout adjustments;
- narrow frontend presentation changes;
- very local logic changes when the development runtime already compiles them reliably.

Typical flow:

`Change → targeted validation → HMR/reload → test`

### 6.2. Build Recommended or Required

A build SHOULD be considered when changing:

- types/interfaces;
- dependencies;
- package configuration;
- bundler configuration;
- environment handling;
- module/import structure;
- shared libraries;
- API contracts;
- backend compilation;
- database integration;
- routing/bootstrap architecture;
- Electron/Tauri/native layer;
- application startup;
- production runtime behavior;
- code that may fail only during compile/build.

Typical flow:

`Change → validate → build → apply latest runtime → health check`

Do not package merely because a build was run.

---

## 7. Validation Strategy

After coding, run validation proportional to the change.

Possible checks:

- targeted tests;
- unit tests;
- typecheck;
- lint;
- compile;
- build;
- integration tests;
- smoke tests;
- health checks;
- runtime verification.

Principles:

1. Start with the smallest meaningful check.
2. Broaden validation when risk is higher.
3. Run mandatory project checks.
4. Do not run expensive full suites mechanically when targeted checks are sufficient.
5. Do not skip meaningful validation simply to save time.
6. Fix failures caused by the current task.
7. Report unrelated pre-existing failures instead of silently expanding scope.

---


## 7.1. Project-Specific Validation Map

The architecture creates several correctness boundaries that require stronger validation than ordinary UI work.

| Area changed | Minimum focused verification |
|---|---|
| Project domain / canonical time | Domain unit tests + invariants/property tests relevant to the change |
| Command Service | Command tests including validation; revision/idempotency/transaction behavior when affected |
| History / undo-redo | Round-trip behavior and state restoration tests |
| Persistence / migration / recovery | Save-load round trip + atomic/recovery or migration test relevant to the change |
| Asset identity / relink | Fixture-based relink/offline-media tests |
| Tauri IPC / Rust native boundary | Rust tests/build as relevant + whole-app restart/runtime verification |
| React-only presentation | Targeted frontend tests/typecheck as relevant + HMR/reload verification |
| Timeline gesture behavior | Interaction test where practical + verify transient state does not bypass authoritative commit |
| FFmpeg/media service | Media fixture integration test using the pinned/audited toolchain |
| Job Manager | Lifecycle/cancel/supersede tests relevant to the change |
| Preview semantics | Representative media/timeline fixture playback check |
| Composition/export semantics | Determinism/parity fixture checks relevant to the supported feature |
| Automation adapter | E2E proof that it uses the same Command Service semantics as manual editing |

Do not substitute a successful frontend render for domain, persistence, media, or command correctness.

## 7.2. Architecture-Gate Validation

When work belongs to an architecture gate in `PLAN.md` / `ARCHITECTURE.md`, the gate's pass criteria are mandatory.

In particular:

- do not broaden timeline feature work if the Command Service gate is not satisfied;
- do not treat save/recovery as complete without the required recovery/relink validation;
- do not permanently lock the preview backend before the Preview POC decision gate;
- do not claim preview/export parity without representative parity fixtures;
- do not finalize FFmpeg distribution assumptions before the licensing/hardware-fallback validation scheduled by the plan.

A green generic build does not override a failed architecture gate.


# 8. Development Reload Strategy

Use the **least disruptive mechanism that makes the latest code active and testable**.

Priority:

`HMR / Live Reload → Reload UI → Restart affected service → Restart whole app`

The agent SHOULD choose the lowest level that reliably applies the change.

---

## 8.1. Level 1 — HMR / Live Reload

Prefer HMR or live reload for ordinary frontend changes when supported and reliable.

Typical examples:

- CSS;
- layout;
- text;
- icons;
- frontend components;
- many local frontend logic changes.

Desired behavior:

`Save code → HMR applies change → UI updates automatically`

The user should not need to manually restart the application for ordinary frontend work.

If HMR successfully applies the latest code, do not restart more of the application.

---

## 8.2. Level 2 — Reload UI

Use a clean frontend/UI reload when:

- HMR is unavailable;
- HMR failed;
- HMR preserved stale state;
- initialization behavior needs testing;
- cached frontend state is suspicious;
- a clean page/app-shell load is required.

Possible mechanisms:

- `F5`;
- `Ctrl+R`;
- framework/runtime reload;
- development-only `Reload UI` action;
- browser refresh.

A UI reload SHOULD:

- reload the frontend;
- avoid restarting the backend when unnecessary;
- avoid rebuilding when unnecessary;
- avoid restarting the whole application when unnecessary.

For Tinix desktop applications, a development-only `Reload UI` action MAY be provided when useful.

---

## 8.3. Level 3 — Restart Affected Service

Restart only the service whose runtime must load new code.

Examples:

### Restart backend when changes affect:

- API code;
- server logic;
- backend configuration;
- backend environment variables;
- database integration;
- backend dependency loading;
- shared code consumed by backend runtime;
- server bootstrap;
- backend runtime state.

### Restart frontend dev server when:

- dev-server configuration changed;
- bundler configuration changed;
- environment variables were loaded only at startup;
- HMR cannot apply the change;
- frontend process is stale or unhealthy.

Do not restart backend for a frontend-only change unless the project runtime actually requires it.

---

## 8.4. Level 4 — Restart Whole Application

Restart the full application only when necessary.

Typical triggers:

- desktop main-process changes;
- Electron/Tauri/native changes;
- native modules;
- startup logic;
- launcher changes;
- application bootstrap changes;
- process-level configuration;
- dependency loading that occurs only at startup;
- runtime changes that cannot be applied with frontend/backend reload alone.

Do not use full app restart as the default response to ordinary UI changes.

---

## 8.5. Core Reload Rule

**Do not restart more of the application than necessary.**

The objective is:

- latest code active;
- minimal interruption;
- fast feedback;
- reliable test state.

---

## 9. Manual Refresh Is a Fallback, Not the Primary Workflow

The preferred user experience is automatic update through HMR/live reload.

Manual refresh exists for recovery and clean-state verification.

Recommended order:

1. Automatic HMR/live reload.
2. Manual `Reload UI`, `F5`, or `Ctrl+R`.
3. Restart affected runtime.
4. Restart whole application.

The user SHOULD NOT need to press refresh after every normal frontend edit.

---

## 10. Preserve Test Context When Reasonable

Fast iteration benefits from preserving useful state.

HMR MAY preserve:

- current screen;
- selected item;
- panel state;
- form state;
- navigation context.

However, preserved state can hide initialization problems.

Therefore:

- use HMR for rapid iteration;
- use clean UI reload when validating initialization;
- use full restart when validating startup behavior.

Do not assume HMR proves that a clean application launch works.

---

## 11. Restart Timing

Do not restart after every tiny intermediate edit.

During implementation:

1. make the necessary edits;
2. run relevant validation;
3. apply/restart the affected runtime after the final meaningful change;
4. verify the runtime;
5. leave the app ready for user testing.

An earlier restart is appropriate when needed for debugging.

---

## 12. Process Ownership and Duplicate Instances

Before stopping or restarting a process, identify the correct project-owned process.

Prefer evidence such as:

- project-owned PID file;
- command line;
- working directory;
- executable path;
- process metadata;
- runtime-specific ownership information;
- port plus additional ownership evidence.

A port number alone SHOULD NOT be treated as sufficient evidence when safer ownership information exists.

Do not:

- kill all Node processes;
- kill all Python processes;
- kill unrelated development servers;
- terminate unrelated applications.

Avoid:

- duplicate frontend servers;
- duplicate backend servers;
- duplicate desktop app instances;
- stale processes holding ports;
- zombie processes;
- testing against an old runtime.

---

## 13. Development Scripts

Projects SHOULD provide simple, predictable development entry points when useful.

Use the platform and stack that fit the project.

Examples:

### Windows

- `.bat`
- `.cmd`
- `.ps1`
- `.vbs`

### Cross-platform / runtime-specific

- package-manager scripts;
- shell scripts;
- task runners;
- Makefile targets;
- framework-native dev commands.

Possible entry points:

```text
DEV_START
DEV_STOP
DEV_RESTART
DEV_STATUS
```

or a single idempotent restart command.

Do not create multiple overlapping scripts when the repository already has a reliable workflow.

---

## 14. Restart Script Requirements

If a project provides a restart script, it SHOULD be safe to run repeatedly.

A good restart workflow can:

1. identify project-owned processes;
2. stop only affected processes;
3. wait for shutdown when necessary;
4. release required ports/resources;
5. start required services;
6. avoid duplicate instances;
7. preserve or write useful startup logs;
8. verify startup;
9. return a clear success/failure status.

Prefer **idempotent** scripts.

Running the same restart command twice SHOULD NOT leave duplicate runtimes.

---

## 15. Do Not Replace Existing Workflow Without Need

Before adding a new development script or process manager, inspect whether the repository already has:

- start command;
- dev command;
- restart command;
- build command;
- test command;
- health check;
- process manager;
- PID management;
- launcher;
- framework-native HMR.

If the existing workflow is reliable, extend or repair it rather than creating a parallel system.

Do not break commands the user already relies on without a concrete reason.

---

## 16. Health Check After Restart

A successful start command is not sufficient proof that the application is ready.

After a runtime restart, verify the relevant runtime.

### Frontend

Possible checks:

- dev server is listening;
- expected frontend URL responds;
- app window loads;
- UI bundle loads;
- no fatal startup error;
- latest relevant code is active.

### Backend / Service

Possible checks:

- process is alive;
- expected endpoint responds;
- health endpoint passes;
- relevant API responds;
- no crash loop;
- no fatal startup exception.

Prefer project-defined health endpoints such as:

```text
/health
/api/health
/ready
/status
```

when available.

### Desktop / Native App

Possible checks:

- app process starts;
- application window opens;
- required local services are healthy;
- UI loads;
- startup logs contain no fatal error.

---

## 17. Verify That the Latest Code Is Running

A runtime can be healthy while still running stale code.

When relevant, verify freshness using the strongest available evidence:

- HMR confirmation;
- process start time after the final change;
- build/revision ID;
- version/fingerprint;
- changed behavior;
- runtime log;
- development server rebuild confirmation.

Do not rely only on:

- browser refresh;
- successful build;
- process existence.

The goal is to verify that the user is testing the current implementation.

---

## 18. Startup or Restart Failure

If restart/startup fails:

1. inspect the relevant log or error;
2. determine whether the failure is related to the current task;
3. fix it when within scope;
4. rerun relevant validation;
5. retry the minimum required runtime action;
6. rerun the health check.

Do not report the app as ready to test if required verification failed.

If blocked by an unrelated or unsafe issue, report:

- failed command/action;
- main error;
- affected runtime;
- what still works;
- remaining blocker.

Do not hide a failed restart behind a successful build.

---

## 19. Avoid Unnecessary Desktop Disruption

When developing local applications:

- avoid opening duplicate terminal windows;
- avoid opening duplicate browser tabs;
- avoid spawning new windows every iteration;
- clean up project-owned stale processes;
- prefer background processes when appropriate;
- preserve the user's current testing context when possible.

Development automation SHOULD make testing easier, not create desktop noise.

---

## 20. Long-Running and Data-Sensitive Work

Do not automatically interrupt important running work merely to restart a runtime.

Examples:

- export;
- render;
- batch processing;
- upload;
- migration;
- transcoding;
- long-running user job;
- data mutation.

If restart would interrupt data-sensitive work:

- follow the project-specific policy;
- wait for a safe point when appropriate;
- use a restart mechanism that preserves work if available;
- report the conflict if it cannot be resolved safely.

A project MAY explicitly authorize interruption for specific development-only jobs, but that authorization should be documented in the Project Runtime Profile or another project-specific rule.

For this editor, export and any user-visible media job that may produce durable output MUST be treated as data-sensitive. Do not restart the native runtime mid-job merely to apply code unless the task explicitly requires failure/recovery testing or the user authorizes interruption.

---

## 21. Environment and Configuration Changes

Environment/config changes often require stronger runtime actions than ordinary code changes.

When changing:

- `.env`;
- runtime environment variables;
- bundler configuration;
- server configuration;
- native configuration;
- dependency resolution;
- process startup settings;

determine which processes read those values only at startup.

Restart only those affected processes.

Do not assume HMR reloads startup-time configuration.

---

## 22. Development-Only Controls

A project MAY expose development-only controls such as:

```text
Reload UI
Restart Backend
Restart App
Open Logs
Show Runtime Status
```

These controls can improve manual testing.

Rules:

- development controls MUST NOT appear unintentionally in production;
- `Reload UI` SHOULD be the least disruptive option;
- destructive or high-impact developer actions SHOULD be clearly distinguished;
- developer controls SHOULD reuse the project's normal restart mechanisms rather than invent a second runtime path.

---

## 23. Optimize the Development Loop

The goal is fast feedback without sacrificing correctness.

Avoid the default workflow:

`Change → clean everything → full test suite → full build → package → installer → full restart`

for a small UI change.

Prefer:

`Change → targeted validation → HMR/reload → verify`

For larger changes:

`Change → tests/typecheck → build → restart affected runtime → health check`

For startup/native/package changes:

`Change → relevant validation/build → restart full app or package when justified → verify`

---

## 24. Packaging and Release

Packaging is a separate phase from normal development.

Package when:

- explicitly requested;
- validating packaged behavior;
- preparing a release;
- testing installer/distribution;
- changing packaging, updater, signing, native distribution, or deployment behavior.

For release work, additional checks MAY include:

- clean build;
- full test suite;
- package generation;
- installer test;
- upgrade test;
- signing;
- release notes;
- version validation;
- deployment verification.

Do not impose release-level work on ordinary development iterations.

For this project, routine packaging SHOULD be deferred until the plan reaches the packaging/distribution validation work or the user explicitly requests it. POC work may exercise FFmpeg/sidecar distribution behavior without turning every development iteration into an installer build.

---

## 25. After Each Coding Task

Before reporting a coding task complete, ensure when applicable:

- requested change is implemented;
- relevant validation passed;
- build ran if justified;
- latest frontend code is active;
- affected backend/service was restarted if needed;
- whole app was restarted only if needed;
- duplicate project runtimes are not obviously present;
- relevant health verification passed;
- application is ready for user testing.

The final report SHOULD stay concise.

Include when relevant:

- what changed;
- validation performed;
- whether build ran;
- reload/restart action;
- runtime/health status;
- blocker or known limitation.

---

## 26. Default Decision Matrix

Use this as a default guide.

| Change type | Validate | Build | Runtime action | Package |
|---|---|---|---|---|
| Text / CSS / spacing | Targeted check | Usually no | HMR | No |
| Normal frontend component | Targeted test/typecheck as relevant | Usually no | HMR → Reload UI if needed | No |
| Frontend env/dev-server config | Relevant check | Maybe | Restart frontend dev server | No |
| Backend/API logic | Targeted tests | Maybe | Restart backend/service | No |
| Shared frontend/backend code | Tests/typecheck | Often useful | Restart affected runtimes | No |
| Dependency change | Typecheck/tests | Usually yes | Restart affected runtimes | No |
| Build/bundler config | Relevant checks | Yes | Restart affected runtime | No |
| Desktop main/native process | Relevant tests/build | Usually yes | Restart whole app | No |
| Startup/bootstrap | Relevant tests/build | Usually yes | Restart whole app/service | No |
| Packaging/installer/updater | Release-relevant checks | Yes | As required | Yes |
| Release | Full required checks | Yes | Clean runtime verification | Yes |

This matrix is guidance. Project-specific runtime behavior takes precedence.

---

## 27. Default Development Loop

Use this default loop:

```text
Inspect current runtime
↓
Implement change
↓
Run targeted validation
↓
Run build only when justified
↓
Apply latest code using the least disruptive mechanism
    HMR
    ↓ if insufficient
    Reload UI
    ↓ if insufficient
    Restart affected service
    ↓ if required
    Restart whole app
↓
Verify latest code is active
↓
Health check relevant runtime
↓
Fix introduced runtime/startup issues if necessary
↓
Leave application ready for user testing
```

Packaging is **not part of this default loop**.

---

## 28. Core Principle

**Use the smallest action that reliably makes the latest code testable.**

Prefer:

`HMR → Reload UI → Restart affected service → Restart whole app → Package only when justified`

Do not restart, rebuild, or package more of the project than necessary.
