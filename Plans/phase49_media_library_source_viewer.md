# Phase 49 — Media library and source viewer

**Status:** Implementation and automated G49 checks complete; native qualification partially passed for exact-row import, Source Viewer playback/Cancel, and 0–15 s insertion with Undo/Redo. Batch import/cancel, relink, collect, and save/reopen remain open.  
**Updated:** 2026-11-04  
**Plan:** [Product completion, Phases 48–57](plan_product_completion_phases48_57.md)

## Implemented in this checkpoint

- Media library search by name, type/status filters, and deterministic name/type/size sorting. Filtering does not mutate project assets.
- Library cards distinguish audio, image, and video and display file size, dimensions, duration, and offline/relink state.
- Source viewer previews video/audio/image without changing project state. Video/audio controls update a playhead; users set In/Out manually or at the playhead.
- Add selected source range to timeline through the existing `ClipCreate` Command Service operation. In/Out is quantized onto the project frame grid; invalid, non-finite, empty, or out-of-duration ranges are rejected.
- Batch import reports per-file progress, continues after an individual file failure, summarizes partial results, and cancels after the currently-running import.
- Bulk relink requires a mapping for every currently offline asset and submits the mappings as one Command Service transaction. The native relink boundary probes the selected file, rejects incompatible media kinds and sources shorter than existing source ranges, and updates verified path/name/size/duration/dimensions. Audio duration tolerance is one sample at the probed sample rate; video tolerance is one source frame.
- A post-commit offline-list refresh failure is reported as a reconciliation problem, not as a rejected transaction, so the UI does not encourage an unsafe retry.
- Collect media copies each source into a new sibling media directory, writes a SHA-256 manifest, updates the cloned project through one Command Service transaction, and saves the new project atomically. The source project and source files are not moved or edited. The collected project opens while its sibling media directory stays at the selected destination; because asset paths are currently absolute, moving that package to a different location requires relinking.

## Verification

- Frontend typecheck: pass.
- Vitest: 101 tests pass, including media search/filter/sort immutability and source-range quantization/validation.
- Production frontend build: pass.
- Rust focused media-relink tests: 5 pass (verified metadata update, missing target rejection, wrong-kind transaction rollback, too-short video rejection, and audio source 10 ms too short rejection).
- Moved-media persistence/relink regression: pass; Rust binary check and format check: pass.
- Collect-media tests: 3 pass, including opening the collected project while the original media path is unavailable, original-source hash preservation, collision/offline-source preflight, and rollback after a rejected relink transaction.
- `git diff --check`: pass for the current workspace changes (Git emitted only existing LF/CRLF normalization warnings).
- Native UI evidence is recorded below using Windows UI Automation plus screenshots because the Computer Use plugin was unavailable.

## Remaining G49 qualification

- Folder batch import/progress/cancel, accurate source-range scrubbing and insertion beyond the full-range smoke, relink, collect, path control, and save/reopen on disposable media.

G49 remains **OPEN** for these items. Native observations below use Windows UI Automation and screenshots because the Computer Use plugin is unavailable; each result is based on the isolated candidate UI.

### Native file-picker path recheck — 2026-11-04

- On the isolated TinixCut candidate (PID 28840), the native Import Media Files dialog opened at `artifacts/phase44/disposable` and visibly listed the intended disposable video fixture.
- Windows UI Automation exposed no ValuePattern for the filename edit. A focused `SendKeys` attempt produced `phae44hoi_an_viualfixture.mp4` instead of `phase44_hoi_an_visual_fixture.mp4`; the mismatch was detected before pressing Open. The dialog was canceled with Escape while focus was on a file-list item.
- The project remained revision 10 with “No media imported”; Core Healthy remained visible. No file was opened, and no project/media/recovery file was modified. Evidence: `artifacts/phase49/phase49_file_picker_input_blocker.json`.
- The file picker remains BLOCKED for import/relink/collect UI work until a reliable filename-selection route is available.

### Native UI media and export smoke — 2026-11-04

- Using Windows UI Automation (Computer Use was not exposed), imported the disposable Phase 44 fixture into the empty project. The app reported Imported 1 of 1; the preview rendered a frame and the Media Library showed the 15-second 1080×1920 source.
- Added the media to Track 1. Native UI reported a successful transaction at rev 7. Undo removed the timeline clip and Redo restored it at rev 9; the asset remained in the library. The source SHA-256 EF2400D263622DA266D0A378B59BA0290ED7EB6475EDAE46E239D8F110B66A5E matches the pre-existing Phase 44 fixture evidence.
- Native export completed in 2196 ms. ffprobe: H.264 1920×1080 at 30 fps, 450 frames, AAC audio, 15 seconds; full FFmpeg decode passed. Output SHA-256: 75F9B3C73C9B25EA1FED65BD9CF9320357BBC4EA22B78D945C87FDE4AA077517.
- The Save As field was set to artifacts/phase49/native_ui_disposable/phase49_native_export.mp4, but the app wrote to artifacts/phase44/disposable/Untitled Project.mp4. This is a disposable fixture folder; no source file was overwritten. No retry was made. Evidence: [native UI evidence](../artifacts/phase49/native_ui_disposable/phase49_native_ui_evidence.json).
- This smoke covers import, preview, add-to-timeline, clip Undo/Redo, and MP4 export only. Save/reopen, path control, relink, collect, batch import/cancel, and broader native workflows remain open.

### Native source viewer playback — 2026-11-04 (supersedes the picker blocker for this fixture)

- On isolated candidate PID 28840, the common picker initially had no filename ValuePattern and SendKeys corrupted the name; that attempt was canceled before Open and is retained as historical evidence.
- Reopened the picker and selected the exact visible row `phase44_hoi_an_visual_fixture.mp4` using the current screenshot-backed row bounds. UI Automation then reported focus on the exact filename before Enter; TinixCut reported `Imported 1 of 1 media file(s)`.
- Source Viewer showed the imported video, with Source In 0 s and Source Out 15 s. Invoking Play rendered the native video; after a 2-second observation it had reached the 15-second end and the control had returned to `play` (paused/ended state). Screenshot: `../artifacts/phase49/native_ui_disposable/phase49_source_viewer_playback.png`.
- Closed Source Viewer through its exact `Cancel` control, not `Add range to timeline`. The viewer closed, the asset remained imported, and the timeline remained empty. The project was not saved and no recovery copy was changed.
- Fixture SHA-256 remained `'+$hash+'`. Structured evidence: `../artifacts/phase49/native_ui_disposable/phase49_source_viewer_playback_evidence.json`.
- This establishes reliable selection for this visible file row and qualifies import/source preview/cancel. It does not qualify folder batch import/cancel, relink, collect, or adding a selected source range; those remain open.

- Continued the same disposable session: `Add range to timeline` applied the selected 0–15 s source range to Video Track 1 (transaction `tx_1793798090864`, rev 18). Undo removed the added clip/track; Redo restored the track/clip; final Undo returned the timeline to empty. The imported asset stayed in Media Library throughout. No save/export was invoked.

- A second source-range test set Source Out to 5.5 s via the accessible spinner (Source In remained 0). Add range created a timeline item whose UIA duration label was 165 frames at 30 fps, matching the 5.5-second selection. Undo removed that temporary clip/track. This checks one non-full-range quantization case; frame-accurate source scrubbing remains open.

### G49 native collect and reopen - 2026-11-05

- Created a separate-ID disposable variant via MCP from the saved G53 fixture, then used the native Collect project media control and selected the exact new output folder. TinixCut reported one collected media item and opened the collected project with Core Healthy.
- SHA-256 of the collected MP4 equals the manifest and the original source (`ef2400d263622da266d0a378b59ba0290ed7eb6475edae46e239d8f110b66a5e`); the source project hash also remained unchanged. Native Open then reopened the exact collected `.tinix` file and reported the correct project name and Core Healthy.
- This covers one media item and reopen at the original collected location. Batch import/cancel, bulk relink, moving the package and broader save/reopen remain open; absolute collected paths require relink after moving. Evidence: `artifacts/phase49/phase49_native_collect_evidence_20261105.json`.
