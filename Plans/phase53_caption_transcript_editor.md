# Phase 53 — Captions, transcript and text-based editing

**Status:** IMPLEMENTED WITH DEFERRED QUALIFICATION — caption editing, source transcript analysis/search across project assets, source-range rough cuts, and transcript-gap pause suggestions are implemented. A preview raster-path candidate addresses the reproduced legacy square-background caption mismatch and passes automated tests; updated-runtime native frame-60 confirmation, dirty-draft cancel and expanded subtitle qualification remain open.
**Updated:** 2026-11-05  
**Plan:** [Product completion, Phases 48–57](plan_product_completion_phases48_57.md)

## Existing capabilities retained

- Local auto-caption uses the transcription job manager with progress and cancellation, then creates text items through Command Service.
- SRT and WebVTT import/export already use native parsing/serialization and atomic output; imported cues become text items on a new track.
- Text clips use the shared text/template renderer and existing timeline/history commands.

## Implemented slice: caption list editor

- TopBar opens a caption editor listing text items in timeline order with track and time range.
- Search and replace-all operate on a local draft; multiple text changes apply as one Command Service envelope (`ClipTextSet`) and one undoable transaction.
- Clicking a time range seeks the playhead to its start. `Split here` is enabled only strictly inside the selected text item's frame range and uses `ClipSplit` at the playhead.
- Start-frame and duration fields allow precise timing changes on the project frame grid. Applying text and timing drafts sends one Command Service envelope; start-only edits use `ClipMove`, while duration edits use `ClipTrim` with an exactly converted source duration. Locked tracks, invalid values, duplicate IDs and durations not representable on the source time grid are rejected.
- Select two immediately adjacent text captions on the same unlocked track and merge; one Command Service transaction joins the text with a line break, extends the first range and deletes the second. The helper rejects gaps, locked tracks, non-text clips, duplicate/unrepresentable source-grid durations and empty text.
- Auto Caption now has a read-only `Analyze transcript` mode alongside caption generation. It reuses the cancellable local Whisper job, displays searchable source timestamps/text separately from timeline captions, and seeks only segments that map into the selected clip's source range. Analysis does not mutate the project; entries preserve source-time provenance in the transient job report.
- Each transcript segment can open the selected source asset at its source timestamp in Source Viewer, seek the mapped timeline position when the segment lies within the selected clip's source range, or insert its source range at the timeline playhead. Multi-select creates a visible rough-cut draft with selected segment count/duration; Apply places the source ranges consecutively on a dedicated track in one Command Service transaction. Individual segment insert remains available. Source seeking waits for media metadata and is clamped to the known asset duration.
- Completed transcripts are searchable across assets analyzed during the current project session. Entries retain asset ID/name/path provenance; relinked or removed assets are excluded, project changes clear the in-memory library, and source insert/rough-cut actions resolve the exact asset ID rather than whichever clip is currently selected. Timeline seek is offered only for the currently active report so an old mapping cannot be mistaken for current timeline placement. Transcript library data is session-only and is not persisted into the project.
- Pause-cut suggestions group transcript segments separated by gaps above a configurable threshold and preserve configurable speech-edge padding. This is explicitly a transcript-timing heuristic, not waveform silence detection. The UI previews the source ranges and asks the user to listen before applying; the resulting rough cut uses the same Command Service path as selected transcript ranges.
- Track-wide text-style application already exists in the Inspector through `TextTrackStyleSet`; it is not duplicated in the caption list editor.
- Transcript results use the existing rebuildable cache, keyed by source-file metadata plus Whisper CLI/model fingerprints, language and algorithm version. Repeat analysis reuses a matching result; source or configuration changes select a new cache entry. The UI and MCP use the same cache namespace and fingerprint contract.
- Split is disabled while text drafts are dirty to avoid losing unapplied edits. UI-only draft state is discarded explicitly or when a new project/revision replaces it.
- Pure tests cover chronological listing, search normalization, split boundaries, valid start/duration frames, no-op filtering, move/trim command construction, exact source-grid conversion, locked-track/non-caption rejection and adjacent merge.
- Locked tracks stay read-only in the editor. Existing SRT/WebVTT import/export now has a Unicode/multiline round-trip test for both formats with timestamp tolerance of one frame.

## Remaining work

- Native SRT import and Caption Editor display were verified on the disposable UI project: the app reported `Imported 1 caption(s) into a new track`, showed `Phase 53 caption smoke` in the caption list, and opened the cue with 00:00:00:00 start and 2.00 s duration. Closing the editor left the cue visible and unchanged.
- Historical first edit probe (superseded below): the WebView accessibility subtree collapsed and rendered black before Apply. The input handler was fixed to copy `event.currentTarget.value` synchronously before scheduling the React updater.
- Native edit, cancel-with-dirty-draft, timing edit, Undo/Redo for captions, save/reopen and export remain for the final blocker pass. Computer Use was unavailable; this evidence uses Windows UI Automation and direct screenshots only.
- True audio-amplitude silence detection is not implemented; current pause cuts are transcript-gap estimates and must remain labeled as such.

## Verification

- Earlier cache/UI candidate verification: Rust transcription tests 3/3, transcript cache tests 4/4, `cargo check --bin tinixcut`, `cargo fmt --check`, TypeScript, full frontend suite 143/143, production build, and `git diff --check` passed. Bundle was 544.70 kB and triggered the existing >500 kB advisory. Vietnamese SRT/WebVTT round-trip/parser tests remain 6/6 from an earlier candidate.
- Current caption/timing/transcript rough-cut and multi-asset search/pause-suggestion candidate: TypeScript passes; focused transcript/caption/App tests 41/41; full frontend suite 148/148; production build passes; `git diff --check` passes. Bundle is 555.07 kB and retains the >500 kB advisory. Runtime restarted after the final code change: TinixCut PID 21744 (13:38:24 local), Tauri CLI PID 35260 and Vite PID 24112 (13:38:20 local); `localhost:1420` returns HTTP 200. Vite logs `TypeError: Cannot read properties of undefined (reading 'invoke')` in health/project/preview IPC, so app readiness is not confirmed. Computer Use tools are unavailable here; native visual checks are deferred to the final blocker pass.
- Caption timestamp fixture regression: **7/7** caption parser/serializer unit tests pass. New test covers non-zero SRT/WebVTT offsets at 29.97 fps, millisecond-to-frame rounding on import, and serialize/parse round-trip within one frame.
- Native UI evidence on 2026-11-04 confirms import and cue-list rendering only; see the `Native SRT import/editor smoke` section below for the accessibility-tree loss and black-WebView blocker during an edit attempt.

### Native SRT import/editor smoke — 2026-11-04

- Imported the disposable `phase53_native_test.srt`; TinixCut reported one caption imported into a new track. Caption Editor listed one text clip with `Phase 53 caption smoke`, start `00:00:00:00`, and 2.00-second duration. Closing the editor preserved the displayed cue.
- A later UI Automation probe targeted the caption text field; the text edit was not applied. The WebView subtree then exposed only pane nodes and a direct window screenshot showed a fully black content area. The editor Apply button was never invoked.
- Restarted only the workspace development candidate from the isolated temp Cargo target. The new window rendered Core Healthy; the recovery prompt was dismissed with `Later`, without restoring or discarding any copy. Standard TinixCut PID 33680 and Paseo stayed running.
- **Result:** SRT import/list display PASS; native edit/Undo/Redo/save/reopen/export remain BLOCKED or unqualified. Do not infer a caption mutation from the unsuccessful probe.
- Rust Phase 52 color/migration/command tests and legacy effect graph regression pass; `cargo check` passes.
- Runtime starts from the current debug binary and Vite serves HTTP 200, but the dev WebView logs `TypeError: Cannot read properties of undefined (reading 'invoke')` in preview IPC (and previously in health/project IPC); typed app health could not confirm core readiness. This remains a runtime blocker to diagnose in the final blocker pass. Computer Use tools are unavailable in this session, so visual review and save/reopen remain deferred there as well.
- Installed-package checks were not run.

### Candidate caption-edit retry — 2026-11-04 (historical failure; superseded)

- On the current disposable project, Caption Editor opened and UI Automation exposed the cue list, `Caption text 1.00–6.00s` edit field, and disabled `Apply edits` before typing.
- After focusing that field and sending one select-all/text-entry action, the candidate WebView rendered fully black and its accessibility subtree no longer exposed the dialog or controls. Apply was not invoked and no successful text edit is claimed. This reproduces the native caption-field blocker.
- Candidate TinixCut PID 4140 and Vite PID 20892 were left running; `localhost:1420` returned HTTP 200. No other TinixCut/Paseo process was stopped. The saved Phase 52 fixture copy was not modified.
- Defer further native caption interaction to the final blocker pass; do not retry blind input while the WebView is black.

### Caption edit recovery and preview/export check — 2026-11-04

- Fixed `CaptionEditorDialog` so the input value is copied from `event.currentTarget` synchronously before the React state updater runs. This removes the observed asynchronous `currentTarget` access that previously crashed the WebView. TypeScript, the focused caption suite (7/7), and production build passed before runtime restart.
- Restarted the exact isolated workspace candidate (`%TEMP%\tinixcut-phase55-runtime-target`); candidate PID 39116 showed `Core Healthy (v0.1.0)`, Vite PID 34308 served HTTP 200 on port 1420. Standard TinixCut PID 33680 and Paseo were left running. Recovery dialogs on both visible instances were dismissed with **Later** only; no recovery copy was restored or discarded.
- On disposable project copies, native Caption Editor Apply committed the exact text `Phase 53 caption after fix`; Undo/Redo restored the previous/new value, and reopening the saved project showed the exact new text. The visible-position copy is `artifacts/phase53/disposable/phase53_caption_edit_visible.tinix` (revision 36, Position Y -100).
- At frame 60 the native preview showed two active layers but did not show the edited caption. The frame contained only the Vietnamese subtitle already burned into the source fixture. Moving the text into the visible canvas and saving a separate fixture did not change this result. Preview/render parity remains **BLOCKED**; do not claim preview parity.
- Native export completed to `phase53_caption_edit_visible.mp4`. FFprobe reported H.264/AAC, 1920×1080, 30 fps, 450 video frames, stereo 48 kHz, 15 seconds; full FFmpeg decode exited 0. Frame 60 visibly contains the editable caption over the source's burned-in subtitle, confirming export rendering.
- Evidence: `artifacts/phase53/disposable/phase53_caption_edit_visible.tinix`, `phase53_caption_edit_visible.mp4`, `phase53_caption_export_frame60.png`, `phase53_caption_preview_frame60_ui.png`, and `artifacts/phase53/phase53_caption_native_qualification.json`.
- **Result:** Native edit/Apply/Undo/Redo, save, export and decode PASS on a disposable fixture. Caption preview visibility/parity remains **BLOCKED / G53 OPEN**. The earlier black-WebView failure is resolved. Evidence uses Windows UI Automation and direct screenshots because Computer Use is not exposed in this session.

### G53 caption preview pipeline isolation — 2026-11-05

Loaded the exact saved disposable revision 36 fixture and sampled its composition at frame 60 using Rust `CompositionPlan::sample_preview_at`. The sample contains both active layers, with the editable caption text `Phase 53 caption after fix` at z-index 1 above the video layer (z-index 0), opacity 1, and a valid 1920×1080 canvas-space bound. The same project’s export frame contains the editable caption; the recorded native screenshot contains only the source-burned subtitle. This rules out missing project data and the Rust composition sampler as the cause. The remaining failure is downstream in frontend/WebView rendering, but the precise cause is not yet established. No speculative frontend change was made. G53 remains OPEN; deterministic PreviewCanvas rendering/native screenshot is required before changing layout behavior. Evidence: `artifacts/phase53/phase53_caption_native_qualification.json`.

### G53 native preview recheck attempt — 2026-11-05

- Created a disposable copy of `phase53_caption_edit_visible.tinix` with a new project ID under `artifacts/phase53/disposable/recheck_20261105/`. The source project hash remained `1C716E57971F2442F3BB518EF164B752E47ABA246DCCE913D0062B56670AD8AD`; the isolated copy hash is `32466FEC9933995665C01B6380858D44B262C4734FE1E681447A45BD4F70F782`.
- Read-only UIA identified workspace TinixCut PID 12440 at Core Healthy, Untitled Project revision 0. The native Open picker appeared and the absolute filename field read back the copy path byte-for-byte. A BM_CLICK to its identified Open button left the picker open and the project unchanged; the picker was canceled with WindowPattern.Close on that exact PID/window.
- A recovery-list modal was visible behind the picker afterward. No Restore or Discard action was taken. Existing recovery files were not modified. Screenshot SHA-256: `3C04BA17729AC4A4AF933BE455310037F075CC105338443EE86DC0FFBF6B53E3`.
- **Result:** no fresh preview observation was obtained. This confirms the native open-picker activation route is still unreliable; it does not supersede the recorded G53 preview/export mismatch or close G53. Machine evidence: `artifacts/phase53/disposable/recheck_20261105/native_recheck_evidence.json`.
- Runtime remains standard workspace TinixCut PID 12440, Vite PID 36536 (HTTP 200), and installed G57 candidate PID 27424. No code changed or runtime restarted.

### G53 fresh native caption-visibility recheck — 2026-11-05

- Read-only UIA found the workspace app's recovery modal. Invoked its unique **Later** control only; SHA-256 values and inventory for all 13 `.unsaved.tinix` snapshots were identical before/after. No Restore or Discard action occurred.
- Copied the isolated new-ID fixture into the picker’s current disposable directory and opened its exact unique ListItem. TinixCut reported `Core Healthy`, project `Phase 53 preview recheck`, revision 36. The accessible frame field read back 60 and Go moved the preview to `00:00:02:00` (frame 60); UI reported two active layers. No project edit or save was made.
- Fresh native screenshot `artifacts/phase53/disposable/recheck_20261105/native_preview_frame60_recheck.png` (SHA-256 `E45688448FE92B41442D7932D7A9CCFE017709A128F2BA96DF08967834AD5BB0`) shows the source-burned Vietnamese subtitle but not the editable `Phase 53 caption after fix`. The timeline still contains the text clip; the same project’s prior export frame shows the editable caption. Original saved fixture hash (`1C716E...`) and source media hash (`EF2400...`) are unchanged.
- **G53 remains OPEN / preview-export visibility mismatch reproduced.** This fresh run confirms the product issue, but does not establish its frontend rendering root cause. Do not claim preview parity or make a speculative renderer change. Full attempt history, hashes and runtime are in `artifacts/phase53/disposable/recheck_20261105/native_recheck_evidence.json`.

### G53 square-background caption preview candidate — 2026-11-05

- Root-cause path narrowed from the captured project style: the reproduced caption uses a semi-transparent square background (`#111827`, opacity 0.82, radius 0). The frontend routed only rich/template/rounded-background text through the shared native raster overlay, so this legacy caption used the DOM text route while export rendered it separately.
- Added a preview-specific legacy `TextStyle`→`TextRichStyle` conversion and raster route for visible text backgrounds. Export behavior remains unchanged. Rasterized background captions no longer expose transient CSS canvas-transform handles, avoiding a stale-bitmap drag state.
- Regression evidence: `preview_rasterizes_legacy_text_with_square_background` passes (1/1), including valid PNG and nontransparent pixels. Phase 30 integration passes 11/11 active tests (one evidence-only test ignored); frontend 168/168, typecheck, build, `cargo check`, `cargo fmt --check`, and `git diff --check` pass.
- Restarted only the workspace development runtime (old workspace TinixCut/Vite processes replaced); the new app runs from `src-tauri/target/phase53-g53-isolated/debug/tinixcut.exe` and Vite returns HTTP 200. Native screenshot showed Core Healthy. Startup opened a different disposable/recovery project, not the G53 caption fixture. Computer Use is unavailable and the previous native path-entry route did not verify input, so no new visual preview claim is made.
- **Disposition: G53 remains OPEN** until the exact caption fixture is opened in the restarted runtime and the editable caption is visibly present at frame 60. Machine evidence: `artifacts/phase53/disposable/recheck_20261105/g53_preview_raster_fix_candidate_20261105.json`.
### G53 native recheck blocked by window identity mismatch — 2026-11-05

- Created a separate-ID copy of the exact caption fixture at `artifacts/phase51/disposable/g53_preview_raster_fix_recheck_20261105.tinix` (copy SHA-256 `765B4E8E153676DDB37FBFC58DBBE281AE27E4A47A1E896DF1C5B4714A629C63`). The source fixture hash remains `1C716E57971F2442F3BB518EF164B752E47ABA246DCCE913D0062B56670AD8AD`; its media path exists.
- Windows UI Automation found workspace PID 25512 and a unique `📂 Open` button; its read-only tree showed `Untitled Project rev0`. One `InvokePattern` call produced no native file picker. The latest desktop screenshot instead showed a TinixCut window at `Untitled Project rev5` / 9:16 with another black TinixCut window behind it. The mismatch is material, and desktop also exposes active Computer Use overlay windows (PID 11920, `ChatGPT is using your computer. Esc to cancel`). No further input was sent.
- No project was opened, edited, or saved; no recovery copy was selected. **G53 remains OPEN**: automated candidate passes, but native frame-60 visual confirmation is blocked until screenshot, UIA and target window bind consistently and the overlay is no longer controlling input.
### Exact workspace-window capture resolves screenshot association — 2026-11-05

- Captured the main HWND `280169270` for workspace TinixCut PID `25512` directly with Win32 `PrintWindow`; screenshot SHA-256 `00EC3343A84BBBA098FD495BE28A4633D35BE06993A26A0378DD67D65376EF56`. It agrees with the PID-scoped UIA tree: Core Healthy, `Untitled Project` revision 0, 16:9 canvas.
- The window contains an **Unsaved projects recovered** dialog. Visible cards are unrelated Phase 44 and Phase 57 projects; no G53 caption fixture is identified. No Restore or Discard action was sent. The earlier full-desktop screenshot showing rev5 belongs to a different TinixCut window and is not valid evidence about PID 25512.
- A `codex-computer-use.exe` host (PID 11920) is still running, while this session has no native Computer Use callable tool. One accessible Open invocation did not create a file picker. No further input, recovery selection, or project mutation occurred. Native G53 frame-60 confirmation remains OPEN.

### G53 native fixture-open input check — 2026-11-05

- On workspace TinixCut PID 25512, invoked the unique accessible `📂 Open` control through UIA `InvokePattern`. The call returned without exception, but after two seconds no native picker/modal appeared; readback remained on the disposable Phase 51 project rev0 at `00:00:36:16`.
- No project edit, save, or recovery action occurred. G53 remains OPEN; the caption-preview fix candidate has not been confirmed on the matched native fixture in this runtime. Computer Use is not callable in this session, and this no-op Open invocation is not retried.
- Evidence: `artifacts/phase53/phase53_native_open_input_recheck_20261105.json`.
### G53 updated-runtime preview/export parity retest — 2026-11-05

- On the disposable `G53 Preview Raster Fix Recheck` project at revision 36 and frame 60, the native screenshot still shows the source-burned Vietnamese subtitle but not the editable `Phase 53 caption after fix` text. The corresponding export frame contains that editable caption.
- The updated frontend HUD reports `Layers: 2`, `Text overlays: 1`, `Blank: 0`, `Overlay errors: 0`, and `Superseded: 1`. Thus the preview overlay request returned nonempty raster data without a surfaced error; this does not establish that the WebView placed or composited it correctly.
- Screenshot: `artifacts/phase53/native_ui/g53_preview_frame60_after_frontend_restart_20261105.png`, SHA-256 `'+$hash+@'`.
- **Result: G53 FAIL reproduced / OPEN.** The candidate did not restore visible preview/export parity. Root cause remains unproven; investigate ROI/DOM bounds and WebView compositing before changing render behavior. Fixture remains unsaved; no recovery copy was selected, restored, or discarded.
### G53 ROI diagnostic HUD and runtime IPC blocker — 2026-11-05

- Added a read-only HUD value showing raster ROI bounds and the project canvas/scale to distinguish a placement error from a WebView composition issue. TypeScript typecheck and production build pass; the existing bundle-size advisory remains (576.13 kB JavaScript chunk vs 500 kB).
- Restarted the exact workspace runtime through `pnpm tauri dev`; the app runs as PID 34184 and Vite PID 40276 returns HTTP 200 and serves the new HUD source. The restart opened `Untitled Project rev0`, not the saved G53 fixture. Recovery chooser was dismissed with **Later** only; no recovery copy was restored or discarded. The G53 fixture SHA-256 remains `765B4E8E153676DDB37FBFC58DBBE281AE27E4A47A1E896DF1C5B4714A629C63`.
- During Vite startup, the runtime log reported `TypeError: Cannot read properties of undefined (reading 'invoke')` in `getHealth`, `getCurrentProject`, and `samplePreviewAt`. After app launch the window showed the empty-project UI without a visible Core Healthy badge. This means the ROI HUD could not yet be read on the G53 fixture and runtime health is **not confirmed**.
- Exact-window screenshot `artifacts/phase53/native_ui/g53_preview_frame60_roi_diagnostic_20261105.png` (SHA-256 `'+$hash+@'`) confirms the new ROI HUD renders but shows `ROI: none` because the current project is empty. A single UIA invocation of the unique Open control produced no picker; no further input was sent.
- **Disposition: G53 remains FAIL reproduced / OPEN; runtime IPC is additionally BLOCKED.** Restore working Tauri IPC and a Core Healthy workspace runtime, then reopen the saved disposable fixture to inspect ROI and verify preview visibility. Do not alter recovery snapshots or infer the overlay coordinates from this empty-project capture.
### G53 current runtime and IPC provenance correction — 2026-11-05

- A PID/HWND-scoped `PrintWindow` capture of workspace TinixCut PID 34184 / HWND `0x20700886` shows **Core Healthy (v0.1.0)** and the ROI HUD rendering on `Untitled Project rev0`. Screenshot: `artifacts/phase53/native_ui/current_workspace_window_20261105.png`, SHA-256 `74A42529B640D0B6BD3D5C336685098A65F2BA8CCE591B29313E5F8343025485`.
- **Correction:** the `invoke` exceptions previously attributed to the native runtime were actually captured from a standalone Paseo browser tab at `http://localhost:1420/`, with no Tauri internals. The provenance record is `artifacts/phase54/phase54_invoke_warning_provenance_20261105.json`. The native IPC blocker is retracted; current native health is confirmed.
- The app currently has an Open dialog for the disposable G53 folder and the exact `g53_preview_raster_fix_recheck_20261105.tinix` row is selected. The Computer Use overlay PID 11920 owns the foreground interaction and no callable Computer Use tool is available in this session, so no Open action was sent. No project or recovery snapshot was changed.
- G53 remains **FAIL reproduced / OPEN** because the saved frame-60 preview previously omitted the editable caption while export included it. The missing item now is a matched native fixture capture/ROI inspection, not an established IPC failure.
### G53 legacy-text keyframe mismatch isolated — 2026-11-05

- A read-only Rust diagnostic loaded the exact disposable revision 36 fixture. At frame 60, `PositionY` samples to **+55 px** while the base transform is **−100 px**. The preview renderer returns ROI `[0,1079,392,1080]`: only the bottom pixel row intersects the canvas. A nonempty PNG/`hasVisibleContent` therefore did not prove legible caption content.
- For the same composition, the full-export graph emits legacy `drawtext` at `y='h-th+-100.0'`, while the single-frame graph emits `y='h-th+55.0'`. Full export bypasses the keyframe for this legacy text path. This explains the observed preview/export difference on the saved fixture; it does not establish that other preview mismatches share this cause.
- Evidence: `artifacts/phase53/g53_legacy_text_keyframe_diagnosis_20261105.json` and `artifacts/phase53/native_ui/g53_rust_overlay_frame60_roi_20261105.png`. Fixture SHA-256 is unchanged. G53 remains **FAIL / OPEN** until the legacy text animation path is corrected and preview/export parity is rechecked.
### G53 native ROI confirmation and input recovery — 2026-11-05

- The previously asserted foreground ownership by the Computer Use overlay is retracted: a direct foreground-window query showed the exact `Open TinixCut Project` dialog of TinixCut PID 34184 in foreground. Its selected disposable fixture row exposed `InvokePattern`; one invocation loaded `G53 Preview Raster Fix Recheck` revision 36 with Core Healthy. No recovery restore/discard or save was used.
- `Go to frame` ValuePattern read back 60 before the unique Go control was invoked. PID/HWND-scoped screenshot `artifacts/phase53/native_ui/g53_frame60_roi_native_20261105.png`, SHA-256 `69D80DE31EB84C32626EA4AA7B35C04AE8E9DB41B413C10B9CB61671F216A88F`, shows two layers and the ROI beginning `0,1079–392,...`, consistent with the independently measured `[0,1079,392,1080]` one-row overlay. This confirms the diagnosis on the native current fixture.
- Input recovery does not close G53: full-export legacy drawtext still ignores the PositionY keyframe on this candidate. The source and disposable fixture SHA-256 values remain unchanged.
### G53 keyframed legacy text fix and verification - 2026-11-05

- Keyframed or animated legacy text now uses the shared raster composition for preview and full export; unkeyframed legacy text retains its prior export path. Animated PNG frames use lossless compression to keep the existing temporary storage bound practical.
- Targeted Rust regression: 3/3 including MP4 frame comparisons; compressed PNG: 2/2; frontend: 171/171; typecheck, build, cargo check and formatting passed. Independent review returned LGTM without independently rerunning tests.
- Real revision 36 disposable fixture exported a 15 s, 450-frame H.264/AAC MP4; ffprobe and full FFmpeg decode passed. Source fixture hash remained unchanged. Total debug export wall time was 35.84 s; this is not evidence of meeting G54 performance goals.
- Restarted the owned workspace runtime: TinixCut PID 26364 reports Core Healthy via UI Automation, Vite PID 28412 returns HTTP 200. Recovery chooser remains open; no recovery copy was restored or discarded. Native preview/export parity on the restarted runtime and the broader G53 gate remain OPEN.
- Evidence: `artifacts/phase53/g53_keyframe_fix_verification_20261105.json`.

### G53 updated-runtime native frame 60 - 2026-11-05

- On restarted PID 26364, selected Later in the recovery chooser without restoring or discarding a copy. Opened the exact disposable revision 36 fixture by its named file-picker row; frame input read back 60 before invoking Go. Core Healthy and ROI starting `0,1079-392` were observed.
- Native screenshot `artifacts/phase53/native_ui/g53_frame60_fixed_runtime_20261105.png` (SHA-256 `16BFFA49FE4E1EE51325312E31B03F15BF649676CAA1BBF91BF1751298236780`) agrees with the fixed export on the observed caption placement: the editable text is clipped at the canvas edge by its +55 px keyframe; source-burned subtitles remain visible. The fixture hash was unchanged. This single frame does not close the complete G53 gate.

### G53 preview/export caption visibility recheck — 2026-11-12

- On the current build, the G52 parity fixture (legacy text `G52 Hội An – kiểm tra` with a PositionY keyframe 55→−200, EaseIn) shows the caption in the native preview and in the export at all five sampled frames (30, 90, 150, 209, 210); the Vietnamese diacritics render in both. The earlier preview-missing/export-present mismatch does not reproduce. See `artifacts/phase52/disposable/g52_parity_20261112/side_by_side_all.png` (pre-fix export column differs only in layer geometry, fixed under G52).
- Caption edit/split/merge/reorder with Undo/Redo, SRT/WebVTT round trip, transcript-derived ranges and analysis cancel were not re-run natively in this pass. **G53 stays OPEN** for those workflows.

### G53 SRT native round trip and automated caption suites — 2026-11-12

- Native **Export captions** wrote `artifacts/phase53/g53_20261112/captions.srt` (`00:00:01,000 → 00:00:06,000`, text `G52 Hội An – kiểm tra`: frame 30 / 150-frame duration, diacritics intact). Native **Import captions** of that file created a new track with a caption at start 30, duration 150 (exact frame match); Undo removed it.
- Frontend 184/184 and Rust caption/SRT/VTT unit tests (9) pass. Native caption split/merge/reorder, WebVTT, transcript-derived ranges and analysis cancel were not re-run: **G53 stays OPEN**.
