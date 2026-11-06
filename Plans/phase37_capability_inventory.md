# Phase 37 — Capability inventory and reference workflow

**Status:** COMPLETE — capability inventory, reference fixture/workflow, persistence ownership boundary and MCP/native evidence are recorded. Unclosed playback and operation-parity gaps have explicit owners in Phase 38/39; they are not counted as PASS. Release-environment qualification is deferred to Phase 43.
**Date:** 2026-11-03
**Roadmap:** `Plans/plan_editor_mcp_ai_roadmap.md`
**Scope:** Source inventory, capability-report correction, and disposable MCP workflow qualification. Native GUI reopen and cross-process concurrency have now been retested on a disposable copy.

## Current qualification checkpoint — 2026-11-03

This summary takes precedence over the initial inventory and historical checkpoints below. Each PASS is limited to the stated path and fixture.

| Workflow area | Current evidence | Remaining gap |
|---|---|---|
| Bootstrap and import | Native UI can open a project; MCP `AssetImport` registered caller-provided metadata in the reference workflow. | No dedicated MCP project-create or media-probe tool was identified; a fully headless bootstrap/probe workflow remains unqualified. |
| Timeline editing | MCP move/trim/split and speed operations passed. UI speed edits were saved; at timeline frame 30, preview was visually compared with the exports and exported Half/Normal/Double matched source frames 15/30/60. | This sample does not establish playback or boundary parity; broader native edit parity remains open. The WebView Speed popup still has a mouse-routing error; keyboard navigation worked. |
| Canvas, text and export | The 9:16 workflow exports H.264/AAC at 1080×1920/30 fps. Native UI title edit saved as rev21 and appeared in the MCP-rendered one-second frame. | Broader typography/layout and preview/export sampling remain open. |
| Audio and transitions | Native UI→save→MCP export verified 30f and 15f two-source CrossDissolves; native audio fade-in 30f/out 60f persisted and matched the measured MP4 envelope. | Audio-fade preview, exhaustive transition types/boundaries and physical A/V remain open. |
| Split audio and fades | `ClipSplit` retains fade-in only on the left fragment and fade-out only on the right. Repeated references to one media stream are explicitly `asplit`; the shared-WAV two-fragment project with fades exports all 1057 frames and passes full decode. | Regression is verified on the saved frame-187 fixture; broader audio composition and preview/export parity remain open. |
| Persistence and concurrency | MCP→UI→save→MCP track-edit round trip passed. Stale UI save reported a conflict and retained the pending edit. Separate-process stale-writer tests passed. | Broader operation parity does not follow from the track-edit example. |
| Export lifecycle | Reference export, ffprobe and full decode passed; acceptance tests cover replay, failed transaction, cancellation and cleanup. | Phase 37 has no export-lifecycle blocker. Clean install and packaged encoder checks are deferred to a release candidate (Phase 43), if distributing an installer. |
| Physical A/V | Diagnostic estimates exist but do not establish a physical-output synchronization claim. | No Phase 37 blocker. Require calibrated shared-clock measurement only before publishing a quantified physical A/V tolerance; otherwise label the claim unqualified. |

The numbered checkpoint entries below retain earlier observations, including failures subsequently fixed; use their dates and the later reconciliation entries when reviewing the history.

### Phase 37 shared-input split-audio fix — 2026-11-03

- Root cause isolated with a path control: two fragments referencing the same WAV input caused FFmpeg to fail at frame 187 with `Error sending frames to consumers: No space left on device` (`-28`); the same media copied to a distinct input path succeeded, including with fades.
- `CompositionPlan::to_ffmpeg_export_args_with_overlays` now emits deterministic `asplit` branches for each audible audio layer sharing one input stream, then feeds each branch through its own trim/speed/gain/fade chain. Single-use streams retain the direct-input path; muted/inaudible layers are excluded.
- Regression assertion in `test_zero_gain_audio_stream_does_not_attenuate_mix` verifies `[1:a]asplit=2[audio_src_1][audio_src_2];` and both branch consumers. The focused test passed (1/1); `cargo test --lib split` passed (6/6).
- Current workspace CLI rendered `phase37_native_split_fixed_disposable.tinix` with both outer-edge fades to `phase37_native_split_asplit_fades.mp4`; render exit 0, H.264/AAC, 1920×1080, 30 fps, 1057 video frames, 48 kHz mono, 35.233333 s. FFprobe exit 0 and full FFmpeg `-xerror` decode exit 0. Evidence: `phase37-native-split-asplit-ffprobe.json` and MP4 under `artifacts/phase37/reference_fixture/`.
- Runtime: after final formatting, Tauri dev rebuilt the workspace binary (binary write 11:30:58, source write 11:30:44) and launched TinixCut PID 43724 at 11:30:59. `localhost:1420` returns HTTP 200; Computer Use identifies TinixCut Core Healthy v0.1.0. The app currently has an empty untitled project; no project UI state was changed.

**Result:** PASS for split-audio export with shared WAV input and fades on this fixture. The Phase 37 inventory gate is complete; broader parity/playback acceptance is transferred to Phase 38/39.

### Phase 37 split-audio export regression — 2026-11-03

- Native UI split the disposable Music bed at timeline frame 187 in revision 28 and saved it. Persisted fragments are contiguous: left timeline `[157,187)` / source `[0,30)` frames; right timeline `[187,1057)` / source `[1,30)` seconds. The original sample remains unchanged.
- Before the code change, both fragments inherited the original 30-frame fade-in and 60-frame fade-out. `src-tauri/src/modules/command/mod.rs` now preserves the original outer edges only: left keeps fade-in and clears fade-out; right clears fade-in and keeps fade-out. The new `splitting_audio_keeps_fades_only_on_the_original_outer_edges` test passes; all six tests matched by `cargo test --lib split` passed.
- Export failure artifact `native-ui-split-export-failure.json` records FFmpeg `Error sending frames to consumers: No space left on device`, task error `-28`, and termination at frame 187. D: had over 638 GB free; this is an FFmpeg graph error, not a full drive.
- Controlled MCP export probes on isolated copies: muted audio = PASS; an initial un-faded split export completed as H.264/AAC, 1920×1080, 30 fps, 1057 frames / 35.233333 s and passed ffprobe/full `ffmpeg -xerror` decode. Preserving either fade-in on the left fragment or fade-out on the right fragment independently reproduced `-28` at frame 187. A subsequent no-fade replay after the experimental build/revert also failed at frame 187. Treat the earlier no-fade PASS as non-repeatable; the failure is at the split-audio composition boundary and is not proven to require fades. D: remained over 638 GB free.
- The diagnostic hypothesis `asetpts=N/SR/TB` after `adelay` was tested, failed both fade and no-fade exports, and was reverted. Focused tests after the revert passed: six `split` tests and 11 `audio` tests.
- Evidence: `native-split-no-fades-ffprobe.json` (initial successful run), `native-split-reverted-no-fade-export.json` (failed replay), `native-split-asetpts-export-retry.json` (rejected experiment), `native-ui-split-fixed-export-mcp.json`, and `native-ui-split-export-failure.json` under `artifacts/phase37/reference_fixture/`.
- Runtime: Tauri dev rebuilt/replaced the native process after the final revert (PID 39760, started 11:25:18). TinixCut Computer Use screenshot/accessibility reports Core Healthy v0.1.0; `localhost:1420` returns HTTP 200. No Paseo or unrelated process was stopped.

**Historical result at this checkpoint:** PASS for split command fade ownership; split-audio export was unstable at frame 187. This finding was resolved by the later explicit `asplit` fix documented above. Remaining manual parity/playback gaps are transferred to Phase 38/39 and do not keep the Phase 37 inventory gate open.

## Acceptance workflow under review

Reference target is a 30-second, 9:16, 30 fps social video with two distinct video sources, music, title and subtitles, saved as a normal editable project and exported to H.264/AAC MP4.

The workflow is decomposed into: create/open project → import/probe media → add and position clips → trim/split/move → set canvas → add title/subtitle → add and mix audio → preview/inspect → save/reopen → export/progress/cancel → verify streams and decode.

## Existing MCP surface

Inventory source: `src-tauri/src/modules/automation/mcp.rs` tool router. The following names were present in source on the inventory date:

| Tool | Source-level role | Workflow coverage / evidence state |
|---|---|---|
| `tinixcut_inspect` | Read project metadata/timeline | Source confirmed; runtime E2E against reference fixture remains |
| `tinixcut_capabilities` | Enumerate operations and export support | Source confirmed; compare report with actual UI/backend behavior |
| `tinixcut_dry_run` | Validate proposed `CommandEnvelope` without disk mutation | Source confirmed; failure and diff review remain in E2E |
| `tinixcut_apply` | Apply transaction and persist | Source confirmed; exact operation coverage listed in Command Service below |
| `tinixcut_save` | Save/save-as existing disk project | Source confirmed; clarify difference from apply's atomic persistence and test reopen |
| `tinixcut_export_start` | Start asynchronous export job | Source confirmed; runtime job and output verification remain |
| `tinixcut_export_status` | Query job status/progress | Source confirmed; runtime lifecycle remains |
| `tinixcut_export_cancel` | Cancel active export | Source confirmed; cancellation/cleanup test remains |
| `tinixcut_template_validate` | Validate typed `TemplateDraft` | Source confirmed; outside first video workflow unless title uses a generated template |
| `tinixcut_template_preview` | Render a template draft frame | Source confirmed; this is template preview, not proof of whole-project preview control |
| `tinixcut_template_apply` | Apply template draft through supported path | Source confirmed; E2E project/editability remains |
| `tinixcut_template_save` | Save template draft | Source confirmed; not needed for baseline video creation |

**Initial coverage gap:** source inventory shows no separately named generic media-import, project-create, timeline-edit, caption, or whole-project-preview MCP tool. Some are expressed through `tinixcut_apply` and its `CommandEnvelope`; confirm schema/capabilities and whether file/probe setup can be performed safely without UI before calling them supported. No new MCP tool should be added until this mapping is proven.

## Command Service source inventory

`src-tauri/src/modules/command/mod.rs` defines a tagged `CommandOperation` and describes the Command Service as the authoritative mutation path. Source variants inspected include:

- Project/timeline: `TrackCreate`, `TrackDelete`, `ClipCreate`, `ClipMove`, `ClipTrim`, `ClipSplit`, `ClipDelete`, `ClipDuplicate`, `TrackReorder`.
- Media: `AssetImport`, `AssetRelink`.
- Canvas/composition: `ProjectSettingsSet`, `CanvasBackgroundSet`, clip transform/crop/placement/opacity and speed operations.
- Text/audio: `ClipTextSet`, text style/template operations, `ClipVolumeSet`, `ClipAudioFadeSet`, `AudioCrossfadeCreate`.
- Effects/transitions/animation: `CutTransitionCreate`, visual-effect and keyframe operations.

The enum establishes that command types exist in source; it does not prove each operation has a complete GUI affordance, MCP JSON schema discoverability, working export semantics, or native qualification. `AutomationAdapter::supported_operations()` is the MCP/CLI discovery source. Comparing its 53 advertised names with the 58 `CommandOperation` variants found five omissions: `AssetRelink`, `ClipSpeedSet`, `ClipVisualEffectSet`, `TextTrackStyleSet`, and `TextTrackTemplateApply`. These were added to the report in this task; the existing automation acceptance test now asserts all five remain discoverable. This is a capability-report correction, not proof that all five work end to end over MCP.

`AssetImport` registers metadata supplied by the caller; the inspected command handler does not probe media itself. The caller therefore needs trustworthy duration/dimensions/file size, or a separate supported probing path, before creating clips. The inspected MCP router has no standalone project-create/open or general project-preview tool. Project-scoped operations address an existing project path; creation/bootstrap and preview require a documented CLI/UI flow or a future capability decision.

## Workflow matrix — initial source inventory

| Step | Current evidence found | Initial assessment | Next evidence needed |
|---|---|---|---|
| Create/open project | Native UI exposes New/Open; MCP tools require `projectPath` | No dedicated create/open MCP tool found in router; bootstrap not established by this source audit | Determine whether CLI/API can safely bootstrap; otherwise document UI bootstrap prerequisite |
| Import/probe media | `AssetImport`/`AssetRelink` command variants; `AssetImport` is in capabilities after correction | `AssetImport` registers caller-provided metadata; no probe in the inspected handler; no standalone import tool | Inspect UI probe path and CLI/API options; compare metadata and unsupported/missing stream handling |
| Add/move/trim/split clips | Command variants listed above; UI timeline exists | Source capability confirmed, parity unverified | Compare same edit via UI and MCP at equivalent revisions |
| Canvas/format | `ProjectSettingsSet`; UI canvas controls in native history | Source/UI evidence exists, exact workflow export still needs E2E | Set 9:16 and verify persisted dimensions/export |
| Title/subtitle | Text commands/templates and caption phases exist | Basic commands exist; speech-to-text requires separately configured local Whisper | Use fixed SRT/title fixture first; test automatic captions separately |
| Music/audio mix | Volume/fade/crossfade command variants exist | Source capability exists; preview native qualification is distinct from export | Verify source range, gain, export stream and native preview |
| Preview | Native app preview exists; MCP template preview is template-only | No general MCP project-preview tool identified | Decide if frame-render API exists elsewhere or make this a documented UI review step |
| Save/reopen | `tinixcut_apply` atomically persists; `tinixcut_save` exists | Source contract present; round trip in E2E needed | Open applied project in UI and compare revision/semantic state |
| Export/job | Start/status/cancel MCP tools and export engine are present | Source surface present; subprocess/job lifetime caveat in ADR-005 | Full export, query/cancel, process exit and output decode on fixture |
| Concurrent UI/MCP writes | ADR-005 documents per-process MCP write serialization and revision/idempotency | No cross-process lock is established by that fact; concurrency behavior is a safety gap | Reproduce stale UI/MCP edits; specify reject/refresh/exclusive ownership before shared editing |

## Candidate disposable fixture

The working tree contains `artifacts/media-samples/hoi-an-morning/` with existing candidate media and related output, including `hoi-an-one-day.mp4`, `ambient-bed.wav`, `hoi-an-story.vi.srt`, `narration.vi.txt`, `.tinix` projects, exports, and Phase 26 test assets. These filenames establish local artifact availability only. This pass did not verify provenance/usage rights, media stream details, matching editability, required length, fixture determinism, or expected rendered result. Do not treat them as an approved reference fixture or overwrite them.

Phase 37 should select or create an isolated fixture copy, record hashes, use rights/source, ffprobe stream metadata, duration, exact expected clip ranges and output acceptance, then preserve it read-only during runs. If the selected sources do not contain two distinguishable shots of adequate duration, create a deterministic generated test fixture instead of using repeated frames as visual evidence.

## Safety and release gates

- `tinixcut_apply` has durable per-request transaction semantics in ADR-005; current docs say the write gate is process-local. Do not infer mutual exclusion with the GUI or another MCP process. Prove how atomic file replacement and revision checks behave under concurrent edits before promising shared-project AI editing.
- AI must receive only declared capabilities and project/media facts. It must produce typed edit-plan data; validate and dry-run it, then apply through the existing Command Service. No model-supplied shell commands or direct project JSON reducer.
- Gate disposition (2026-11-03): these checks do not block editor/MCP/AI development or Phase 37. Physical A/V measurement is conditional on making a quantified physical-sync claim. Clean Windows VM and packaged-encoder qualification are release-candidate checks only when shipping an installer; defer them to Phase 43. Do not label either claim or release check PASS without its evidence.

## Work remaining to close Phase 37

1. Verify native UI→save→MCP readback for remaining edit families, especially clip move/trim/split, text style, canvas settings, and media relink/import where UI affordances exist. Keep each test on a disposable fixture and compare normalized project semantics.
2. Consolidate the independently passing native/MCP slices into one reproducible 30-second reference workflow: open/import, edit, save/reopen, export, job lifecycle, ffprobe and full decode. Record any steps that cannot be headless because MCP lacks a project-create or media-probe capability.
3. Complete sampled preview/export checks for representative transitions and audio fades; distinguish qualitative UI screenshot checks from numeric export analysis. Test continuous playback/frame drops only with a calibrated capture method.
4. Keep the automated persistence, replay/idempotency, failed transaction, cancellation/cleanup and native stale-write/conflict evidence linked. Do not repeat already passing cases unless code changes.
5. Update Phase 38–39 priorities from measured gaps and close Phase 37 only when its acceptance workflow is demonstrably complete. Phase 26 physical A/V, clean-VM and installed-encoder gates remain separately OPEN.
## 2026-11-02 MCP reference workflow checkpoint

The synthetic fixture workflow was executed against the workspace TinixCut binary over MCP stdio using a disposable copy under `artifacts/phase37/reference_fixture/`. The earlier capability-only smoke (12 tools and the five corrected operations) is still distinct from this workflow run.

| Check | Result | Evidence |
|---|---|---|
| Fixture edit plan dry-run | PASS; 18 operations validated before applying | `edit-plan.json`, `dry-run.json` |
| Transaction apply | PASS; revision 6 → 7, 4 tracks and 6 clips in the 30 s / 9:16 project | `apply-receipt.json`, `phase37_social_cut.tinix` |
| Export job | PASS; job `export_2996c128723743a68516641c53d52996` completed at progress 1.0 | `export-start.json`, `export-final-status.json` |
| MP4 streams | PASS; H.264 1080×1920 at 30 fps / 900 frames and AAC 48 kHz; 30.000 s | `ffprobe.json` |
| Full decode and visual samples | PASS per completed FFmpeg decode; representative frames extracted at 1.0, 4.5, 16.5 and 29.9 s | `frame-01s.png`, `frame-04_5s.png`, `frame-16_5s.png`, `frame-29_9s.png` |
| Disposable project hash | Recorded: `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801` | SHA-256 of `phase37_social_cut.tinix` |
| Export hash | Recorded: `865303CC98F33F7582715ED01F9A5909B1A989044244B2C18BF2CCF5147739FE` | SHA-256 of `phase37_social_cut.mp4` |
| Native UI reopen/readback | PASS, visual evidence | Computer Use HWND 45423108 showed Core Healthy v0.1.0, opened the disposable fixture at revision 7, displayed four media assets and 9:16 preview content; playback advanced. Accessibility tree was unavailable, so this is visual-only evidence. |

A previous-session report that native UI open was blocked by file-picker input routing is superseded by the successful visual reopen/readback recorded above. Computer Use browser access host remains unavailable, but native window observation worked.

### UI/MCP cross-process write qualification — 2026-11-03

- Test fixture: `phase37_ui_mcp_concurrency_disposable.tinix`, a byte-identical copy of `phase37_social_cut.tinix` before the test (SHA-256 `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`, revision 7). A separate `.baseline-copy` preserves the starting file. The original reference project retained the same hash throughout.
- MCP subprocess `tinixcut mcp` applied one `TrackCreate` at base revision 7; receipt reported `priorRevision=7`, `newRevision=8`, `operationsApplied=1`, `isError=false`, process exit 0. Disk then contained `MCP concurrent track`. Response evidence: `ui-mcp-concurrency-mcp-response.json`.
- The already-open UI still held revision 7. Clicking `+ Track` created local `Track 5`; UI showed revision 8 and `Add track uncommitted`, without a stale-revision conflict or refreshing the MCP track. Clicking Save reported atomic save to the disposable path.
- Disk then showed revision 8 with 5 tracks: the original 4 plus `Track 5`; `MCP concurrent track` was absent. This reproduces a **silent lost update**: independent writers both accepted revision 7 and produced revision 8; the later UI save replaced the MCP result without conflict detection. Final test-copy SHA-256 `81421252F564ED431AF13AF9161984CD0E292CD19FC9A1754A2AAB6546285EEE`. The untouched baseline copy and original fixture both retain the starting hash.
- **Result: FAIL / high-severity data-integrity gap.** Do not allow UI and separate MCP processes to edit/save one project concurrently until cross-process ownership/locking or disk-revision compare-and-swap prevents stale writes and UI handles external changes/unsaved edits. MCP's process-local `write_gate` and in-process AC24 do not cover this boundary.

These results complete MCP dry-run/apply/export/readback and native UI open/preview on a disposable fixture, but Phase 37 is not complete because shared UI/MCP writing failed. Standalone stale-revision/replay/failure/cancel cases also remain unqualified. Phase 26 A/V, clean-VM, and installed-encoder gates remain open as recorded in `Plans/phase26_manual_qualification.md`.

### Cross-process write guard implementation and retest — 2026-11-03

- Added a persistence-level compare-and-save API. It takes an OS lock on a stable sidecar derived from the canonical project path, checks the on-disk revision while holding that lock, and keeps the lock through the atomic replacement. A stale snapshot returns `PROJECT_REVISION_CONFLICT` and does not replace the newer file.
- Native UI and automation sessions retain their last persisted revision independently of the in-memory command revision. MCP apply/save and agent-template apply use the guarded path; MCP conflict responses include `expectedRevision` and `actualRevision` details. Save As to a canonical alias of the open file still uses the original persisted revision.
- Persistence unit tests passed: stale-writer rejection preserves the newer project; two simultaneous writers from separate file handles yield one success and one conflict.
- Cross-process integration test passed: `src-tauri/tests/phase37_cross_process_persistence.rs` starts a child test process with a stale revision-0 expectation after the parent writes revision 1. The child receives `RevisionConflict { expected: Some(0), actual: Some(1) }`; the final project remains revision 1 with the newer `MCP update` content.
- MCP acceptance tests passed: `cargo test --test phase15_mcp_acceptance test_p15_ac` (25 passed, 0 failed; 2 filtered).
- Runtime build/restart passed after the final Rust change: `cargo build --manifest-path src-tauri/Cargo.toml` succeeded; restarted only the workspace TinixCut `pnpm tauri dev` process tree after verifying its executable/command-line ancestry. App PID 42440 started at 2026-11-03 00:56:38 from binary last written at 00:55:23; workspace Vite PID 69360 serves HTTP 200 on port 1420, and a read-only real MCP stdio `initialize` + `tools/list` smoke exited 0 with 12 tools and empty stderr. Paseo was not stopped.
- **Native UI retest: BLOCKED.** Computer Use listed TinixCut windows `184951336`, `35918644`, and `71768400` across refreshes/restarts, but returned `Accessibility state unavailable for window 184951336`, `35918644`, and again for `71768400`; one refresh between observations omitted the window. No UI input was sent. The original UI/MCP failure remains the historical manual reproduction; native unsaved-edit retention and visible conflict messaging still need a controlled UI retest.
- The reference project and baseline fixture were not modified. Phase 37 remains OPEN for native UI retest and remaining replay/idempotency, failure, cancellation, and cleanup cases. Phase 26 A/V, clean-VM, and installed-encoder gates remain OPEN.

### Phase 37 native UI/MCP stale-write guard retest — 2026-11-03

- Computer Use recovered successfully after a fresh session. TinixCut HWND 110764890 showed Core Healthy v0.1.0; its Open dialog loaded `phase37_ui_mcp_concurrency_guard_disposable.tinix` at revision 7 with four tracks and media in the library. The disposable copy began byte-identical to `phase37_social_cut.tinix` (SHA-256 `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`). The source fixture was not opened for editing.
- A separate `tinixcut mcp` subprocess applied `TrackCreate` at base revision 7. MCP returned `isError=false`, prior revision 7, new revision 8, one operation applied, process exit 0, and empty stderr. Response is preserved in `artifacts/phase37/reference_fixture/ui-mcp-guard-response.json`.
- With the native UI still holding its revision-7 snapshot, clicking `+ Track` created an in-memory `Track 5` at UI revision 8. Disk remained revision 8 with the MCP track (`MCP guarded track`) and no `Track 5` before Save.
- Clicking Save produced the visible status: `Save error: Project revision conflict: expected disk revision Some(7), found Some(8); reload the project or save as a new file to preserve your edits`. The UI retained `Track 5` in the timeline at revision 8 after the error. This confirms explicit stale-write rejection and preservation of the pending UI edit for user recovery.
- Disk verification after the conflict: revision 8, exactly the original four tracks plus `track_phase37_mcp_guard`; MCP track present, UI `Track 5` absent. SHA-256 remained `7C764D5712FD5DB11E6244904F47640DC8FF62B008C03BB765C984D7A2170AF0`, identical to the post-MCP pre-UI-save hash. Original fixture and earlier baseline copies were not touched.
- **Result: PASS for native stale-write prevention, visible conflict reporting, and pending edit retention.** Computer Use is operational in this session. Leave the live UI open with the unsaved `Track 5` and conflict message visible; do not save over the test file. Phase 37 remains IN PROGRESS for outstanding workflow parity/acceptance criteria; Phase 26 physical A/V, clean-VM, and installed-encoder gates remain OPEN.
## Verification performed for this inventory

- Read existing project roadmap/status and checked source definitions in MCP router and Command Service.
- Inspected filenames of candidate media artifacts only; no media was edited or decoded in this pass.
- Targeted unit acceptance `cargo test --manifest-path src-tauri/Cargo.toml --lib test_phase10_automation_surface_e2e_acceptance` passed (1 passed) using an isolated temporary `CARGO_TARGET_DIR`; its assertions cover the five capability omissions found in source.
- The regular integration-test invocation was attempted but could not finish because a pre-existing Phase 29 test has a missing `background_corner_radius` field and Cargo could not replace the running binary. A focused isolated test passed; see next line.
- The code changed only the automation capability report and its unit assertion. Restarted the exact workspace TinixCut PID 10624 and Vite PID 40700 using documented `pnpm tauri dev`; the new TinixCut PID 38380 started at 2026-11-02 22:44 and `http://localhost:1420/` returned HTTP 200. Computer Use AX reports Core Healthy v0.1.0, revision 0. The paired screenshot intermittently showed Tinix Photo Crop despite the TinixCut AX tree; no UI input was sent after that mismatch. Paseo was not restarted.
- Focused integration acceptance `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance test_p15_ac` passed (25 passed, 0 failed; 2 filtered) in isolated `%TEMP%\tinixcut-phase37-target`; it includes in-process AC24 but not cross-process writes.
- A read-only real MCP stdio smoke against the restarted workspace binary returned 12 tools from `tools/list`, returned `tinixcut_capabilities` successfully, and included all five formerly missing operations. The MCP process exited 0 on stdin EOF. This verifies capability discovery in the real binary, not the complete create/import/edit/export workflow.
- MCP E2E dry-run/apply/export/MP4 verification and native UI visual open/preview pass on disposable fixtures. Cross-process UI/MCP concurrent-write qualification failed with the reproduced lost update above. The regular Cargo integration-test invocation is blocked by the unrelated pre-existing `phase29_rich_text_domain.rs:1216` missing-field compile error; focused tests passed.

### Phase 37 native retest retry - 2026-11-03

- Computer Use `getState()` returned one running TinixCut window (HWND 71768400, title `TinixCut`). Binding that returned window failed with the exact error `Accessibility state unavailable for window 71768400`.
- Followed recovery guidance: refreshed the app/window list, which returned the same HWND, and retried binding once. It failed with the same exact error. No UI input was sent; the native visible-conflict and unsaved-edit retest remains **BLOCKED** by Computer Use state capture.
- Runtime check only: workspace TinixCut PID 42440 and workspace Vite PID 69360 remain running; `http://localhost:1420/` returned HTTP 200. Paseo was not restarted or stopped.
- The untouched reference project and baseline copy both remain SHA-256 `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`. The prior disposable concurrency test copy retains its recorded post-failure hash `81421252F564ED431AF13AF9161984CD0E292CD19FC9A1754A2AAB6546285EEE`.
- No code or project files changed in this retry. Phase 37 remains OPEN; do not interpret the blocked UI retest as a pass.
### Phase 37 automated acceptance coverage reconciliation - 2026-11-03

The already recorded command `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance test_p15_ac` passed 25 tests (0 failed; 2 filtered) after the persistence guard changes. Source/test-name cross-check confirms that this run includes:

| Concern | Acceptance test | Current evidence |
|---|---|---|
| MCP stale command revision | `test_p15_ac25_stale_revision_rejection` | Covered by the passing AC25 case; separate-process disk revision conflict is also covered by `phase37_cross_process_persistence` (2/2 PASS). |
| Failed multi-operation transaction | `test_p15_ac26_multi_op_atomicity` | Covered by the passing AC26 case; failed operation leaves revision, tracks, and command ledger unchanged. |
| Export job lifecycle cancellation | `test_p15_ac30_ac31_export_start_status_cancel` | Covered by the passing AC30/31 case; start, status lookup, and cancellation response are exercised. Cleanup after cancellation is not proven by that assertion. |
| Replay after MCP server restart | `test_p15_ac39_restart_replay_consistency` | Covered by the passing AC39 case; replay returns the existing revision. |

Therefore these automated acceptance cases are PASS and should not remain listed as wholly unqualified. Remaining Phase 37 gaps are the native UI visible-conflict/unsaved-edit retest (BLOCKED by Computer Use state capture), export cancellation resource/output cleanup, and any native or fixture-level parity checks not covered by the cited tests. This reconciliation relies on the recorded passing suite result; no test was rerun in this documentation-only update.
### Phase 37 export shutdown/cancellation cleanup test - 2026-11-03

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance test_p15_ac35_subprocess_shutdown_orphan_and_temp_cleanup -- --exact --nocapture`, using isolated target `%TEMP%\tinixcut-phase37-target`.
- Result: **PASS**, 1 passed, 0 failed, 26 filtered (2.26 s). The test starts a 10-second export in a real `tinixcut mcp` subprocess, closes its stdio while the render is active to trigger shutdown cleanup, waits for successful process exit, verifies no `.tc_render.tmp.mp4` remains, and creates/removes a write probe to show no orphan process holds the directory.
- This qualifies cleanup after shutdown-triggered cancellation. It does not exercise the `tinixcut_export_cancel` RPC and then poll the active job/output/temp artifacts; that explicit cancellation cleanup path remains unqualified. AC30/31's existing test verifies the cancellation response, while AC35 verifies the shutdown path.
- No source code changed. Workspace TinixCut was not restarted; the test binary and subprocess ran from the isolated Cargo target.
### Phase 37 explicit MCP export-cancel cleanup regression - 2026-11-03

- Added `test_p15_ac31_explicit_cancel_cleans_active_render_artifacts` to `src-tauri/tests/phase15_mcp_acceptance.rs`.
- It creates a 60-second disposable synthetic-media project and a pre-existing output file, starts an MCP export, waits until the partial `.tc_render.tmp.mp4` exists, calls `tinixcut_export_cancel`, verifies job status `cancelled`, waits for render/automation temporary artifacts to disappear, and confirms the pre-existing output bytes are preserved.
- Targeted test passed twice using isolated `%TEMP%\tinixcut-phase37-target`: 1/1 PASS on both runs; 27 other tests filtered. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed after formatting.
- The prior AC35 subprocess shutdown cleanup test also passed 1/1 and remains complementary evidence for shutdown-triggered cancellation.
- This closes the explicit cancel/output/temp cleanup automated gate. Native UI visible conflict and unsaved-edit verification remains BLOCKED by Computer Use state capture; remaining workflow parity/native checks are still open.
- Runtime after the test-source edit: Tauri dev app PID 35544 started at 2026-11-03 01:22:41, two seconds after `phase15_mcp_acceptance.rs` was last written at 01:22:39; the Tauri dev parent remains the workspace `pnpm tauri dev` process. Workspace Vite PID 69360 returned HTTP 200 on port 1420. The native window/Core health command could not be verified because Computer Use state capture remains unavailable; no Paseo process was stopped.

### Current Phase 37 status reconciliation — 2026-11-03

This entry supersedes the earlier native UI BLOCKED notes and the older export-cancel cleanup-open note above. Native UI/MCP stale-write prevention, visible conflict reporting, and unsaved `Track 5` retention are now PASS per the controlled retest. The explicit MCP export-cancel cleanup test also passed twice in the later entry. Remaining Phase 37 work is limited to outstanding workflow parity/acceptance checks; Phase 26 physical A/V, clean-VM and installed-encoder gates remain OPEN.
### Phase 37 read-only MCP inspect parity follow-up — 2026-11-03

- Called `tinixcut_inspect` in a fresh real `tinixcut mcp` subprocess against the concurrency-guard disposable. Response is preserved in `ui-mcp-guard-inspect-response.json`; process exit 0, stderr empty, `isError=false`.
- MCP inspect and disk both report revision 8, five tracks, six clips, four assets, 900 frames / 30.000 s at 30 fps, and 1080×1920. The original four tracks/items and media match the native UI view captured at rev7 (Video Track 1 / Scene A 450f; Video B / Scene B 450f; Music / Music bed 900f; Titles and subtitles / three text items of 90f, 60f, 60f; four media assets).
- The fifth track deliberately differs at the same numeric revision: disk/MCP have `track_phase37_mcp_guard` / `MCP guarded track`; stale UI has local `Track 5`. This is the external-change divergence the save conflict correctly detected. Disk SHA-256 before and after read-only inspect remained `7C764D5712FD5DB11E6244904F47640DC8FF62B008C03BB765C984D7A2170AF0`.
- **Result: PASS for read-only MCP inspect vs persisted project semantics, and PASS for conflict detection on divergent UI state. Full same-revision UI/MCP editing parity remains unqualified** because the app exposes the stale edit only in memory pending user recovery; do not compare their equal revision numbers as semantic equality.
- Computer Use/native runtime remained healthy (HWND 110764890, Core Healthy v0.1.0); workspace Vite health remained HTTP 200. No code was changed or runtime restarted.
### Phase 37 MCP → native UI → persistence → MCP parity slice — 2026-11-03

- After preserving the stale-edit/conflict evidence above, the native UI reopened the same disposable project from disk at revision 8. UI readback showed `MCP guarded track`, the four original tracks, six clips and four media assets; the former pending `Track 5` edit was discarded during this explicit reload.
- Native UI `+ Track` added `Track 6` at revision 9. Save succeeded and reported atomic save to `phase37_ui_mcp_concurrency_guard_disposable.tinix`.
- A fresh `tinixcut_inspect` subprocess then read the saved project at revision 9: six tracks including both `MCP guarded track` and `Track 6`, six clips, four assets, 30 s, 30 fps, 1080×1920. Exit 0, empty stderr, `isError=false`; read-only inspect left SHA-256 stable at `0958CEE65AFA3A01F45C5A4D98D21AE53DA2B7E29816E57F7B5FDF220C721E80`. Response: `ui-mcp-post-ui-inspect-response.json`.
- **Result: PASS** for loading MCP-persisted edits in the UI, adding/saving a native edit without dropping the MCP edit, and reading the combined canonical project back through MCP. This is a track-create/save parity slice; it does not qualify clip move/trim/split, audio, text or transition operation parity.
- The original `phase37_social_cut.tinix` remains unchanged. Runtime remains Core Healthy; no source code changed or restart was needed.
### Phase 37 clip speed parity checkpoint — 2026-11-03

- On the disposable concurrency fixture only, real MCP `ClipSpeedSet` applied `Half` to `clip_p37_scene_a` at revision 9→10 (receipt: `artifacts/phase37/reference_fixture/ui-mcp-speed-apply-response.json`). Disk readback is `speed=Half`, timeline duration 450 frames, source duration 225 frames. Disposable project SHA-256: `B147653BFF28EC93C7252411F61EC3EF3C526F00C7FB89B22443653EC3CC73F1`; original `phase37_social_cut.tinix` remains unchanged at `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`.
- Reloaded native TinixCut at revision 10 and selected Scene A. Computer Use accessibility state reports Core Healthy (v0.1.0), revision 10, item `clip_p37_scene_a`, and Inspector `Speed Value: 0.5×`. This passes MCP→persisted project→native UI readback parity for Half speed.
- UI→MCP speed-change parity remains **BLOCKED**. Selecting Normal via the dropdown hit `point (1971, 1595) is over msedgewebview2.exe "Chrome Legacy Window", not target window tinixcut.exe "TinixCut"; activate the target or take a fresh screenshot before retrying`. A second attempt using the observed accessibility item also returned the same target-window routing error. Refreshing TinixCut confirmed the value stayed at 0.5×; no UI speed change was applied. Per Computer Use recovery guidance, stop further input after the refreshed retry failed. Do not infer Normal/Double behavior from the MCP Half pass.
- Runtime observation: TinixCut native window remains available and reports Core Healthy v0.1.0. Workspace Vite PID 69360 was present, but a direct request to `http://127.0.0.1:1420/` returned curl exit 7 / HTTP `000` (connection refused). This is recorded as a development-server health issue; the app is not restarted because this checkpoint changed documentation only. Paseo was not stopped.
- **Result:** MCP→UI Half speed readback PASS; UI→MCP speed edits and UI Normal/Double speed selection BLOCKED by Computer Use input routing. Phase 37 remains IN PROGRESS; no code or original sample project changed.

Next: restore/verify Computer Use target-window input routing, then qualify UI Normal and Double edits by saving and reading them back through MCP. Continue remaining move/trim/split, audio, text and transition parity checks afterward.
### Phase 37 runtime health probe clarification — 2026-11-03

The earlier `127.0.0.1:1420` probe returned HTTP 000 because the current Vite listener is bound only on IPv6 loopback (`::1:1420`, PID 69360). Retesting through `http://localhost:1420/` and `http://[::1]:1420/` returned HTTP 200. The development frontend is healthy; no restart is needed. The TinixCut native window remains Core Healthy at project revision 10. This corrects the interpretation of the IPv4-only failure recorded in the speed checkpoint; the command result itself remains accurate.
### Phase 37 speed core regression evidence — 2026-11-03

- Ran `cargo test --manifest-path src-tauri/Cargo.toml --lib speed -- --nocapture` with isolated `CARGO_TARGET_DIR=%TEMP%\tinixcut-phase37-target`: **4 passed, 0 failed, 190 filtered**. Passing cases cover exact source-frame duration mapping, preview timeline-to-source mapping, Phase 26 speed-aware preview sampling (Half/Normal/Double), and `ClipSpeedSet` Double source coverage plus media-overrun rejection.
- An initial invocation with `--exact` matched zero tests because the test's full module-qualified name was not supplied; it was immediately rerun without `--exact` and passed 1/1. No test was claimed from the zero-test invocation.
- This is automated core regression evidence; it does not close native UI→MCP parity. The latest native read-only state still shows Scene A at Half / `0.5×`, revision 10. No UI input was sent because the current Computer Use surface exposed observation but no documented input action in this session. The previously observed Windows routing errors remain the blocker for manual Normal/Double selection and save/readback.
- Runtime remains testable: TinixCut reports Core Healthy v0.1.0 at rev10; `localhost:1420` and `[::1]:1420` return HTTP 200 via Vite PID 69360. The tests used an isolated Cargo target and did not replace the running executable. No code changed, so no server restart was needed.

**Result:** automated speed core PASS (4/4); native Half MCP→UI readback PASS; native UI→MCP Normal/Double remains BLOCKED. Phase 37 remains IN PROGRESS.
### Phase 37 MCP speed operations — Normal and Double — 2026-11-03

- **Normal PASS:** On `phase37_speed_normal_disposable.tinix`, a real `tinixcut_apply` subprocess changed `clip_p37_scene_a` from Half to Normal at revision 10→11; receipt reported one applied operation, exit 0, empty stderr, `isError=false`. Disk and a separate read-only `tinixcut_inspect` report Normal, 450 timeline frames / 450 source frames. Evidence: `mcp-speed-normal-double-evidence.json`.
- **Double PASS:** On separate `phase37_speed_double_disposable.tinix`, one atomic transaction first normalized the clip, trimmed timeline/source ranges to 225 frames, then set Double; receipt reported three applied operations at revision 10→11, exit 0, empty stderr, `isError=false`. Disk and a separate read-only inspect report Double, 225 timeline frames / 450 source frames. This avoids reading beyond the 450-frame source asset.
- Original `phase37_social_cut.tinix` hash remains `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`. The currently open native concurrency fixture remained rev10/Half and Core Healthy; these MCP tests used only the two new disposable copies.
- An initial subprocess attempt placed `_meta` inside tool arguments and omitted modern `server/discover`; server rejected it before applying anything. Both test copies remained unchanged at rev10/Half. The corrected request followed the repository's real subprocess handshake and succeeded. The rejected attempt is transport setup error, not a product result.
- These results qualify **MCP speed mutation/persistence/readback**, not manual UI parity. Native UI still shows Half at rev10. UI Normal/Double interaction and save/readback remain **BLOCKED** by the previously recorded Computer Use input-routing error.

**Result:** MCP Normal and Double operation paths PASS on isolated copies. Phase 37 remains IN PROGRESS pending native UI↔MCP edit parity and remaining workflow cases.
### Phase 37 MCP clip move / trim / split workflow parity slice — 2026-11-03

- Created `phase37_clip_move_trim_split_disposable.tinix` as a copy of the rev10 concurrency fixture; did not modify the original sample or the other qualification fixtures.
- A real `tinixcut_dry_run` evaluated all three operations with no warnings: move `clip_p37_scene_b` to the empty Track 6 at frame 450, trim it to 300 timeline/source frames, then split at frame 600 into `clip_p37_scene_b` and explicit right item `clip_p37_scene_b_right`.
- A real `tinixcut_apply` applied all three operations atomically, revision 10→11, 3 applied operations; a separate `tinixcut_inspect` returned `isError=false`. Both MCP subprocesses exited 0 with empty stderr. Evidence: `move-trim-split-dry-run-response.json` and `mcp-move-trim-split-evidence.json`.
- Persisted state confirms both fragments on `trk_43314d74a3314f478f2b0c500b48dec6`; timeline spans 15s + 5s contiguously and source spans 5s + 5s contiguously. Source timebase is mixed after split (`150/30` seconds on the left, `5/1` seconds on the right); comparisons must normalize rational time rather than compare raw `value` fields.
- The first post-apply harness assertion compared raw source `value` fields and exited nonzero (`source_duration=5`, expected raw value `150`). Inspection showed this was an invalid assertion across different timebases, not a product mutation failure. Rechecked using normalized seconds: both fragments are contiguous in timeline and source, on the target track, and inspect succeeded. No corrective project mutation was needed.
- Original `phase37_social_cut.tinix` SHA-256 remains `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`.

**Result:** PASS for MCP move→trim→split dry-run, atomic apply, persisted project readback, and independent MCP inspect on a disposable project. This does not qualify the corresponding native UI editing path; manual UI→MCP parity and other untested edit families remain open.

### Phase 37 MCP audio fade persistence slice — 2026-11-03

- On a fresh disposable copy, `tinixcut_dry_run` and `tinixcut_apply` accepted `ClipAudioFadeSet` for `clip_p37_music` with 30-frame fade-in and 60-frame fade-out; revision 10→11, one operation applied, no warnings.
- Independent `tinixcut_inspect` and persisted JSON confirm both rational frame durations exactly. MCP subprocess exited 0 with empty stderr and `isError=false` throughout. Evidence: `artifacts/phase37/reference_fixture/mcp-audio-fade-evidence.json`; project copy: `phase37_audio_fade_disposable.tinix`.
- **Result:** PASS for MCP audio fade mutation/persistence/inspect. Does not qualify native UI interaction or audible rendered fade shape; audio playback/export waveform verification remains open.

### Phase 37 MCP clip transition persistence slice — 2026-11-03

- On `phase37_transition_disposable.tinix`, a real MCP dry-run and apply set a 30-frame `FadeIn` transition on `clip_p37_scene_a`; revision 10→11, one operation applied, no warnings.
- Independent inspect and persisted JSON confirm `kind=FadeIn`, duration 30/30 seconds-rate (one second); all MCP responses had `isError=false`, subprocess exit 0 and empty stderr. Evidence: `artifacts/phase37/reference_fixture/mcp-transition-evidence.json`.
- **Result:** PASS for MCP `ClipTransitionSet` mutation/persistence/inspect. This verifies the clip-level FadeIn setting; it does not qualify native timeline transition controls or rendered preview/export visual parity.

### Phase 37 text and rendered fade checks — 2026-11-03

- Copied the audio-fade fixture into `artifacts/phase37/render-check-943d0fe6ddbe4eeba19cb893693b0d79/effect-and-text.tinix`. Real MCP dry-run and atomic apply set `clip_p37_title` to `PHASE 37 TEXT CHECK` and set 30-frame `FadeIn` on Scene A, revision 11→12, two operations applied, no warnings; MCP inspect succeeded and persisted project text matched. The changed title is visibly legible in `effects-0_5s.png`. This verifies MCP→saved project→export text, not native UI→MCP parity.
- Exported control (rev10, no fades) and effects (rev12, 30-frame FadeIn, 30-frame audio fade-in and 60-frame fade-out) with real MCP jobs to separate MP4s. Both jobs completed; ffprobe 9.0.2 found H.264/AAC, 1080×1920, 30 fps, 900 frames, 30.000 s. FFmpeg 9.0.2 full decode with `-xerror` exited 0 for both.
- Compared the same 200×200 region at `(0,700)` in the MP4s, away from title: mean gray at frames 0/15/30 was control 85.69/88.97/89.81 and effects 0.00/36.64/89.68. The frame at 0.5 s was visually inspected as well. This supports an exported FadeIn on this fixture; preview/native and cut-transition parity remain open.
- Measured mono 48 kHz PCM RMS in 0.1-second windows from decoded MP4s: at 0.1/0.5/1.5 s control 99.89/100.71/100.17 versus effects 15.45/55.80/100.89; at 27.0/29.0/29.7 s control 100.48/101.06/101.03 versus effects 100.41/47.93/12.70. These comparisons support the expected fade-in and fade-out in this synthetic fixture; they do not qualify physical A/V synchronization or all audio inputs.
- Evidence, MP4s, snapshots, measurement script, ffprobe results and normalized PCM/window metrics are in that artifact directory (`mcp-evidence.json`, `render-analysis.json`, `analyze.py`). The script's first attempt used the removed FFmpeg `-vsync` option and failed before the analysis; it was corrected and rerun. An initial black-margin crop produced all zero values; the reported crop was chosen after inspecting the frames. Original `phase37_social_cut.tinix` SHA-256 remained `8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801`.

**Result:** PASS for this fixture's MCP text→persisted project→rendered title and exported FadeIn/audio fades. UI operation parity, preview/export parity, Phase 26 physical A/V and clean-VM/installed-encoder gates remain OPEN.

### Phase 37 Computer Use input recovery retest — 2026-11-03

- Reselect/activate plus screenshot-backed capture allowed Step Forward (0→1) and opening a new disposable project. Screenshot and delayed accessibility state agreed; TinixCut remained Core Healthy.
- Native Speed dropdown opened on Scene A but clicking Normal in its screenshot-backed popup failed: `point (2233, 1296) is over msedgewebview2.exe "Chrome Legacy Window", not target window tinixcut.exe "TinixCut"; activate the target or take a fresh screenshot before retrying`. Refreshed state remained 0.5×, rev10. Input was stopped; UI Normal/Double→save→MCP parity remains BLOCKED. Full recovery details: `Plans/phase26_manual_qualification.md`.

### Phase 37 native UI speed save/readback — 2026-11-03

Native keyboard navigation through the Speed control enabled Half/Normal/Double without clicking the WebView popup. Native UI committed and saved Normal at rev11 (450 timeline/450 source frames) and, after a split, Double at rev13 (149/298 frames) in `phase37_native_speed_recovery_disposable.tinix`; a separate `phase37_native_speed_half_disposable.tinix` was changed Half→Normal→Half and saved at rev12 (450/225 frames). MCP inspect independently returned the matching project revisions with `isError=false`; the speed and ranges were verified in persisted project JSON, since inspect's summary does not expose per-item speed. Evidence: `native-speed-{normal,double,half}-inspect.json` in `artifacts/phase37/reference_fixture/`; full procedure in `Plans/phase26_manual_qualification.md`.

**Result:** PASS for UI speed selection/commit/save and persisted-data readback of Half/Normal/Double on these clips. The WebView popup mouse-routing issue remains; native preview/export parity is not established by this check.

### Phase 37 speed export and native preview samples — 2026-11-03

MCP exported the UI-saved Half and Double projects and the earlier MCP-saved Normal project. All three 30 s 1080×1920 H.264/AAC files passed ffprobe and full FFmpeg decode. At output frame 30, source-frame comparison found best matches at frames 15, 30 and 60 respectively. Native UI previews at frame 30 visually matched the corresponding exported PNGs; separate FadeIn preview at frames 0/15/30 showed black/dim/bright video, consistent with previous export measurements. This is sampled visual evidence, not continuous-playback, pixel-level or physical A/V qualification. Full methods and artifacts are in `Plans/phase26_manual_qualification.md` and `artifacts/phase37/speed-render-28872a89cda046ae94478628a997f4b9/`.

### Phase 37 Double speed boundary and playback follow-up — 2026-11-03

On the native UI-saved rev13 Double project, output frames145–152 from the existing MP4 match source frames290,292,294,296, then149,150,151,152 (best grayscale MAE 0.729–0.737). Native preview was sought to frames147–149, then Play traversed the 149f split; later capture confirmed playback stopped at frame298. This covers only eight export samples and qualitative UI traversal. The split contains no transition; continuous frame-by-frame playback and native/export pixel parity remain open. Details and method: `Plans/phase26_manual_qualification.md`, `artifacts/phase37/speed-render-28872a89cda046ae94478628a997f4b9/boundary_analysis.json`.
### Phase 37 native CrossDissolve qualification slice — 2026-11-03

This update supersedes the prior note that transition rendering on this project was wholly open: native UI created and saved a 30-frame `CrossDissolve` between the Double and Normal fragments in disposable project rev14. Native previews at frames134/149/164 were qualitatively visible; MCP export completed at H.264/AAC 1080×1920, 30 fps, 900 frames/30 s, and full FFmpeg decode passed. Comparison against the no-transition rev13 export showed a distinct 30-frame render change at frames134–163. This does not establish native/export pixel parity, other transitions, frame-drop behavior, physical A/V or release qualification. Evidence: `Plans/phase26_manual_qualification.md`, `artifacts/phase37/speed-render-28872a89cda046ae94478628a997f4b9/{cross-dissolve-export-mcp.json,cross-dissolve-analysis.json,cross-dissolve-frames-contact-sheet.png}`.
### Phase 37 two-source native transition retest — 2026-11-03

- Prepared disposable `artifacts/phase37/reference_fixture/phase37_cross_dissolve_two_sources_disposable.tinix` by MCP Command Service from rev14; it is rev15 on disk, with Scene A and Scene B as distinct sources on one track and no persisted track transition. Original media/project were not edited.
- Computer Use was available and targeted TinixCut HWND 110764890. The app screenshot showed project rev15 while its accessibility document reported rev13. Opening the fixture through the native file picker closed the dialog, but the resulting tree still reported rev13; therefore native load identity was not proven.
- One native transition-control action returned status `Command failed: Cannot create or update transition: required media handle exceeds available source frames. Code: INSUFFICIENT_TRANSITION_HANDLE. Required preroll: 15/30 (0.500s), available preroll: 0/30 (0.000s), required postroll: 15/30 (0.500s), available postroll: 0/30 (0.000s).` No further timeline input was sent. No transition was confirmed saved on the two-source fixture.
- **Result: BLOCKED** for native two-source transition, multiple transition durations, and associated preview/export parity. The fixture file remains at rev15; runtime was not restarted. Existing same-source native 30f CrossDissolve evidence remains valid for its own fixture only.
- Next: establish a fresh UI load whose screenshot and accessibility both identify the two-source fixture/revision; prepare a separate copy with usable source handles on both sides of the edit; then qualify a 30f transition followed by a second duration on another disposable copy. Export each through MCP and compare native preview samples against matching export frames.
### Phase 37 two-source CrossDissolve render variants — 2026-11-03

- Created `phase37_cross_dissolve_two_source_handles_disposable.tinix` from the previous disposable rev15 copy. Through MCP Command Service, set Scene A to Normal (source 0–149; asset duration 450), trimmed Scene B to source 30–331 (301 frames) at timeline 149–450, and removed the duplicate later Scene B clip. Dry-run and apply passed; fixture advanced rev15→16. This provides 301f outgoing post-roll and 30f incoming pre-roll.
- Independent MCP dry-runs for CrossDissolve durations 30f and 15f both passed at rev16 without mutation. Applied each duration via Command Service to its own copy; both advanced rev16→17 with one persisted track transition. Evidence: `artifacts/phase37/reference_fixture/two-source-handles-fixture-mcp-evidence.json`, `two-source-transition-handle-probe.json`, `two-source-{30f,15f}-transition-apply.json`.
- MCP exported both transition projects and a no-transition rev16 control. Each MP4 is H.264/AAC, 1080×1920, 30 fps, 900 frames/30 s; each full FFmpeg `-xerror` decode returned 0. Center-video MAE against control is near baseline before/after the effect, while the 30f variant differs over frames 134–163 (e.g. frame149 MAE 57.757) and the 15f variant over frames 142–156 (frame149 MAE 61.429); at frame163 both return to baseline. Contact sheet, frame PNGs, analysis script and MCP receipts: `artifacts/phase37/two-source-transition-render/`.
- Native UI attempt remains **BLOCKED**. After opening the handle-safe fixture, screenshot title reported rev16 but Computer Use accessibility continued to report rev13 even on a fresh text-only capture. No further timeline action was sent. Therefore these MCP render passes do not qualify native UI creation or native-preview/export parity. Existing same-source 30f native evidence is separate.
- **Result:** PASS for MCP transition handle validation, persistence and two-source export rendering at 15f/30f. BLOCKED for native two-source edit and preview/export parity until Computer Use exposes a consistent loaded project/revision. TinixCut stayed Core Healthy; no code changed and no runtime restart was needed.

### Phase 37 native two-source CrossDissolve UI qualification — 2026-11-03

This entry supersedes the BLOCKED notes below for the handle-safe two-source fixture; those entries remain accurate for the earlier rev15 fixture and stale Computer Use captures.

- Computer Use is operational. TinixCut screenshot and accessibility both identify the currently open disposable project at revision 18, Core Healthy v0.1.0, paused at frame 156. The Inspector identifies Scene A and its outgoing CrossDissolve with Scene B different source, Active, duration 15 frames. No input was sent during this read-only verification.
- Native UI created and saved a 30-frame CrossDissolve between clip_p37_scene_a and clip_p37_scene_b_right on phase37_cross_dissolve_two_source_handles_disposable.tinix, revision 16→17. A separate copy was opened, changed through the native Inspector to 15 frames, and saved as phase37_cross_dissolve_two_source_15f_native_disposable.tinix, revision 17→18. Persisted JSON confirms CrossDissolve durations 30/30 and 15/30 respectively, same distinct-source item IDs and track.
- Both saved native projects were exported through MCP. ffprobe reports H.264 1080×1920, 30 fps, 900 frames, 30 seconds; full FFmpeg -xerror decode exits 0. Each native-UI-saved MP4 is byte-identical to its corresponding MCP-applied variant: 30f SHA-256 E6FAAC1F7381B7CEB2CFFB46DCC365836CB48A641C30F83A2CEC7F39228534F7; 15f SHA-256 8D450CDBEBA25A3C0A91A48D196F1C6903DC47A3B4618BFF9D144FBD13E979DC. Sampled native preview frames 141/142/148/149/156 were qualitatively consistent with rendered output; this is not a numerical screenshot-to-export pixel comparison.
- Disposable project hashes: 30f AFB3C4927FE0F0FEA1112087040664DBDC40063EA39F84285C96146B8C5CA07E; 15f B1845DD8F575E6868E2E8F60C8A6AC6B5F6B625C14E65E763354D2176FDA6AE8. Original phase37_social_cut.tinix SHA-256 remains 8718459B49C896B9C1AD2D54976CE2BEA5F9EABB150B3DBA47B785BAEF14B801. No original sample was edited.

**Result:** PASS for native UI two-source transition creation, duration change (30→15 on a separate copy), save/readback, preview sampling, MCP export and sampled preview/export consistency. Prior BLOCKED notes about stale AX are superseded for this fixture. Other native operation parity, continuous playback/frame drops, physical A/V, clean VM and installed encoder remain open. TinixCut is running; this was documentation/evidence-only and required no restart.


### Phase 37 native UI audio fade parity — 2026-11-03

- On disposable copy `phase37_native_audio_fade_parity_disposable.tinix`, selected `Music bed` in the native Inspector. Set Fade In to 30 frames and Fade Out to 60 frames; both edits committed through the UI and were saved atomically, revision 18→20. Persisted `clip_p37_music` values match 30/30 and 60/30 frames. The earlier two-source 15f transition remains on this copy; the 15f baseline export has the same transition and music source at 35% volume, without audio fades.
- Exported the UI-saved revision through MCP. ffprobe reports H.264/AAC, 1080×1920, 30fps, 900 video frames, 48kHz mono audio, 30 seconds; FFmpeg full `-xerror` decode exits 0. PCM RMS from matching 100ms windows confirms exported fade shape: relative to baseline, 0.1s=15.46%, 0.5s=55.41%, 0.9s=95.79%, 1.1s=101.4%; at the end, 27.9s=100.58%, 28.5s=73.31%, 29s=47.43%, 29.5s=22.66%, 29.9s=2.90%.
- Reproducible artifacts: `artifacts/phase37/reference_fixture/native-ui-audio-fade-evidence.json`; MCP receipt and MP4, baseline MP4, PCM analysis JSON and script are in `artifacts/phase37/two-source-transition-render/` (`native-ui-audio-fades-export-mcp.json`, `native-ui-audio-fades.mp4`, `native-ui-two-source-15f.mp4`, `native-ui-audio-fade-analysis.json`, `analyze_native_ui_audio_fade.py`). Original sample hash remains unchanged.

**Result:** PASS for native UI audio-fade edits, saved project readback and exported audio envelope compared with the same fixture without fades. This supports native UI→persisted project→MCP export parity for this operation. It does not qualify physical A/V synchronization or audio-fade preview. No code changed; runtime restart was not applicable.

### Phase 37 native UI title edit parity — 2026-11-03

- On isolated copy `phase37_native_text_parity_disposable.tinix`, selected `Opening title` and changed its caption through the native Inspector to `PHASE 37 NATIVE TEXT CHECK`. The UI committed `Set Text Caption` at revision 21 and saved the copy atomically. Persisted `clip_p37_title.text_content` matches the requested title; project SHA-256: `A23FC3CFD9EC20F4DB41EC6F473A107E1BCC8753ECB2BA5322C9B9CB8E6E7263`.
- MCP exported the saved project. ffprobe reports H.264/AAC, 1080×1920, 30fps, 900 video frames, 48kHz mono audio, 30 seconds; full FFmpeg `-xerror` decode exits 0. The extracted one-second frame visibly contains the updated title. Evidence: `artifacts/phase37/reference_fixture/native-ui-text-edit-evidence.json` and `artifacts/phase37/two-source-transition-render/native-ui-text-edit-{export-mcp.json,mp4,frame1s.png}`.

**Result:** PASS for native UI title edit, persistence readback and MCP rendered export. This is a sampled render check on a synthetic fixture; broad typography/layout coverage remains open. Original sample untouched; no code change or runtime restart.

### Phase 37 native text-style UI ? persisted-project parity ? 2026-11-03

- Computer Use selected TinixCut HWND 110764890. Screenshot and accessibility tree agreed on the open disposable project `phase37_native_text_parity_disposable.tinix`, revision 21, Core Healthy v0.1.0.
- In the native Inspector, changed the selected `Opening title` font size from 48 to 60. UI reported `Update text style committed (rev 22)`; Save reported the project saved atomically. No other style fields were changed.
- Independent JSON readback from the saved project confirms revision 22, `clip_p37_title.text_content=PHASE 37 NATIVE TEXT CHECK`, `text_style.font_size=60.0`, and `font_family=Inter`. SHA-256 after save: `BB36A3705B6800E11F1986EF0B624C88058F4FA14E561A666DDCB3A9C531AC48`. A fresh MCP `tinixcut_inspect` call on this project returned revision 22 with `isError=false`; as the inspect schema omits per-item style, the JSON readback is the per-field evidence. MCP export job completed as `export_37430bedb242458289fb459f41463c80`; output `artifacts/phase37/reference_fixture/phase37_native_text_style_rev22.mp4` is H.264/AAC, 1080?1920, 30 fps, 900 frames, 30 seconds, and passed full FFmpeg `-xerror` decode. The extracted one-second frame visibly renders the larger title. Artifacts: `native-text-style-export-evidence.json`, `native-text-style-ffprobe.json`, `native-text-style-frame-01s.png`.
- Original sample project was not modified. TinixCut remains Core Healthy; no code/runtime configuration changed, so restart was not applicable.

### Gate disposition ? 2026-11-03

Physical-output A/V measurement is not a Phase 37 or development prerequisite; it is required only before publishing a numeric physical synchronization tolerance such as ?within one frame.? Clean Windows VM and packaged encoder checks are release-candidate qualification for an installer and are deferred to Phase 43; they are removed as blockers for ongoing phases. Historical Phase 26 entries remain intact as dated evidence and are superseded by this disposition.

### Phase 39 MCP bootstrap and media probe increment — 2026-11-03

- Added `tinixcut_project_create(projectPath, name)`, creating the canonical empty `Project` and persisting it through revision-checked atomic save (`expected_revision=None`). Existing destinations are refused; no overwrite path is exposed.
- Added `tinixcut_media_probe(mediaPath)`, a read-only metadata operation using `FFmpegService::probe_file`; it returns file size, duration, dimensions, frame rate, codecs, audio details and stream summaries. Invalid/offline paths produce `MEDIA_PROBE_FAILED`.
- `src-tauri/tests/phase15_mcp_acceptance.rs` now requires 14 tools and tests create/reopen, non-destructive duplicate-create refusal, valid audio/video probe, missing-media error, and generated JSON Schema 2020-12.
- Verification: three isolated-target integration tests passed (`test_phase39_mcp_project_bootstrap_and_read_only_media_probe`, MCP tool inventory, MCP schema). `rustfmt` applied to the two changed Rust files. Workspace `cargo fmt --check` still detects an earlier formatting-only diff in `src-tauri/src/modules/command/mod.rs`; that existing code was not changed in this increment.
- This closes only the API/bootstrap slice. The 14-tool subprocess E2E and native UI reopen/export gates remain open. No original project was changed.
### Phase 39 complete MCP acceptance-suite recheck — 2026-11-03

- Ran `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance -- --nocapture` with isolated `%TEMP%\tinixcut-phase37-target`.
- Result: **29 passed, 0 failed** (1.53 s). The real stdio subprocess E2E also passed with the expanded tool router; tool discovery reports 14 tools.
- This suite does not prove the entire Phase 39 gate: the new create/probe calls were exercised through the MCP server duplex transport, while a full real-subprocess create→probe→import/edit→export workflow and native UI reopen remain unverified.
### Phase 39 full stdio workflow smoke — 2026-11-03

- Executed against the restarted workspace binary via a fresh real `tinixcut mcp` process: `tools/list` (14 tools) → `tinixcut_project_create` → `tinixcut_media_probe` → dry-run/apply of AssetImport + TrackCreate + ClipCreate → `tinixcut_inspect` → asynchronous export start/status → stdin EOF shutdown.
- The 2-second synthetic audio/video fixture was generated inside `artifacts/phase39/mcp_reference_workflow_20261103/`; project and MP4 output are disposable and self-contained there. Dry-run evaluated 3 operations; apply produced revision 1 with one track/clip; export completed as 1920×1080 H.264 video (60 frames, 30 fps) + AAC audio, 2.0 seconds. Packaged ffprobe inspection and full `ffmpeg -xerror` decode passed; MCP exited 0 with empty stderr.
- Machine-readable result: `artifacts/phase39/mcp_reference_workflow_20261103/report.json`. This closes the headless bootstrap/probe/import/edit/export slice. Phase 39 remains open for native UI reopen and same-edit semantic parity, plus remaining stale/retry/rollback/offline/cancel/process-lifetime/concurrent-client cases not already covered by existing tests.
- Runtime after Rust rebuild/restart: local `pnpm tauri dev` restarted; TinixCut PID 61464 started at 12:37:08, workspace Vite serves HTTP 200 at port 1420, Computer Use AX reports Core Healthy v0.1.0 and the Go to frame control. The blank UI project remained untouched; Paseo stayed running.
### Phase 39 complete — MCP workflow and native reopen — 2026-11-03

- Fresh real `tinixcut mcp` stdio workflow discovered 14 tools; created project revision 0, probed synthetic H.264/AAC input, dry-ran and applied AssetImport + TrackCreate + ClipCreate, inspected persisted revision 1, exported asynchronously, and exited with code 0 and empty stderr. Report: `artifacts/phase39/mcp_reference_workflow_20261103/report.json`; fixture, project and MP4 are colocated.
- Export verification: H.264/AAC, 1920×1080, 30 fps, 60 video frames, 2 seconds; packaged ffprobe passed and full FFmpeg `-xerror` decode exited 0.
- Native TinixCut opened the exact MCP-created `.tinix` file without editing or saving it. Screenshot and accessibility both showed `Phase 39 MCP reference (rev 1)`, `fixture.mp4`, one Video track, `Fixture clip (60f)`, and one timeline clip. The project/report agree on revision 1 and counts (1 asset, 1 track, 1 clip).
- The full MCP acceptance suite passed 29/29. It covers tool/schema lifecycle, create/probe, idempotency replay/collision/sequence gaps, concurrent writes, stale revision rejection, transaction atomicity, save/reopen, export status/cancel/cleanup/snapshot isolation, shutdown cleanup, security boundary and subprocess replay. Cross-process stale-writer regression also passed earlier (2/2).
- **Result: PASS — Phase 39 complete.** The file-picker accessibility index had failed, but a fresh screenshot-backed coordinate action opened the project successfully; Computer Use is operational for this native reopen. No original project was changed. Runtime was already built/restarted for the MCP code and remains Core Healthy; this verification changed no code and required no restart.