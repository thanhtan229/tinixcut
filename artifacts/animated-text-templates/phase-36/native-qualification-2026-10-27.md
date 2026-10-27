# Phase 36 Native Qualification — 2026-10-27

Scope: a partial interactive desktop session plus direct encoder smoke checks. This is evidence for the deferred native gate, not a full Phase 26 sign-off. No application code was changed, so no build or server restart was applicable.

## Desktop session

- App: TinixCut debug desktop (`tinixcut.exe`, v0.1.0); UI reported `Core Healthy`.
- Template browser: rendered 12 of 22 templates, including the user-imported CapCut sample `Variety show sense JP - いざ本番！`.
- CapCut inspector: inspected `draft_content.json` from `TINIX_CAPCUT_IMPORT_TEST`; result `FULL CONVERSION`, profile `StaggeredCharacterIn`, one layer, 1920×1080, rich gradient/stroke/shadow and validated animation fingerprint.
- Apply: created a disposable project, added a track and text clip, applied the imported template, and edited its Title Text parameter to `Tinix — Thử nghiệm tiếng Việt`. UI reported `Applied template: Variety show sense JP - いざ本番！ (v1)` and exposed its one layer/parameter in the Inspector.
- Save: atomic save succeeded to `%LOCALAPPDATA%\Temp\TinixCut-Phase26-qualification\phase26_native.tinix` (resolved path: `C:\Users\Admin\AppData\Local\Temp\TinixCut-Phase26-qualification\phase26_native.tinix`).
- Reopen: opened that file again. One track, one text clip, caption text and template parameter/layer state were present. The persisted template displayed as `capcut_tpl_7522343677012774197` in the inspector. This confirms persistence of the applied content and parameters; template identity/name round-trip should be included in follow-up qualification.
- Undo/redo: not verified in this run. UI automation shortcut/click did not produce observable revision/status change after reopening.

## Encoder smoke checks

Host reports one GPU: NVIDIA GeForce RTX 3090, driver `32.0.16.1742`. FFmpeg on PATH listed NVENC, QSV and AMF encoders.

- `h264_nvenc`: PASS — encoded 30 generated 1280×720 frames to null output, exit 0.
- `h264_qsv`: FAIL on this host — MFX session creation returned `-9`; no Intel GPU was enumerated.
- `h264_amf`: FAIL on this host — `amfrt64.dll` could not be opened; no AMD GPU was enumerated.

This is a direct FFmpeg smoke check, not proof that TinixCut's packaged export path selects and successfully uses each hardware encoder.

## Additional desktop workflow evidence

- Disposable local fixtures were generated under `%LOCALAPPDATA%\Temp\TinixCut-Phase26-qualification\fixtures` (video+audio, standalone WAV, still image, and SRT); media library recognized the video/audio/image and allowed video/audio placement on the timeline. The audio was placed on its own track.
- Imported `captions.srt`: UI reported `Imported 2 caption(s) into a new track`; both cue texts appeared in the timeline.
- Exported captions: UI reported `Exported 3 caption(s)` to `%LOCALAPPDATA%\Temp\TinixCut-Phase26-qualification\captions-export.srt` (two imported cues plus the title item).
- These checks verify media ingest/placement and SRT import/export basics only. Canvas ratio/placement/background, playback/seek/mute, transition/crossfade/speed, integrated MP4 export, and missing-media relink remain unverified. The integrated MP4 export control was not reached in this desktop pass.
- Whisper auto captions: not run; CLI/model availability was not established.
- Clean Windows VM installer/offline qualification: not run. `Get-VM`, `vmrun`, `VBoxManage`, and `qemu-system-x86_64` were unavailable in PATH; `HypervisorPresent=True` only establishes that a hypervisor is present, not that a clean VM is available.
- Hardware encoder matrix: NVENC smoke passed; QSV/AMF require suitable Intel/AMD hardware and runtime. TinixCut packaged export integration remains open.

## Qualification result

Partial native qualification only. Keep GAP-01 (full GUI workflow), GAP-02 (clean VM/offline install), and GAP-03 (packaged hardware-encoder integration) open. Next: continue the Vietnamese Phase 26 checklist with canvas ratios, playback/seek/mute, transitions, integrated MP4 export, missing-media relink, and reliable undo/redo evidence. Then run the installer in a clean Windows VM. Repeat QSV/AMF on corresponding physical hardware.
## Continued desktop checks — 2026-10-27

- Canvas presets: the app committed `9:16 Vertical` and `1:1 Square` (revision advanced to 11). The initial `16:9` state was present; `4:5 Portrait` did not commit through the accessible control. Do not count all-ratio qualification as passed.
- Transport: Step Forward moved the playhead by one frame. Playback advanced the playhead; Pause stopped it. The preview showed generated color bars, but the UI also exposed an `Unable to play media` state during playback, so media playback remains inconclusive.
- Audio track mute: toggled `Mute Audio Track 2`; the control changed to `Unmute Audio Track 2` and revision advanced to 12. Undo was attempted but did not revert the toggle; undo/redo remains unverified. The track was left muted in the disposable project.
- Still open: transition behavior, reliable undo/redo, integrated MP4 export, missing-media relink, all canvas presets, clean-VM/offline installer, and packaged export with each hardware encoder.
- Transition control: invoking `Add CrossDissolve between "Text" and "video_with_audio.mp4"` returned `UNSUPPORTED_TRANSITION_MEDIA` (`participant is not a video item`). This was a text/video pair, so it does not establish whether a video/video transition succeeds; that case remains open.
- Follow-up session: attempted to open the integrated MP4 export control via accessibility; Windows reported `coordinate input geometry is unavailable`. The subsequent screenshot was of the Paseo window rather than TinixCut, so the export dialog could not be safely reached or verified in this pass. No export was started.
