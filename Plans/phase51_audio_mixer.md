# Phase 51 — Audio in video and mixer

**Status:** IMPLEMENTATION AND AUTOMATED ACCEPTANCE COMPLETE; G51 NATIVE ACCEPTANCE OPEN — standalone-audio meter lookup is corrected and regression tested. Native WebView meter, audio-clock and mixer qualification remain open for the final blocker pass.  
**Updated:** 2026-11-05  
**Plan:** [Product completion, Phases 48–57](plan_product_completion_phases48_57.md)

## Baseline audit

- Video items currently always enter the normalized audio composition when their track is not muted. The asset probe/export layer handles silent video streams safely, but there is no per-video embedded-audio enable switch.
- Clip volume, keyframed volume, fades, track mute, and audio crossfade already exist for video/audio items. Standalone audio can be placed on an Audio track.
- No explicit detach operation exists. Manually duplicating a video into an Audio item would cause its embedded audio to remain active, so it could play twice.
- There is no track solo/meter/mixer surface or loudness analysis/normalization flow. These remain distinct follow-up slices; no DSP or external upload is introduced here.

## First implementation slice

First add an explicit `embedded_audio_enabled` project field (default true for older projects), expose it in the Inspector, and route it through Command Service. Then add a single atomic `ClipAudioDetach` operation that creates a standalone Audio item with matching source/timeline/speed/volume/fades/keyframes, disables the original Video item's embedded audio, and chooses an unlocked Audio track without overlap (creating one if needed). Reject invalid/missing/silent assets and preserve rollback semantics. Detach depends on reliable stream probing or metadata; do not assume every Video asset has audio.

## Checkpoint 2026-11-04 — embedded audio control

- Schema v13 adds `TimelineItem.embedded_audio_enabled`; v12 migration defaults it to true.
- Schema v14 adds `Track.solo`; v13 migration defaults it to false.
- `ClipEmbeddedAudioSet` is handled by the Command Service and advertised to automation. The Inspector exposes a default-on checkbox for Video clips.
- `ClipAudioDetach` probes the source using FFmpeg, rejects missing/offline/silent sources, selects or creates a non-overlapping unlocked Audio track, creates a matching standalone Audio item, and disables embedded sound in the same transaction. The Inspector exposes the detach action.
- `TrackSoloSet` is command-backed and exposed as an accessible Solo button in the timeline header. When any track is soloed, normalized audio mutes non-solo tracks while retaining all visual layers; multiple solo tracks remain audible.
- Schema v15 adds `Track.volume` in the canonical linear-gain domain; v14→v15 initializes existing tracks to unity. `TrackVolumeSet` validates finite gain in `[0, 2]`, uses Command Service/history and is advertised to automation. Track and clip faders display dB (`−60` to `+6`), converting at the UI boundary; normalized preview/export composition multiplies track gain after clip keyframes/fades.
- Gain conversion helpers cover mute (`−∞ dB` label / `−60 dB` fader), unity, half amplitude and the positive-gain cap; the track fader is available in each timeline header.
- Normalized composition omits only the embedded audio layer when disabled; the visual Video layer remains.
- Automated checks passed: v12→v13 embedded-audio migration default, v13→v14 solo migration default, v14→v15 gain migration default, command mutation/detach/solo/gain (including NaN rejection), detach success using a generated H.264/AAC fixture, offline-source rollback, composition solo/gain behavior, TypeScript typecheck, 118 frontend tests, frontend production build, `cargo fmt --check`, and native `cargo check`.
- Source audio level analysis now reports FFmpeg `mean_volume` and `max_volume` in dBFS from each online Video/Audio asset. The Media Library exposes the result with the measurement method and source fingerprint; it is not LUFS loudness normalization and does not alter media or project state. Parser tests and a generated AAC fixture check passed.
- Schema v16 adds reciprocal linked video/audio IDs with a lossless v15→v16 migration. Detach creates both link references atomically; moving, group-moving, trimming, changing speed, and deleting a linked item preserve pair consistency. Splitting a linked pair is rejected until the user unlinks it. Inspector offers explicit unlink. `ClipLinkSet` is exposed through Command Service and automation. A command test covers group move of one selected side, mirrored speed, split rejection, explicit unlink, and project invariants. Playback peak metering, reviewed normalization/ducking, voice/music preview/export acceptance, and native UI qualification remain open; native observation stays in the final blocker pass.


### Playback peak meter implementation — 2026-11-04

- Preview playback samples active `<audio>` and `<video>` elements through `captureStream()` when the WebView exposes it. The captured stream feeds an `AnalyserNode` and a zero-gain destination branch; the element's native playback path remains intact.
- The preview displays a master sample peak in dBFS, while Audio tracks and Video tracks with enabled embedded audio display per-track peak bars. Meter values update during playback and clear while stopped.
- Unsupported capture/context paths produce an explicit N/A state. They do not substitute source-file analysis for a playback meter.
- Peak conversion/scale tests pass. Typecheck, 120 frontend tests, production build, and `git diff --check` pass. Native WebView2 capture behavior and voice/music/export acceptance still require native qualification.
## Verification

- Automated: migration v12→v13, default-on behavior for existing projects, command undo/redo and rollback, normalized composition contains exactly one audio layer after detach, and a representative export/audio decode fixture.
- Native mixer/detach UI and full voice/music acceptance remain deferred to the final blocker pass if Computer Use stays unavailable.

### Reviewed normalization and voice-ducking draft — 2026-11-04

- Added an Audio mix action that selects voice/video items and music clips, analyzes each selected music source with the existing FFmpeg mean/peak dBFS analyzer, and builds a deterministic review plan.
- Normalization targets mean dBFS and caps requested gain against a −1 dBFS sample-peak ceiling, then clamps clip gain to the supported 0–2 range. The UI and plan explicitly state this is not LUFS and is based on the full source file, not just the trimmed region; it does not claim true-peak compliance.
- Ducking creates clip-relative volume keyframes with configurable duck level, attack and release. Short gaps whose attack/release windows overlap remain ducked; clips with existing volume keyframes are rejected to avoid silently replacing user automation.
- The proposal is checked by Command Service dry-run, bound to project revision, and applied as one existing edit-plan transaction with undo history. Canceling leaves the project untouched.
- Automated frontend checks: 3 focused audio-mix planner tests; TypeScript check; full frontend suite 123 tests; production build. The bundle-size warning remains (518.65 kB minified).
- Still open: native UI confirmation, voice/music preview/export acceptance, and playback peak-meter behavior in WebView2. These remain queued for the final blocker pass.
### Voice/music export acceptance — 2026-11-04

- Added `phase51_audio_mix_acceptance`, which generates a video with embedded voice plus stereo music using app-owned FFmpeg, exports baseline and ducked MP4s through `ExportEngine`, decodes both to stereo float PCM, and probes the ducked stream with app-owned FFprobe.
- Acceptance verifies AAC, 48 kHz, two channels, decoded sample count within 4096 samples of the 3-second target, unchanged RMS before attack (ratio 1.000), lower RMS during voice (0.729), and recovery after release (1.001).
- The first export exposed a real channel-layout defect: `Project.settings.channels=2` still exported mono when the first mixed input was mono. `CompositionPlan` now carries the project channel count and the final mix explicitly applies its FFmpeg channel layout. The stereo acceptance passes after the fix.
- `cargo test --lib composition::tests`: 19 passed; `cargo fmt --check` passed; Phase 51 export integration passed using isolated `CARGO_TARGET_DIR`.
- Still open: preview audio-clock behavior and native UI/meter confirmation; native UI remains for the final blocker pass while Computer Use is unavailable.

### Audio preview seek tolerance regression — 2026-11-04

- Extracted the existing 120 ms audio-element correction threshold into `shouldSeekPreviewAudio` and added boundary tests. The first test exposed floating-point comparison at an exact 120 ms delta (`4.12 - 4` evaluated slightly above `0.12`), which could trigger needless seeks. The helper now applies a scale-aware floating-point slack; exact-boundary drift stays in tolerance while larger drift seeks.
- Focused helper tests pass (2/2); full frontend suite passes (125/125 across 20 files), TypeScript check and production build pass. Build retains the existing large-bundle warning (519.29 kB).
- Rust library suite passes (279 passed, 2 ignored for an explicitly started local LM Studio); audio mix integration passes (1/1), with stereo AAC and measured RMS ratios 1.000 / 0.729 / 1.001. Native compile and rustfmt check pass.
- This test covers the correction threshold only; it does not prove physical or native WebView playback synchronization. Native meter and preview audio-clock/UI qualification remain open for the final blocker pass.

### Native audio UI smoke — 2026-11-04

- On the imported disposable video, Analyze audio levels rendered Mean −31.3 dBFS · Peak −23.0 dBFS. The UI labels this FFmpeg volumedetect, not LUFS.
- Track 1 gain slider moved from 0 dB to −6 dB; the app committed rev 10. Undo restored 0 dB and the video remained on the track. The source fixture was not changed.
- This covers read-only level analysis and track gain/history only. Audio clip trimming, fades/crossfades, automation, and perceived preview/export loudness remain unqualified.
### Native embedded-audio playback meter recheck — 2026-11-04

- FFprobe confirmed the disposable Phase 44 fixture has 15 seconds of H.264 video and AAC stereo at 48 kHz; FFmpeg `volumedetect` measured mean −31.3 dBFS and peak −23.0 dBFS.
- With the clip playing in the isolated native candidate, the preview playhead advanced to frame 38 and the control changed to Pause. The playback meter reported **N/A** with HelpText `Live peak meter is unavailable in this WebView`; the preview did not label the source as silent. Screenshot and structured observation: `../artifacts/phase51/native_ui/phase51_webview_meter_unavailable.png` and `.json`.
- The Add-to-Timeline transaction was undone while playback was active; removing the track stopped preview playback. The unsaved project ended with no timeline track; the imported media remained in the library and its source was not changed.
- This qualifies explicit unsupported-state presentation and playback transport only. Live peak measurement and preview audio-clock/physical output remain unqualified on this WebView runtime.

### Separate video and audio playback failure reporting — 2026-11-04

- Found that a rejected `HTMLVideoElement.play()` promise was written into `audioPlaybackError`, causing video start failures to display under `Audio playback failed`.
- Added a separate `videoPlaybackError` state, clear it on successful video start, and render a `Video preview playback failed` alert. Audio element decode/start failures continue to use the audio alert.
- Validation: `pnpm exec tsc --noEmit` passed; frontend tests **150/150** passed; `pnpm build` passed (existing 568.98 kB chunk warning).
- Runtime: restarted the isolated workspace `pnpm tauri dev` target after the final code change. New isolated binary PID 28192 reports Core Healthy; Vite returns HTTP 200; the built bundle contains the new video-specific message. The global recovery prompt is currently open, so no recovery item was selected and no fixture-based playback reproduction was attempted. Standard TinixCut PID 33680 and Paseo were not stopped.
- This corrects error attribution but does not close G51's native audio-clock or playback-meter qualification.

### Playback error mapping regression tests and final runtime reload — 2026-11-04

- Extracted the audio/video label and empty-error fallback into `src/components/previewPlaybackErrors.ts` with three regression tests. Video rejection stays in the video channel; audio diagnostics remain audio-specific; blank browser messages receive a useful fallback.
- Final validation after this change: focused tests **3/3**, full frontend suite **153/153**, typecheck and production build pass. Existing bundle-size advisory remains; `git diff --check` passes with only existing LF/CRLF warnings.
- Final isolated runtime restart replaced the prior candidate with PID 28596 from `%TEMP%\tinixcut-phase55-runtime-target`; Vite PID ownership changed and `http://localhost:1420/` returns 200. UIA reports Core Healthy on both the new candidate and standard TinixCut PID 33680. Both show the global recovery prompt; no Restore/Discard action was taken. Paseo remains running.
- G51 still needs the audio-clock, meter and playback qualification on a disposable project after a safe recovery path is available.

### Native M4A transport smoke — 2026-11-04

- The `.m4a` picker-filter correction was verified in the current isolated native candidate: the 30-minute AAC-LC fixture appeared in Media Library, and `+ Timeline` created an Audio Track 1 clip (54,001 frames) on a disposable unsaved project.
- Play advanced the timeline to `00:01:31:02`; Pause returned the transport button to Play. This verifies the transport path and imported audio asset, not physical audible output.
- The preview peak meter remained at `-60 dBFS` while the fixture measures `-32.8 dBFS` peak over the first 5 seconds. Meter behavior and audio-clock/output qualification remain OPEN. Evidence: `../artifacts/phase54/native_ui/phase54_m4a_audio_transport_evidence.json`.

### G51 implementation checkpoint — 2026-11-05

- Source/automated acceptance for this phase is complete: linked detach and edits, solo/gain, level analysis, reviewed normalization/ducking, and voice/music export have regression coverage; the latest full frontend run passed 155/155 and includes the meter/gain tests.
- Native evidence has two distinct outcomes: an embedded-audio WebView reported N/A because `captureStream` was unavailable; a later standalone M4A transport advanced but the peak meter stayed at −60 dBFS against a −32.8 dBFS FFmpeg source peak. Neither observation proves audible output or live-meter accuracy.
- G51 is therefore **not fully closed**. Remaining work is native playback/mixer/audio-clock and perceived preview/export acceptance; these depend on reliable native interaction and an observable audio-output path. The source/export implementation is not marked blocked by the native-only gate.

### G51 native follow-up — 2026-11-05

- On the separate-ID Phase 51 project, the embedded-audio checkbox started On. Native UIA toggled it Off and Undo/Redo/Undo read back On/Off/On. Detach then created one Audio clip, disabled the source Video clip audio, and the final Undo restored the original 2-video/0-audio state. The saved fixture stayed at revision 8 and SHA-256 `CA1E131870B6B843767F849F333B45F7BBB918D2FC16D4B3C5CFC629D40A7DBD` throughout.
- The AAC source is 48 kHz stereo. FFmpeg `volumedetect` reports mean −31.3 dBFS and peak −23.0 dBFS. During five native playback meter reads, the control stayed at −60 dBFS and changed to `Meter idle` after Pause. This is a live-meter discrepancy; it is not evidence that the source is silent. The audio-level UI fields remained blank after invoking Analyze audio levels; the Media Library text row did not expose a selection pattern, so that UI analysis is **not qualified**.
- Native UI control/history **PASS**. Meter, source-analysis UI and audible output/audio-clock **OPEN / NOT PASSED**. No physical audio output capture was available. Existing automated voice/music export acceptance remains valid; no new native export was run here.
- Structured readback: `../artifacts/phase51/phase51_native_audio_controls_meter_20261105.json`. Runtime during qualification: standard workspace TinixCut PID 35856, Core Healthy; Vite PID 9112 returned HTTP 200. No code changed and no process was restarted.

### System loopback recheck — 2026-11-05 (inconclusive)

- On the disposable `Phase 51 Loopback Recheck` project (revision 8), Windows WDM-KS Stereo Mix captured 4 seconds at 44.1 kHz stereo: 355,200 samples, 200 callbacks, peak −40.59 dBFS, RMS −48.81 dBFS, no callback flags. An earlier paused 1-second baseline measured peak −66.6 dBFS; the interval is about 26.0 dB higher.
- FFmpeg measured the first four seconds of the fixture at mean −31.5 dBFS / peak −23.0 dBFS. This comparison does not establish equivalent gain because Stereo Mix is system-wide and uncalibrated.
- UIA invoked the exact `▶ Play` button, but post-capture readback still showed `▶ Play` and timecode `00:00:00:00`. Therefore the loopback signal cannot be attributed to TinixCut playback. No raw audio was retained.
- **Disposition: INCONCLUSIVE; G51 remains OPEN.** This does not qualify the live meter, preview audio clock, audible output, or physical A/V timing. Project/media source hashes remained unchanged. Full evidence: `artifacts/phase51/phase51_system_loopback_attempt_20261105.json`.

### Confirmed native playback loopback check — 2026-11-05

- On the disposable Phase 53 UI fixture, UIA invoked the exact `▶ Play` control. Readback changed to `⏸ Pause`; the playhead advanced from frame 16 during capture to frame 149 after four seconds. The source fixture is H.264/15 s/30 fps/450 frames with AAC 48 kHz stereo; project/media hashes are recorded in the JSON.
- One-second paused Stereo Mix baseline: peak −78.68 dBFS, RMS −93.08 dBFS. Four-second capture during confirmed playback: peak −40.42 dBFS, RMS −48.55 dBFS (peak +38.26 dB over baseline); 176,400 samples at 44.1 kHz stereo. Raw audio was not retained. The UIA playback peak meter remained at −60 dBFS while playhead advanced.
- The capture is system-wide WDM-KS Stereo Mix, not process-isolated. It correlates with confirmed TinixCut playback but cannot rule out unrelated concurrent system audio. A later read-only snapshot showed Play/frame 0; the reset timing was not isolated. This improves evidence for digital output but does not qualify the internal meter, audio-clock synchronization or linked-clip behavior. **G51 remains OPEN.**
- Evidence: `artifacts/phase51/phase51_confirmed_playback_loopback_20261105.json`. TinixCut PID 12440 stayed Core Healthy; Vite PID 40396 returned HTTP 200. No code change or restart.

### Standalone audio peak-meter lookup fix — 2026-11-05

- Source inspection found the meter loop looked up `videoRefs` twice and never selected a standalone `<audio>` element. This explains why a native M4A clip could play while the meter remained at its −60 dBFS floor; it does not explain the separate WebView `captureStream` unsupported result for embedded video audio.
- Added `getPlaybackMeterElement`, which prefers the standalone audio ref and falls back to the video ref for embedded audio. Added focused regression coverage for both paths and the missing-element case.
- Focused meter tests pass **3/3**, TypeScript typecheck passes, production build passes, and `git diff --check` passes. Vite was restarted as PID 41904; `localhost:1420` returns HTTP 200 and the served helper contains the audio-first lookup.
- **G51 remains OPEN:** native retest of M4A peak readings, internal meter accuracy, audio-clock synchronization and linked-clip behavior is still required. The fix and automated checks alone do not qualify playback metering.
- Evidence: `artifacts/phase51/phase51_audio_meter_ref_fix_evidence_20261105.json`.

### Standalone-audio meter native retest attempt — 2026-11-05

- Prepared a separate-ID one-audio-track project using the 30-minute AAC fixture. The project file hash is `5C319A548FBBBF3993E60328D86DEB6E4D99A7FAA6D5931B31B265A1F4E7ED49`; source media hash is `1A6EE8F58904F54B84F2D7CDF12DA1F8D343085DCE6AEFECA0F7A6EB2461A525`.
- The first exact Open control did not load the fixture; it was replaced by a second native picker. A fresh screenshot of the second picker showed the exact `artifacts/phase51/disposable` directory. UI Automation selected the one visible `phase51_standalone_audio_meter_retest_20261105.tinix` row; the screenshot confirmed its full filename in the File name field. The Open control did not expose `InvokePattern` (`Unsupported Pattern.`). `BM_CLICK` to the second picker’s exact Open HWND `0x470C00` closed the picker, but the main UI remained `Untitled Project` rev0 and the existing unsaved-recovery chooser stayed visible. No recovery item was restored or discarded.
- **Historical disposition: BLOCKED / NOT TESTED; superseded by the later G51 standalone meter native retest below.** This earlier attempt did not load the project; the subsequent recovery-chooser dismissal and native Open succeeded.
### G51 standalone meter native retest — 2026-11-05

- The earlier open blocker was superseded: the recovery chooser was safely dismissed with **Later**; no Restore/Discard action was used. A subsequent native Open loaded `Phase 51 Standalone Audio Meter Retest` (rev 0, one audio track/clip). Recovery snapshot hashes remained unchanged.
- On the exact disposable fixture, native Play advanced from frame 0 to `00:00:03:20` (~frame 110); Pause stopped transport. The peak meter continued to display **Meter idle** and `-60 dBFS` during playback. No project save was made. A later read-only UIA check found the transport paused at `00:00:36:16`, stable across two snapshots four seconds apart; the meter still read −60 dBFS. FFmpeg measured the source segment at 36 s mean −36.3 dBFS / peak −29.0 dBFS. Project SHA-256 `5C319A548FBBBF3993E60328D86DEB6E4D99A7FAA6D5931B31B265A1F4E7ED49`; media SHA-256 `1A6EE8F58904F54B84F2D7CDF12DA1F8D343085DCE6AEFECA0F7A6EB2461A525`; neither changed.
- Source review found a plausible but unconfirmed status path: if `captureStream()` returns a stream with no audio tracks, the meter loop continues without marking the source unsupported, leaving the meter at idle. A bundled FFmpeg `volumedetect` run over source audio at 3.6 s measured mean −40.4 dBFS / peak −31.2 dBFS (clip volume 0.35; theoretical peak after that gain ≈ −40.3 dBFS). The native observation did not expose the captured stream track count, so the capture-stream branch remains a hypothesis, not a confirmed root cause.
- **Disposition:** transport PASS; accurate live meter FAIL / NOT QUALIFIED; audio clock and linked-clip interaction remain open. The inability to open the fixture is no longer the active blocker. Full evidence: `artifacts/phase51/phase51_standalone_meter_native_retest_20261105.json` and `artifacts/phase51/native_ui/phase51_standalone_project_loaded_20261105.png`.
- Runtime remains TinixCut PID 25512 Core Healthy; Vite PID 41904 returns HTTP 200. No code changed, so restart is not applicable. The unsaved disposable project remains open and paused; do not save it.
### G51 explicit unsupported-meter status — 2026-11-05

- Fixed the meter state path so an absent/unsupported audio capture source no longer leaves the control labeled idle. This change does not alter playback routing. The focused `audioMeter.test.ts` suite passes **4/4**; typecheck and production build passed, and `git diff --check` passed.
- After restarting only the workspace Vite server, UIA reopened the disposable standalone-M4A project without restoring/discarding recovery data. During confirmed Play, the meter stayed at **−60 dBFS** and the control HelpText changed to **“Live peak meter is unavailable in this WebView”**. After Pause, it returned to the generic meter description. This confirms the new unavailable state is surfaced; it does not identify which unsupported source condition occurred or prove the source peak is measured.
- Project and media SHA-256 remained `5C319A548FBBBF3993E60328D86DEB6E4D99A7FAA6D5931B31B265A1F4E7ED49` and `1A6EE8F58904F54B84F2D7CDF12DA1F8D343085DCE6AEFECA0F7A6EB2461A525`; project was not saved. Runtime: TinixCut PID 25512 Core Healthy; Vite PID 38324, HTTP 200; served `PreviewCanvas.tsx` contains the unsupported status and track-detection helper.
- **Disposition:** clearer native failure state PASS; accurate live meter, audible-output, audio-clock and linked-clip acceptance remain **OPEN**. Full evidence: `artifacts/phase51/phase51_meter_unsupported_status_native_retest_20261105.json`.
- Follow-up accessibility correction: `aria-valuetext` now reports `N/A` for unsupported capture instead of saying `Meter idle`. Added regression coverage for unsupported/idle/measured text; focused meter tests are now **5/5 PASS**. Typecheck, production build, Vite restart and served-source check all pass. Latest Vite PID 39200 serves HTTP 200; TinixCut PID 25512 remains Core Healthy. Evidence JSON above records the final runtime/test state.
### G51 linked slip/roll source inspection — 2026-11-06

- Read-only inspection: `ClipSlip` updates the requested item's source range; `ClipRoll` updates the two same-track items. Neither handler visibly updates their linked counterparts. `Project::validate` requires linked video/audio items to have equal source and timeline ranges, and `CommandService::execute` validates the staged project after each operation. Therefore a slip/roll involving linked clips is expected to fail validation and preserve the committed project; this is a source-derived hypothesis, not a runtime reproduction.
- G51 linked-clip behavior and G50 roll remain OPEN. Next verify a disposable linked pair through Command Service, including revision/state preservation on rejection, before choosing whether to mirror the edit or surface an explicit unlink requirement.
### G51 linked slip/roll reproduction — 2026-11-06

- Created a separate-ID variant of the Phase 50 roll fixture and applied `ClipAudioDetach` via MCP. A one-frame `ClipSlip` of the linked video and a one-frame `ClipRoll` involving that video both returned `OPERATION_VALIDATION_FAILED`: linked video/audio ranges must match.
- The revision remained 1 and the project SHA-256 remained `6E76F361B78E17AC76EB4B010013DD140F388D8475EC81BA6F5D4D429DA4278F` after each failed operation. This confirms rollback for this fixture, not general linked-operation support. The initial variant attempt was rejected before mutation because its envelope lacked `client_id` and remains revision 0.
- Evidence: `artifacts/phase51/phase51_linked_slip_roll_probe_b_20261106.json`. G51 remains OPEN. Implement atomic linked counterpart handling with locked-track, source handle, transition, undo and rollback coverage before claiming these operations work for detached audio.
### G51 linked slip/roll implementation — 2026-11-06

- `ClipSlip` mirrors the source range to the linked counterpart and rejects a locked linked track. `ClipRoll` mirrors both resulting ranges to linked counterparts, rejects locked linked tracks, and checks affected linked-track transition/crossfade handles. Both remain staged Command Service mutations, so validation failure discards the transaction.
- Focused Rust regression `phase51_linked_slip_and_roll_preserve_pair_and_reject_locked_track` passed 1/1; existing `phase50_clip_` tests passed 5/5, `cargo check --bin tinixcut`, `cargo fmt --check`, and `git diff --check` passed. The focused test covers two linked pairs and locked-track rollback; it does not cover every transition, Undo/Redo, or native interaction.
- Fresh MCP run on a separate-ID variant: detach revision 0→1, slip 1→2, roll 2→3, all applied; persisted linked video/audio source and timeline ranges match. Evidence: `artifacts/phase51/phase51_linked_slip_roll_probe_c_20261106.json`. Source fixture SHA-256 stayed `03A03C1AA760CC1ABC291BA952E13CB040F5DA6845CBCB3BAC771498EA5B150C`.
- Replaced the workspace runtime after the final source change: TinixCut PID 45568 reports Core Healthy, Vite PID 46344 returns HTTP 200; its new process and binary were built after the change. The previously open unsaved roll qualification UI session was interrupted; its timeline semantics had been undone to the saved ranges and the existing recovery copies were preserved. The new app is Untitled rev0 with the recovery chooser open. Native linked interactions, Undo/Redo, and broader transition/crossfade qualification remain OPEN; G51/G50 are not closed.
### G51 linked history and crossfade qualification — 2026-11-06

- Extended `phase51_linked_slip_and_roll_preserve_pair_and_reject_locked_track` with a video CrossDissolve and HistoryManager snapshots. Slip and roll preserve the transition and linked ranges; two Undo operations restore both earlier track snapshots and two Redo operations restore the final tracks. The focused test passed 1/1. This is history-engine coverage, not native Undo/Redo qualification.
- On a unique disposable MCP variant with two detached audio clips, `AudioCrossfadeCreate` returned `UNSUPPORTED_TRANSITION_MEDIA` / `UNSUPPORTED_AUDIO_CROSSFADE_MEDIA`. The project SHA-256 remained `4ECDAC6C69ECF7975571A9B6290AAA810ECB1EC82AD6059633CC93CCC7C22796` after rejection. The validator requires `MediaKind::Audio`, while detached audio retains the source video asset. Evidence: `artifacts/phase51/phase51_crossfade_probe_20261106.json`. G51 remains OPEN for detached-audio crossfade support and linked roll with crossfade.
- `cargo fmt --check` and `git diff --check` pass. The Rust watcher rebuilt and relaunched TinixCut PID 23776 after the test source change; accessibility reports Core Healthy with the recovery chooser open. Vite was restarted as PID 32380, HTTP 200. The candidate has not undergone native linked-edit validation.
### G51 detached-video audio crossfade — 2026-11-06

- Command and domain validation now allow Audio timeline items whose source asset is an Audio or Video container. `ClipAudioDetach` probes the video audio stream at detach time; validation of existing crossfades does not probe the file, preserving offline project handling. Video containers without an audio stream are not ruled out for audio items created through other paths.
- The linked slip/roll regression with video CrossDissolve, detached-audio crossfade and snapshot Undo/Redo passed 1/1; existing audio-crossfade tests passed 7/7. `cargo fmt --check` and `git diff --check` passed. The earlier test failed until the matching domain validator was updated.
- On a fresh variant, MCP detached the second clip and created a 10-frame audio crossfade successfully (revision 1→2), superseding the earlier unsupported-media result. Evidence: `artifacts/phase51/phase51_crossfade_probe_fixed_20261106.json`.
- Two export attempts failed with FFmpeg `Error sending frames to consumers: No space left on device` / error -28: first with default temporary directory, second with TEMP and TMP set to a new directory on D:. Both output and overlay inputs in the second log were on D:. Free space after the failure was approximately 5.08 GB on C: and 576.6 GB on D:, so the failing resource and cause remain unconfirmed. Evidence: `artifacts/phase51/phase51_crossfade_fixed_export_20261106.json` and `artifacts/phase51/phase51_crossfade_fixed_dtemp_export_20261106.json`. No successful MP4/full-decode claim is made. G51 remains OPEN for render/audio and native verification.
- Runtime was rebuilt and restarted after the code change: TinixCut PID 36012 reports Core Healthy; workspace Vite PID 44360 serves HTTP 200. The prior Untitled rev0 session was interrupted; recovery chooser remains open and no recovery copy was restored or discarded.
### G51 crossfade export timestamp fix — 2026-11-06

- Controlled variants: removing text retained the FFmpeg ENOSPC failure at frame 231; removing the crossfade as well completed a 15 s H.264/AAC export, and full decode passed. Audio-only filtergraphs with and without unity envelopes passed; the combined A/V graph still failed with unity envelopes and one filter thread. Removing the video frame limit led to invalid/non-monotonic audio DTS and a long-running diagnostic, which was stopped. This narrows the failure to the combined graph/timestamp interaction; it does not prove the exact FFmpeg internal origin of ENOSPC.
- Added `asetpts=N/SR/TB` after `amix` and before padding/trimming, giving the mixed audio a sample-count clock. An isolated diagnostic with this adjustment completed, then the actual MCP project with its real crossfade envelope exported successfully. FFprobe: H.264/AAC, 1920×1080, 30 fps, 450 video frames, 48 kHz stereo, 15.000 s; FFmpeg full decode passed; SHA-256 `9402735029810AFBDFE1CA2B3450B5457E10012C345CE0F905992C50AD472349`. Audio at 7.4–7.7 s was non-silent (mean −52.4 dBFS, peak −37.2 dBFS); this is not a perceptual crossfade-quality test.
- Automated checks: 7/7 audio crossfade library tests, phase51 voice/stereo ducking export integration 1/1, rustfmt and `git diff --check` pass. Evidence: `artifacts/phase51/phase51_crossfade_timestamp_fixed_export_20261106.json`; output `artifacts/phase51/disposable/phase51_crossfade_timestamp_fixed_20261106.mp4`. The earlier ENOSPC attempts and no-crossfade/diagnostic artifacts remain retained.
- Workspace Rust watcher rebuilt and replaced TinixCut PID 36012 with PID 44948 after the source change. New app is Core Healthy; Vite PID 44360 serves HTTP 200. The prior Untitled rev0 session was interrupted by the rebuild; recovery chooser remains open and no recovery copy was changed. G51 remains OPEN for native linked interaction, audible/parity qualification and the live meter.
### G51 native crossfade playback smoke — 2026-11-06

- After preserving the recovery chooser via Later, opened the exact disposable `phase51_crossfade_probe_fixed_20261106.tinix`, project `Crossfade probe` revision 2. UI accessibility reported two video/audio pairs at 0/226 and 226/224 frames and an audio crossfade of 10 frames.
- Set the frame field to 210, clicked Go, confirmed playhead 210, then played. Accessibility observed Pause and playhead 326, past the frame-226 cut. Paused later; the displayed playhead had wrapped to frame 127. The live meter reported `Live peak meter is unavailable in this WebView`. This is transport smoke only; audible output, exact crossfade sound and playback-clock parity are not established.
- Attempting to select the crossfade via accessibility failed with `point (1119, 866) is outside window bounds { originX: 0, originY: 0, width: 1283, height: 831 }`; stopped native input. No project save was invoked; fixture SHA-256 remained `F0EB67606734C3DA124C3E242CDF3FCF603851202F629D3197CC703A6CC320D9`. TinixCut PID 44948 remained Core Healthy and Vite PID 44360 remained present. G51 native linked edit/history, meter and audible parity remain OPEN.
### G51 runtime resume — 2026-11-06

- At resume no TinixCut window or project Vite/Tauri process was present and localhost:1420 timed out. Started `pnpm tauri dev` from this workspace; TinixCut PID 35044 reports Core Healthy and workspace Vite PID 30056 returns HTTP 200. The recovery chooser was deferred with Later; no recovery copy was restored or discarded.
- Opened `phase51_crossfade_probe_fixed_20261106.tinix` through the native file dialog. Project `Crossfade probe` revision 2 displayed the linked video/audio pairs and 10-frame crossfade. Selected the first video clip; its inspector reports source start 1 frame. Native linked slip and Undo/Redo have not yet been performed or qualified. No save was invoked; fixture SHA-256 remains `F0EB67606734C3DA124C3E242CDF3FCF603851202F629D3197CC703A6CC320D9`.
- Computer Use inventory and screenshot-based inputs worked; an initial attempt to set the file-dialog element returned `element 463 is not available in cached app state for tinixcut.exe`, after which a fresh screenshot allowed selecting the exact file. This does not establish that the earlier off-screen timeline geometry failure is fixed.

### G51 native input geometry — 2026-11-06

- Computer Use reselected the existing TinixCut window and confirmed `Crossfade probe` revision 2, selected first linked video clip, source start 1 frame, and Core Healthy. Attempting to click the timeline Zoom In control before navigating focus failed with `coordinate input geometry is unavailable`. No further native input was sent. Linked slip and Undo/Redo remain unverified; this does not establish an application editing failure.

### G51 Computer Use capture mismatch — 2026-11-06

- Rebound the existing TinixCut window: accessibility reported `Crossfade probe` revision 2 and Core Healthy, but the captured image displayed Zalo rather than TinixCut. Sent no native input after observing the mismatch. G51 linked slip and Undo/Redo remain blocked on reliable window capture/input; the previous Command Service and export evidence remains separate.

### G51 native slip focus attempt — 2026-11-06

- With a matching TinixCut screenshot, clicked the selected first video clip, pressed Tab and Alt+Right. The source start remained 1 frame and revision remained 2; the accessibility focus was reported as the WebView pane and a trim-start tooltip appeared. This does not prove the clip received the intended shortcut. Stopped further edit input; no Undo/Redo claim or Save was made.

### G51 delayed slip result correction — 2026-11-06

- The immediately observed revision 2 after the prior Alt+Right was stale. On the next stable observation, TinixCut reported `Slip Clip committed (rev 3) | Recovery saved (rev 3)` and the selected video source start was 2 frames, up from 1. This supersedes the preceding no-change observation; no second slip was sent. The counterpart source range and native Undo/Redo remain unverified.
- A new screenshot for Undo showed the Paseo conversation instead of the bound TinixCut window. No further input was sent. The disposable project was not manually saved; the application reported a recovery save at revision 3.

### G51 native slip undo/redo — 2026-11-06

- On the disposable crossfade project, native Undo of the preceding slip reported `Undid: Transaction tx_1793947589967` at revision 4 and the video inspector returned to source start 1 frame. Native Redo reported `Redid: Transaction tx_1793947589967` at revision 5 and source start 2 frames. Readback was taken after the UI updated; immediate accessibility snapshots were stale.
- The revision 5 project recovery snapshot shows both first linked video and detached audio source ranges start at 2/30 s with duration 226/30 s, and both timeline ranges start at 0 with duration 226/30 s. This confirms matching ranges after Redo; counterpart state after Undo was not independently inspected. The 10-frame crossfade was still present in the observed timeline. No manual project save was invoked; original disposable project SHA-256 remained `F0EB67606734C3DA124C3E242CDF3FCF603851202F629D3197CC703A6CC320D9`. Recovery snapshots were saved by the app at revisions 4 and 5. Core Healthy observed. G51 remains OPEN for broader native editing and audible/parity checks.

### G51 native linked roll and export — 2026-11-06

- On disposable `Crossfade probe`, keyboard roll moved the linked cut from frame 226 to 227 at revision 6; both video/audio pairs showed durations 227 and 223 frames, with the 10-frame crossfade still present. Undo revision 7 returned both pairs to 226/224; the revision 7 recovery snapshot showed corresponding source and timeline ranges matched. Redo revision 8 returned both pairs to 227/223; recovery revision 8 showed matching source ranges (2/227 and 227/223 frames) and timeline ranges (0/227 and 227/223 frames).
- Native export of revision 8 completed to `artifacts/phase51/disposable/phase51_native_roll_redo_20261106.mp4`. FFprobe: H.264 1920x1080, 30 fps, 450 frames; AAC stereo 48 kHz; duration 15.000 s. Full FFmpeg decode exit 0. This confirms file integrity for this fixture, not perceptual audio quality. Project file SHA-256 remained `F0EB67606734C3DA124C3E242CDF3FCF603851202F629D3197CC703A6CC320D9`; no manual project save was invoked. App wrote recovery revisions 6, 7 and 8. Core Healthy observed; G51 remains OPEN for the other audio acceptance items.

### G51 captureStream SecurityError diagnosis — 2026-11-06

- In the workspace TinixCut WebView for disposable `Crossfade probe` revision 8, the DevTools console reported `typeof AudioContext === "function"`, `typeof HTMLMediaElement.prototype.captureStream === "function"`, and `mozCaptureStream === "undefined"`. Audio and video elements both had `readyState` 4 in the initial paused observation.
- After starting native playback, a direct diagnostic call to `captureStream()` on each current audio/video element returned `SecurityError` for both. Playback had been observed in progress earlier; its exact state at the instant of this console call was not captured, so this does not establish a playing-state result. The UI subsequently showed Play at frame 0. The error confirms the unsupported meter's capture exception branch for these elements; the exact security policy and any safe remedy remain unproven. No security settings or source code were changed. DevTools was closed.

### G51 WebView media origins — 2026-11-06

- On the development WebView for the disposable revision 8 project, console observation reported document origin `http://localhost:1420` and both audio/video media origins `http://asset.localhost`; each element reported `crossOrigin: null`, `readyState: 4`, and `paused: true`. Only origins were observed, not media paths. The source uses Tauri `convertFileSrc` and the asset protocol is enabled.
- The media and document have different origins in this development runtime. This is a plausible explanation for the previously observed `captureStream()` SecurityError, but CORS response behavior and an alternate configuration were not tested; no causal fix or production behavior is claimed. DevTools was closed without changing project or security configuration.

### G51 standalone audio CORS change — 2026-11-06

- In the development WebView, a CORS fetch of the scoped fixture media succeeded with status 206 and response type `cors` (the access-control-allow-origin header was not exposed). A separate temporary audio element with `crossOrigin=anonymous` loaded the same source and `captureStream()` returned one audio track without SecurityError. This was a paused temporary element and does not prove meter accuracy.
- Added `crossOrigin="anonymous"` to standalone preview audio elements only. Production build/typecheck passed (existing bundle size warning). Restarted the exact owned Tauri/Vite workspace processes: new TinixCut PID 9992 reports Core Healthy, Vite PID 30052 serves HTTP 200 and the transformed source contains `crossOrigin: "anonymous"`. The prior disposable revision 8 session was interrupted; its recovery snapshot was retained and the recovery chooser deferred with Later.
- Reopened `phase51_standalone_audio_meter_retest_20261105.tinix` revision 0. During playback, playhead advanced through frames 95 and 199 while accessibility reported meter value -60 dBFS and no unsupported message in the sampled observations; playback was paused at frame 497. This does not establish accurate metering or audible output. Existing video-element capture remains unchanged. G51 remains OPEN and the standalone playback change needs further native validation, including a known non-silent signal and installed runtime compatibility.

### G51 fixture level recheck — 2026-11-06

- The standalone fixture contains an audio item spanning 0–600 seconds at gain 0.35, sourced from the synthetic M4A. FFmpeg `volumedetect` on source seconds 3–7 reported mean −38.4 dBFS and peak −29.0 dBFS before clip gain. This proves the source interval is not silent, but does not prove the preview element played that exact interval or that the analyser received samples.
- The current runtime still has the fixture open at revision 0, paused at frame 497 and Core Healthy. The previously observed −60 dBFS meter remains unqualified; analyser sample/context state was not captured in this check.

### G51 analyser inspection attempt — 2026-11-06

- Computer Use bound the TinixCut window and accessibility showed the disposable standalone audio fixture at revision 0, paused at frame 497, with Core Healthy and meter value -60 dBFS. Ctrl+Shift+I opened a DevTools console visible in the window capture, but the DevTools window did not appear in the enumerated top-level windows. An attempted click in its visible console instead activated the underlying TinixCut window; subsequent text was not observed in a DevTools console and no analyser result was obtained. The app still displayed the disposable project at revision 0 and frame 497 afterward. No claim is made about where the text was received.
- Stopped further input rather than treating the attempted diagnostic as a measurement. Context state, capture track state and analyser samples during playback remain unverified; G51 meter acceptance remains OPEN. The prior non-silent source measurement and cross-origin observations do not establish the live meter result.
### G51 DevTools binding recheck — 2026-11-06

- A later Computer Use inventory identified a separate `DevTools - localhost:1420/` WebView window and its accessibility tree exposed the Console prompt. Selecting that prompt returned `coordinate input geometry is unavailable`; the tool call stopped before typing or submitting the diagnostic expression. No analyser samples or playback measurements were obtained. G51 remains OPEN; the earlier failure to locate DevTools is superseded by this more precise geometry blocker.
### G51 keyboard focus attempt — 2026-11-06

- Rebound the inventoried `DevTools - localhost:1420/` window after its console prompt previously returned `coordinate input geometry is unavailable`. Escape opened a drawer; Ctrl+Shift+J opened DevTools for the DevTools window itself, as confirmed by its screenshot. The target console focus was not confirmed and no diagnostic expression or playback action was sent. Further input stopped; the extra developer tools window may remain open. No analyser result was collected and G51 remains OPEN.
### G51 temporary meter observation — 2026-11-06

- Temporarily exposed meter state in the accessibility description through Vite HMR. On the standalone fixture at revision 0 and paused frame 497, the observation reported AudioContext `running`, one active audio item, zero taps, media `paused`, time 16.57 s and readyState 4. Zero taps while paused is expected from the cleanup path; this is not a playback measurement.
- Attempt to select Return to Start through Computer Use failed with `coordinate input geometry is unavailable`; no playback or analyser sample was obtained. Removed the temporary description and verified the source Git blob hash matched the pre-instrumentation hash `e3eadfe0ed987f2173d38c1235a783212b81cba0`.
- Production build/typecheck passed with the existing bundle-size warning. Replaced workspace Vite PID 30052 with PID 42168; HTTP 200 serves the component without the temporary marker. TinixCut PID 9992 remains running; native health was not rechecked after server replacement. G51 remains OPEN.
### G51 continuation check — 2026-11-06

- Rechecked the current TinixCut window through Computer Use. Accessibility reports the standalone audio fixture, revision 0, Core Healthy and playhead frame 0; the corresponding screenshot instead displays the Paseo conversation. No UI input was sent because the image did not match the selected window. Native playback/analyser sampling remains blocked by Computer Use capture/input geometry.
- Re-ran the focused audio helper suites: `audioMeter.test.ts`, `audioGain.test.ts`, and `audioMixPlan.test.ts` — 11/11 passed. This confirms helper behavior only and does not validate WebView capture, live peak accuracy, audible output, or audio-clock synchronization. No application source changed in this check; no runtime restart was needed.
### G51 video media CORS capture enablement — 2026-11-06

- The WebView document runs at `http://localhost:1420`, while local media uses `http://asset.localhost`. Earlier native diagnostics returned `SecurityError` from video-element `captureStream()`. A temporary audio element using `crossOrigin=anonymous` successfully captured one track, and the standalone preview audio element already uses that attribute. Applied the same `crossOrigin="anonymous"` setting to preview video elements so embedded audio capture can use the tested CORS path. This is a targeted compatibility change; native meter accuracy and playback are not yet proven.
- `pnpm build` (TypeScript + production build) passed; `audioMeter.test.ts` and `previewVideoSync.test.ts` passed 7/7. Existing bundle-size warning remains. Replaced the owned workspace Vite PID 42168 with PID 31836; transformed `PreviewCanvas.tsx` returned HTTP 200 and includes `crossOrigin: "anonymous"`. TinixCut PID 9992 remains running and UIA reports Core Healthy on standalone-audio fixture revision 0. The app's selected-window screenshot still showed Paseo content, so no native playback/input was attempted; the running fixture also has no active video element. G51 stays OPEN pending capture-track/analyser samples during playback and installed compatibility.

### G51 screenshot-backed live meter sample — 2026-11-06

- Standard Computer Use selected the unique workspace TinixCut window and captured a matching TinixCut screenshot and accessibility tree. The existing recovery chooser was deferred with **Later**; no recovery was restored or discarded.
- On the disposable `Phase 51 Standalone Audio Meter Retest` project (revision 0), a screenshot-backed click started playback. At frame 303 the UI showed **Pause** and a master/per-track peak of **−31.844944 dBFS**. A later read about 800 ms afterward showed frame 311, **Play**, and the idle floor **−60 dBFS**; playback had stopped between observations. No edit revision or project save occurred.
- The source M4A is 1,800.017 s. FFmpeg decoded stereo samples around source 10.05–10.18 s; applying the fixture's 0.35 linear clip gain gives six 1024-sample window peaks from **−33.471 to −34.552 dBFS**. The WebView sample was not synchronized to the same captured source-time window, so this comparison does not establish meter accuracy or an error bound.
- This is positive evidence that the live analyser can produce a non-floor value during native playback. Sustained repeatability, exact source/gain accuracy, audible-output attribution, playback/audio-clock synchronization, and installed-runtime compatibility remain **OPEN**. Machine-readable evidence: `../artifacts/phase51/phase51_cua_live_meter_sample_20261106.json`.

### G51 timestamp-fixed crossfade export decode recheck — 2026-11-06

- Rechecked the already completed disposable MCP export `artifacts/phase51/disposable/phase51_crossfade_timestamp_fixed_20261106.mp4`: FFprobe reports H.264 1920×1080 at 30 fps, 450 frames, AAC stereo 48 kHz, 15.000 s, 944,179 bytes; full FFmpeg decode exits 0. SHA-256: `9402735029810AFBDfE1CA2B3450B5457E10012C345CE0F905992C50AD472349`.
- This confirms container/stream integrity for the timestamp-fixed export. It does not qualify perceived audio crossfade quality or the live preview meter. G51 remains OPEN.

### G51 screenshot-backed live meter follow-up — 2026-11-06

- Computer Use selected the unique workspace TinixCut window and the screenshot matched the accessibility tree. On the disposable standalone audio project (revision 0), playback produced three non-floor samples with matching master/per-track values: frame 412 **−27.91 dBFS**, frame 683 **−29.73 dBFS**, and frame 842 **−25.70 dBFS**. A later observation showed transport **Play** and **−60 dBFS** after playback stopped. The attempted Pause action was not immediately reflected; no further playback input was sent. Project and source hashes remained unchanged; no save occurred.
- FFprobe identifies the source as stereo AAC, 48 kHz, 1,800.017 s. The fixture starts source at 0 and applies linear gain 0.35. FFmpeg 200 ms windows near the observed timeline positions measured peaks **−33.0**, **−38.8**, and **−32.8 dBFS** after that gain. These estimates differ by roughly 5–9 dB from the UI snapshots, but the analyser's exact sampling window and its source timestamp were not synchronized; this is a diagnostic discrepancy, not a measured error bound or confirmed meter defect.
- This strengthens evidence that native capture can return live non-floor values and return to idle floor. Meter accuracy, sustained repeatability, audible-output attribution, audio-clock synchronization, and installed-runtime compatibility remain **OPEN**. Evidence: `artifacts/phase51/phase51_cua_live_meter_followup_20261106.json`.

### G51 native track mute command confirmation — 2026-11-06

- On the disposable audio fixture, the prior mute action was confirmed on a fresh Computer Use snapshot: project revision 1, the track control changed to **Unmute Audio 1**, and the status bar read "Mute Track committed (rev 1)". This confirms the native mute command committed.
- Playback was stopped and the meter read −60 dBFS. No paired unmuted/muted playback samples were captured, so this does **not** establish that mute suppresses audible output or changes the live meter as expected. No further playback action or project save was performed.
- The UI reported "Recovery save failed: Project already has a canonical path; use its project recovery snapshot". The disposable fixture was not saved or recovered. G51 mute-output behavior and meter qualification remain **OPEN**. Evidence: artifacts/phase51/phase51_cua_mute_track_followup_20261106.json.

- Follow-up: after confirming the muted state, one accessibility click and one screenshot-backed click at the visible Unmute control produced no change (revision remained 1; label stayed Unmute Audio 1). Stopped after these two distinct methods. This is an input-path limitation for this check, not evidence that the app's unmute command is defective; paired playback comparison remains OPEN.

### Native input recheck ? 2026-11-06

- Computer Use selected the unique workspace TinixCut window (HWND 333272); accessibility reported Core Healthy and the saved `Recovery Session Qualification` project revision 1. Selecting the Open accessibility button returned `coordinate input geometry is unavailable`. A subsequent screenshot showed Paseo content while accessibility still identified TinixCut. No further input was sent; the audio fixture was not opened and no new G50/G51 native qualification was obtained.
- Native G50 roll/gestures and G51 paired muted/unmuted playback remain OPEN. The saved G48 fixture and recovery copies were not intentionally changed.
