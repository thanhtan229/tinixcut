# Phase 57 — Export and release acceptance

**Status:** Latest-source installed candidate workflows and automated acceptance are verified for the recorded scope; G57 remains OPEN only for cross-version project/settings preservation against an authentic earlier-version installer. Literal ENOSPC is optional under the Product Owner's decision.

## Implemented

- Added a read-only `ExportEngine::preflight` bound to project ID/revision. It validates project-matching canvas/rate and codec profile, compiles the canonical composition, checks all referenced video/image/audio sources with the bundled FFprobe, verifies the destination and reports current free disk bytes.
- Existing destinations are reported before export. Rendering still writes to a temporary output and promotes only after success, so an existing file survives failed/cancelled renders.
- Exposed the same preflight through Tauri IPC and MCP `tinixcut_export_start`; MCP now derives an omitted profile from the project settings. The UI runs preflight before starting FFmpeg and reports sources, dimensions, duration, free space and replacement behavior.
- Added read-only MCP `tinixcut_export_preflight` so automation clients can inspect the same report before starting an export. It reports existing-output behavior/free disk and does not mutate the project; unavailable destinations return `EXPORT_PREFLIGHT_FAILED`.
- Checkpoint export metadata from Phase 56 stores the actual output SHA-256, size, codec, dimensions and duration.

## Verification

- Export module tests: 11/11 pass, including output-preservation behavior, existing-destination preflight and silent-video export.
- MCP acceptance: workflow checkpoint lifecycle, export start/status/cancel and cancellation cleanup pass.
- Full MCP acceptance after the Phase 57 regression: **49 passed, 0 failed, 3 ignored**; includes existing-project three-item stdio batch, new-project creation/import, stale-revision recovery and export. The two batch outputs and new-project output pass FFprobe and full FFmpeg decode. Read-only preflight verifies project/output immutability, invalid destination rejection and offline-media rejection. Tool discovery/schema inventory contains 32 tools.
- Frontend typecheck, tests (150/150), production build, `cargo check --bin tinixcut`, `cargo fmt --check` and `git diff --check` pass.
- Production frontend bundle emits the existing advisory warning for a 567.46 kB minified JS chunk.

## NSIS candidate check — 2026-11-04

- Release bundle build succeeded from the current source in an isolated `CARGO_TARGET_DIR`: `TinixCut_0.1.0_x64-setup.exe`, 62,102,537 bytes (59.23 MiB), SHA-256 `677158a22f34052c923ac4718d57158ba5f3ef2e4587b0705375af8b519edb4b`.
- Silent install of this final candidate into a unique `%TEMP%` directory exited 0; silent uninstall exited 0 and removed the isolated install directory. Installed executable reports product version `0.1.0`; FFmpeg/FFprobe binaries and `ffmpeg_provenance.json` were present. Installed FFmpeg and FFprobe hashes match the checked-in provenance manifest.
- Installed FFprobe identified the disposable Phase 45 MP4 as H.264 1920×1080 with 450 frames; installed FFmpeg decoded the whole file with exit 0. 
- Machine-readable evidence: [`phase57_nsis_candidate_evidence.json`](../artifacts/phase57/phase57_nsis_candidate_evidence.json). Installer remains in the isolated temp build target; it was not copied over the user's existing TinixCut installation.
- This package check does not qualify app launch/rendering/UI on the installed candidate. The native window cannot currently be observed through Computer Use in this session.

## Remaining G57

- Exact candidate install and process-level launch now pass in a unique temp directory: the installed executable started with window title `TinixCut`, remained alive for eight seconds, then the isolated install was removed successfully. Screenshot/rendering and upgrade/data-preservation verification remain open.
- Run manual, AI draft and MCP batch workflows in native TinixCut and inspect those outputs. The three-project MCP stdio batch now passes; native UI review, unsaved-project ownership and manual/AI native workflows remain open.
- Installed MCP missing-media path now passes through both preflight and export_start with a timeline-referenced offline source; machine evidence is in [`phase57_installed_offline_mcp_evidence.json`](../artifacts/phase57/phase57_installed_offline_mcp_evidence.json). Installed UI missing-media/cancellation and upgrade-preserves-user-data scenarios remain open. Installed MCP cancellation now passes as documented below. User-facing help/diagnostics and installed UI workflows still need native review.
- Searched the project release target, phase artifacts, isolated package target, `%TEMP%`, and Downloads for an authentic older installer. Available TinixCut installers are both version `0.1.0`; no cross-version upgrade was attempted or claimed. A genuine prior-version installer is needed to close that specific check.
- Computer Use/native runtime observation is not exposed in this agent session. The process-level launch smoke test does not prove pixels rendered or any editor workflow; do not claim native UI qualification.

## Runtime checkpoint

- After the caption regression source edit, the workspace Tauri watcher rebuilt/restarted its isolated candidate: PID 33736 at `%TEMP%\tinixcut-phase55-runtime-target\debug\tinixcut.exe`, started 17:27:56; binary timestamp 17:27:53, SHA-256 `68FC84F17548389540B381D7FB24676AC5C00262363A20DADC816E171FE6A55C`. The window title is `TinixCut` and Windows reports it responding. Vite PID 23584 listens on IPv6 loopback; `http://localhost:1420/` returns HTTP 200 while `http://127.0.0.1:1420/` returns HTTP 000. These checks do not verify WebView pixels. The separate standard workspace PID 33680 and Paseo were left untouched. Computer Use is not exposed in this session.
- Native visual fallback attempt against HWND 8194758 failed: FFmpeg reported `Impossible to convert between the formats supported by the filter 'Parsed_null_0' and the filter 'auto_scale_0'`, followed by `Function not implemented (-40)` and `Could not open encoder before EOF`. A `hwdownload,format=bgra` attempt did not complete; only the three FFmpeg helper PIDs verified against this exact HWND/output were stopped. No app/Paseo process was stopped. Native rendering and installed-candidate UI remain **BLOCKED / NOT VERIFIED**.
- The standard TinixCut process and Paseo are not to be terminated as part of this work.

## Supported input probe/decode matrix — 2026-11-04

- Bundled FFmpeg 9.0.2 and the real MCP `tinixcut_media_probe` were checked against seven one-second disposable generated inputs: MP4 H.264/AAC, MOV H.264/AAC, WebM VP9/Opus, WAV PCM S16LE, MP3, PNG and JPEG/MJPEG.
- All seven MCP probes and full FFmpeg decodes passed. Per-file stream metadata, byte length and SHA-256 are recorded in [`phase57_supported_input_format_matrix.json`](../artifacts/phase57/phase57_supported_input_format_matrix.json).
- This qualifies probe/decode only. It does not prove import UI, timeline composition, playback, or export for every format, and does not claim support for other codec variants, VFR, HDR, 4K or long-form media.

## Export preflight offline-media regression — 2026-11-04

- Ran `test_p57_mcp_export_preflight_rejects_offline_media_without_mutation` through both `tinixcut_export_preflight` and `tinixcut_export_start` after removing its disposable source. Both return `EXPORT_PREFLIGHT_FAILED` with offline/missing-media detail before creating output or mutating the saved project.
- Targeted result: **1 passed, 0 failed**; full MCP integration suite: **49 passed, 0 failed, 3 ignored** (52 total; the ignored tests require a local LM Studio model). Existing-destination and invalid-output-folder behavior remains covered by `test_p57_mcp_export_preflight_is_read_only_and_reports_replacement`.
- Evidence: [`phase57_export_preflight_offline_media_evidence.json`](../artifacts/phase57/phase57_export_preflight_offline_media_evidence.json). This verifies the MCP boundary; it does not qualify the installed UI flow.


## Installed candidate MCP offline-media check — 2026-11-04

- Reinstalled the recorded NSIS candidate into a unique temporary directory, then used its installed tinixcut.exe mcp process to create a disposable project, import a generated one-second H.264 clip, attach it to the timeline, and remove the source.
- Both installed MCP tinixcut_export_preflight and tinixcut_export_start returned EXPORT_PREFLIGHT_FAILED (Source media offline_asset is offline or missing). Project revision 2 and SHA-256 remained unchanged across both calls; no output was created.
- Installed FFmpeg generated only the disposable source. The MCP process exited on stdin EOF; silent uninstall exited 0 and removed the isolated install. No TinixCut development instance or Paseo process was stopped.
- Evidence: [phase57_installed_offline_mcp_evidence.json](../artifacts/phase57/phase57_installed_offline_mcp_evidence.json). This qualifies the installed MCP boundary only; native pixels/UI remain unverified.


## Installed candidate invalid-output check — 2026-11-04

- Reinstalled the same NSIS candidate to a unique temporary directory and used its installed MCP process with a new disposable project and a destination under a nonexistent folder.
- Both tinixcut_export_preflight and tinixcut_export_start returned EXPORT_PREFLIGHT_FAILED with Export destination folder does not exist; the saved project hash was unchanged and no output was created. Silent uninstall succeeded and removed the isolated install.
- Evidence: [phase57_installed_invalid_destination_evidence.json](../artifacts/phase57/phase57_installed_invalid_destination_evidence.json). This is installed MCP coverage only; native UI and disk-full behavior remain unverified.


## Installed candidate active cancellation — 2026-11-04

- In a unique isolated install, generated a 120-second disposable H.264 source with the packaged FFmpeg, created a project and timeline clip through installed MCP, then started export.
- Waited until the render temporary file existed before cancelling. Installed MCP returned cancelled: true; final status was cancelled, the temporary file was removed, no output was promoted, and the project hash was unchanged. The MCP process exited and the isolated candidate uninstalled successfully.
- Evidence: [phase57_installed_cancel_evidence.json](../artifacts/phase57/phase57_installed_cancel_evidence.json). This verifies installed MCP job handling; it does not verify UI cancellation or disk-full behavior.


## Installed candidate successful MCP export — 2026-11-04

- In a unique isolated installation, used the installed MCP process to create a project, import a generated one-second source, place it on the timeline, and export.
- The job completed. The installed FFprobe reported H.264, 1920×1080, 30/1 fps, 30 frames and 1.0 second; the installed FFmpeg full decode exited 0. The project hash remained unchanged by export. Silent uninstall exited 0 and removed the candidate.
- Output SHA-256: 71B95DDA6D896938F74A59F83FC7AD0F2C8B5331BB0E470E67C7009B185F0B15. Evidence: [phase57_installed_mcp_export_evidence.json](../artifacts/phase57/phase57_installed_mcp_export_evidence.json).
- This verifies installed MCP export, not installed native UI, manual/AI native workflows, or upgrade/data preservation.


## Same-version reinstall and uninstall preservation — 2026-11-04

- In a unique sandboxed %APPDATA%, created a project through installed MCP and placed a valid settings.json sentinel outside the install directory. Reinstalled the same 0.1.0 candidate over its isolated install, then silently uninstalled it.
- Both install operations and uninstall exited 0. Project and settings hashes stayed unchanged; the install directory was removed, and the external data remained until the temporary sandbox was removed by the harness.
- Evidence: [phase57_same_version_reinstall_preservation_evidence.json](../artifacts/phase57/phase57_same_version_reinstall_preservation_evidence.json). This proves same-version installer preservation of external files only. Cross-version upgrade and settings written through native UI remain unverified.

### Native development-candidate export smoke — 2026-11-04

- From the running native dev candidate, imported the disposable Phase 44 video, added it to a timeline track, verified Undo/Redo, then exported MP4.
- The app completed export in 2196 ms. ffprobe verified H.264 1920×1080, 30 fps/450 frames, AAC and 15 seconds; full FFmpeg decode passed. Output SHA-256 75F9B3C73C9B25EA1FED65BD9CF9320357BBC4EA22B78D945C87FDE4AA077517.
- The expected Phase 49 output path was not honored by the native Save As control; the generated disposable export landed at artifacts/phase44/disposable/Untitled Project.mp4. No source fixture was overwritten. This is a development-candidate smoke, not installed-candidate UI qualification.
- Evidence: [Phase 49 native UI evidence](../artifacts/phase49/native_ui_disposable/phase49_native_ui_evidence.json).

## Installed candidate native launch smoke — 2026-11-04

- Verified installer SHA-256 `677158a22f34052c923ac4718d57158ba5f3ef2e4587b0705375af8b519edb4b` against the recorded 0.1.0 candidate, then silently installed into a newly created `%TEMP%` directory (exit 0).
- Launched that installed executable as PID 34112. Windows UI Automation found the TinixCut window, `Core Healthy (v0.1.0)`, and New/Open/Export controls.
- The app displayed a recovery prompt despite APPDATA/LOCALAPPDATA being overridden for this process. Only `Later` was invoked; no recovery copy was restored or discarded and no project edit was made. The isolated candidate window closed normally; silent uninstall exited 0 and removed the install directory.
- Evidence: [installed UI launch smoke](../artifacts/phase57/phase57_installed_ui_launch_evidence.json).
- Scope is launch/core-health only. The unexpected recovery prompt means this process was not sufficiently isolated for project workflows; manual/AI edit, save/reopen, recovery, export UI, cross-version upgrade and native settings preservation remain open.

### Final current-worktree regression run — 2026-11-04

- Ran `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance` with an isolated `%TEMP%\tinixcut-phase50-57-final-target` build directory: **49 passed, 0 failed, 3 ignored**. The ignored cases require a deliberately started local LM Studio server/model. Coverage includes G56 real stdio batch/checkpoint flows and G57 preflight/cancel flows.
- Ran `pnpm run typecheck`: pass; `pnpm test -- --reporter=dot`: **25 files / 150 tests passed**; `pnpm run build`: pass (existing >500 kB chunk warning); `cargo fmt -- --check`: pass; `git diff --check`: pass.
- These are automated/current-worktree checks. They do not close native UI, exact installed UI, cross-version upgrade, disk-full I/O, or independent editorial-review gates. No application source changed in this verification slice.
### Export storage-error fallback classification recheck — 2026-11-04

- Ran `cargo test --manifest-path src-tauri/Cargo.toml --lib test_phase12_hardware_error_discrimination` in the isolated final target: **1 passed, 0 failed**. It confirms `No space left on device` is treated as an I/O error and does not trigger hardware-encoder fallback.
- This is deterministic classification coverage: `No space left on device` remains an I/O error and does not trigger hardware fallback. The installed promotion-failure scenario below separately covers an output I/O failure and preservation behavior; literal disk-full behavior and native UI error presentation remain open.

## Installed candidate output-promotion I/O failure — 2026-11-04

- Installed the exact recorded 0.1.0 candidate (installer SHA-256 `677158A22F34052C923AC4718D57158BA5F3EF2E4587B0705375AF8B519EDB4B`) into a unique temp directory and used its stdio MCP endpoint with protocol metadata to render a disposable 10-second, two-source project copy.
- Held a pre-existing output file open with exclusive sharing. Preflight and export start succeeded; rendering reached atomic promotion, then the job reported `IO error during export promotion: Failed to promote temporary render file to destination: Access is denied. (os error 5)` and status `failed`.
- The pre-existing output sentinel and project-copy hashes were unchanged; the original project and media sources were untouched; no render temporary file remained. The isolated installed candidate uninstalled with exit 0 and its temp root was removed.
- Evidence: [installed output-promotion failure](../artifacts/phase57/phase57_installed_output_promotion_failure_evidence.json). This qualifies an installed destination write/promotion failure and preservation behavior. It does not simulate a full disk or qualify native UI error presentation.

## Final blocker reconciliation — 2026-11-04

Automated installed-candidate evidence remains as listed above; no application code or installed runtime was changed in this pass. G57 remains **OPEN**. The unresolved acceptance items are grouped for the final qualification pass:

- **Installed native workflows/UI:** render pixels, run manual and AI-draft flows, review installed MCP outputs, verify save/reopen/recovery, and observe missing-media/cancel/error presentation on the exact installer candidate. Process startup/Core Healthy and MCP export do not prove these UI claims.
- **Cross-version upgrade/data retention:** blocked because only authentic TinixCut 0.1.0 installers are available; a prior-version installer is required. Same-version reinstall preservation passed and is not a substitute.
- **Literal disk-full behavior:** current-source error classification and installed output-promotion access-denied/preservation pass; no isolated disk-full test volume was available. Do not claim literal disk-full UX from those results.
- **Native export artifact review:** three Phase 56 outputs have SHA-256, ffprobe and full-decode evidence and persistent copies. An attempted import into a blank project failed because SendKeys corrupted the path; the dialog was closed and the blank project remained empty. See `artifacts/phase56/phase56_persistent_export_verification.json`.

Optional physical A/V calibration, clean VM and installed hardware encoder remain out of the required feature-completion scope unless public release claims depend on them.

## Installed native manual and AI workflow qualification — 2026-11-04

- Reinstalled the exact recorded NSIS candidate (0.1.0; SHA-256 `677158A22F34052C923AC4718D57158BA5F3EF2E4587B0705375AF8B519EDB4B`) into a unique temporary directory. Installed process PID 12172 reached Core Healthy. The global recovery prompt was handled with Later; no recovery project was restored or discarded.
- Manual workflow: imported the verified disposable green MP4, placed it on Video Track 1, confirmed Undo and Redo, and visually observed the native timeline preview. The installed UI exported a 1-second 1920×1080 H.264/AAC MP4; ffprobe and full FFmpeg decode both exited 0. Export took 647 ms. Evidence screenshot: `../artifacts/phase57/native_ui/phase57_installed_manual_timeline.png`.
- AI workflow: local `google/gemma-4-e4b` analyzed a disposable 15-second Hoi An clip for “Hoi An street scenes with people and lanterns”, returning suggestions at 00:02, 00:07 and 00:12. Review showed three scenes/15.00 s and validated operations without changing the project. Apply created `Scene Draft` (rev6); Undo removed it; Redo restored it (rev8). Native timeline frame matched the FFmpeg-decoded source content. Screenshot: `../artifacts/phase57/native_ui/phase57_installed_ai_draft_applied.png`.
- The test project remains unsaved. Save opened the native dialog, but exact absolute-path keyboard input read back as an empty filename; I closed the dialog without saving. Save/reopen/recovery UI acceptance remains BLOCKED in this input session. The candidate remains installed/running for inspection.
- Installed candidate picker did not list `.m4a`; it predates the current development candidate's media-filter fix. Do not treat it as qualification of that latest source change.
- Machine-readable evidence: [`phase57_installed_native_workflow_evidence.json`](../artifacts/phase57/phase57_installed_native_workflow_evidence.json).
### Installed missing-media UI probe — 2026-11-04

- Temporarily renamed only the disposable 15-second AI fixture, invoked Preview on its exact Media Library row, and restored the source in a `finally` block. Source Viewer opened; UI Automation exposed no offline/error text. Pixels were not captured while the source was unavailable, so this result is **inconclusive**, not a missing-media UI pass. The original Phase 44 fixture remained untouched.
- Evidence details: `../artifacts/phase57/phase57_installed_native_workflow_evidence.json`.
### G57 continuation checkpoint — 2026-11-05

- Re-ran the export hardware-error discrimination regression: **1 passed, 0 failed**. `No space left on device` remains classified as an I/O error and does not trigger hardware-encoder fallback.
- This is classification coverage, not a literal ENOSPC write or installed UI error-presentation test. G57 remains OPEN for installed Save/reopen/recovery and missing-media/error UI checks; cross-version upgrade needs an authentic prior-version installer, and literal disk-full UX needs a disposable quota/volume or equivalent deterministic ENOSPC harness.
- Existing installed manual/AI/export evidence remains valid; no installer or runtime was changed in this checkpoint.
### Prior-version installer availability audit — 2026-11-05

- Inspected repository release metadata: `git tag --list` returns no tags. The three available historical source commits (`1e5147f`, `20e1aea`, `d07073b`) each declare TinixCut **0.1.0** in `src-tauri/Cargo.toml`; none is evidence of a prior released product version.
- No TinixCut installer executable is present under `artifacts/phase57/` or the current user's Downloads directory. Therefore a genuine older installer cannot be assembled from current local evidence; changing version metadata on an old commit would not establish an authentic prior release.
- Cross-version upgrade/data retention remains BLOCKED pending an authentic older-version installer. Same-version reinstall evidence remains valid only for same-version preservation.
### Simulated disk-full error guidance — 2026-11-05

- FFmpeg failures containing `No space left on device`, `not enough space`, or `disk is full` now receive an actionable export message: free disk space and retry. The original FFmpeg encoder and stderr detail are retained; other render failures keep the existing generic render message.
- Added `test_phase57_disk_full_error_is_actionable_and_keeps_encoder_detail`; it passed. The existing encoder-fallback discrimination regression also passed, proving the storage failure does not switch hardware encoders.
- This injects representative FFmpeg stderr into the error formatter; it is not a real ENOSPC filesystem write. Literal full-volume behavior and installed-candidate UI presentation remain open.
- Validation: focused tests passed 1/1 each; `cargo check --bin tinixcut`, `cargo fmt -- --check` and `git diff --check` passed.
- Runtime restart: exact workspace Tauri app PID 33680 and its Cargo parent were replaced by `pnpm tauri dev`; latest workspace TinixCut PID is 39364 and Vite PID 4292 serves localhost:1420 with HTTP 200. Startup log still reports `Cannot read properties of undefined (reading 'invoke')` for `getHealth`/preview IPC. Computer Use was unavailable, so native window health/rendering was not visually verified. Isolated candidate PID 37972, installed candidate PID 12172 and Paseo were preserved.
### G57 gate-scope reconciliation — 2026-11-05

The Product Owner approved treating literal full-volume ENOSPC as optional unless TinixCut claims literal disk-full behavior. The existing isolated output-promotion I/O failure and no-space classification tests satisfy simulated software-failure coverage. Therefore old statements listing a literal full-volume run as a required G57 gate are superseded. Installed native Save/reopen/recovery/error UI and an authentic previous-version installer remain open.

### Installed-candidate UIA observation — 2026-11-05

After the workspace runtime restart, read-only UI Automation against isolated installed candidate PID 12172 (executable under the phase57 temp install) exposes only the TinixCut title bar and no client controls/Core Healthy badge. No UI input, restart, or uninstall was attempted. Keep the candidate preserved and G57 OPEN; see the consolidated final qualification blockers.
### G57 latest-source installed native UI recheck — 2026-11-05

- Built and installed the current worktree candidate in an isolated `%TEMP%` directory. UI Automation exposed the actual TinixCut editor, a `Core Healthy (v0.1.0)` status, and the recovery prompt; this supersedes older title-bar-only observations for this fresh candidate. Invoked **Later** once and verified all 11 recovery snapshot hashes remained unchanged.
- On a new disposable project, added Track 1, verified Undo removed it and Redo restored it. Saved through TinixCut's native Save dialog to `artifacts/phase57/disposable/phase57_latest_candidate_native_project.tinix` (project `proj_7efb5a014a6246df9368a1b263a2f582`, schema 17, revision 3, one track). The file is 1,544 bytes with SHA-256 `2D5635EEE0723F597490243437816024CEDFD975B1B49F4BFD4108FE2565F2ED`.
- Reopened that exact saved fixture through the native Open dialog. The dialog closed, TinixCut remained responsive, and Track 1 was present after load. This passes basic installed recovery-list preservation and native project save/reopen for the latest-source candidate. It does not qualify missing-media/error/cancellation UI or cross-version upgrade.
- Machine evidence: `artifacts/phase57/phase57_latest_candidate_native_ui_evidence.json`. The candidate process remains running; the standard workspace runtime and Paseo were not stopped or restarted.
### G57 installed offline-media warning and relink — 2026-11-05

- Opened an isolated current-source project whose media path intentionally does not exist. Native UI Automation showed **Media offline**, the exact last-known path, and **Relink…**. No sample project/media outside `artifacts/phase57/disposable/` was changed.
- Relinked to the separately copied, hash-verified media fixture through TinixCut's native picker. UIA reported `Relinked phase57_intentionally_missing_source.mp4`; the offline warning and Relink button disappeared and Preview returned. Saved the disposable project at revision 1, verified its asset path points to the existing fixture, then Preview opened **Source viewer** without an access/error message. Original Phase 54 project and media hashes remained unchanged.
- This qualifies the installed missing-media warning, recovery action and media preview after relink on the current-source package. It does not qualify export cancellation/failure presentation or cross-version upgrade.
- Evidence: `artifacts/phase57/phase57_latest_candidate_native_ui_evidence.json`.
### Latest-source NSIS export cancellation — 2026-11-05

- Built a new NSIS installer from the current working tree (still dirty from the multi-phase implementation) using an isolated Cargo target. Installer: `TinixCut_0.1.0_x64-setup.exe`, 59.26 MiB, SHA-256 `47BBFA3F036180A962F49C0078AA2CB9BB2159A6B43B75C431ECCBD3313AF9CE`. Installed into `%TEMP%\tinixcut-phase57-final-installed-47bbfa3f036180a9`; isolated `LOCALAPPDATA` was used. Candidate PID 33692 exposed native UIA controls and `Core Healthy`.
- Installed candidate exported a 120 s disposable project to H.264 1920×1080/30 fps MP4 (41,794,857 bytes, SHA-256 `C6F7D33A53FD8CB7F379571C6F9586C70320A1DFEFC8CBF8FA69D4A57D3D3308`); FFprobe passed and full FFmpeg decode exited 0.
- On the same installed candidate, a separate disposable project included 120 s and 600 s clips. At 60.3% progress, the exact candidate window was brought above the foreground Terminal; `WindowFromPoint` confirmed the Cancel control’s click location belonged to that TinixCut HWND. One native click produced `Export cancelled; temporary output was removed.` The requested final MP4 and render temporary were both absent afterward; candidate remained Core Healthy. A preceding attempted click was correctly withheld when Terminal occluded the candidate, and that earlier export completed normally; it is not counted as cancellation evidence.
- The same cancellation path passed on the workspace dev runtime at 69.1%; target and temporary output absent. Machine-readable evidence: [`phase57_current_source_export_cancel_evidence.json`](../artifacts/phase57/phase57_current_source_export_cancel_evidence.json).
- G57 remains OPEN: the newest exact installer has not yet repeated all manual, AI-draft and MCP batch scenarios; a genuine previous-version installer is unavailable, so cross-version upgrade preservation cannot be demonstrated. G54/G55 remain independent final-pass gates.
### G57 latest-source AI draft and MCP export — 2026-11-05

A corrected current-source NSIS candidate was built from the dirty worktree, installed to a new isolated directory, and launched as PID 27424. Installer SHA-256: `E0C686E088000B8D5587633011A4E3F96AF50901CD84D58B1512E1485B46801D`; app SHA-256: `9EF90BC3CB840B4DE7D1540A44C6BB4A2E9656DD01A8EE5D3587E30DF5A667CF`. It reached Core Healthy.

- Fixed an incorrect deterministic “mock provider” warning leaking into LM Studio receipts; regression test `local_planner_receipt_does_not_keep_mock_provider_warning` passes.
- Native local-AI review used Gemma 4 E4B and showed a validated 3-operation draft. The false mock warning was absent; the accurate notice states the model received metadata only and did not inspect media bytes or scenes. Apply, Undo and Redo behaved as expected.
- Native Save produced a separate disposable project (project ID `proj_ae791cd4d3244453a51e10d4ec37479f`, revision 5, two tracks); the exact file reopened with stable SHA-256 `183D25761376196F19F5C8F7D7CE1B1B6BDF0BFABF058ECF01F434B77BAD71CC`.
- Native export and MCP export both produced H.264/AAC 1080×1920 at 30 fps, 450 video frames, 15 seconds. FFprobe and full FFmpeg decode passed for both. MCP preflight passed, job reached `completed` at 100%, and the project copy hash remained unchanged.
- Exact machine evidence: [`phase57_latest_candidate_ai_workflow_evidence.json`](../artifacts/phase57/phase57_latest_candidate_ai_workflow_evidence.json).

This closes exact-candidate native manual/AI/save-reopen/export and MCP AI/export smoke coverage. G57 remains OPEN for native installed export-error presentation and cross-version data preservation; the latter needs an authentic earlier-version installer, which is not available. G54 and G55 remain independent blockers.

### Latest-candidate native export-error UI — 2026-11-05

- On the exact latest installed candidate (installer SHA-256 `E0C686E088000B8D5587633011A4E3F96AF50901CD84D58B1512E1485B46801D`, PID 27424), exported through the native UI to a task-created sentinel file held with exclusive sharing. The overwrite confirmation named `phase57_native_locked_destination_20261105.mp4`; the app rendered the disposable project, then showed `Export error: IO error during export promotion: Failed to promote temporary render file to destination: Access is denied. (os error 5)` and remained Core Healthy.
- After releasing the test lock, the sentinel SHA-256 remained `BD86E40873E5A38F793E94A0105A29D169EEB7E0EAD738348C47C8C4EE61ADF0` (38 bytes); the project SHA-256 remained `183D25761376196F19F5C8F7D7CE1B1B6BDF0BFABF058ECF01F434B77BAD71CC`; no render temporary remained. This qualifies latest-candidate native error presentation and preservation for a promotion/access-denied failure, not literal ENOSPC.
- Exact machine evidence: `../artifacts/phase57/phase57_latest_candidate_native_export_error_ui_20261105.json`.
- **G57 remains OPEN only for cross-version upgrade/data preservation** against an authentic older-version installer; none is available locally. Literal ENOSPC remains optional per the Product Owner’s decision.

### G57 rebuilt NSIS candidate on current source — 2026-11-12

- `pnpm tauri build` produced `TinixCut_0.1.0_x64-setup.exe` (59.29 MiB, SHA-256 prefix `2f3eed435f06a78`) from the source including this session's changes (preview proxy, focus fix, waveform cache, animated-scale export fix).
- Silent install into a temp directory (exit 0): the installed CLI rendered the G52 fixture (animated scale/position, text, rich audio) to 1920×1080 H.264 + AAC, 450 frames; full FFmpeg decode passes and frame 150 is pixel-identical (mean abs diff 0.0) to the dev-build export. The installed GUI launched and stayed responsive; the window was not inspected through UIA. Silent uninstall exited 0 and removed the directory.
- Not re-run on this candidate: installed native UI workflows (open/edit/save/export), recovery, relink, cancellation. Cross-version upgrade remains **BLOCKED** (no authentic prior installer). **G57 stays OPEN.**
