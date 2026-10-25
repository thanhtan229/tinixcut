# Phase 26 — Consolidated Native Qualification

Vietnamese step-by-step guide: [huong_dan_test_phase26_tieng_viet.md](huong_dan_test_phase26_tieng_viet.md).

Run this once in the restarted TinixCut desktop application after all feature work is complete. Record `PASS`, `FAIL`, or `BLOCKED` for every item, plus the shortest reproduction note for each failure.

## Preparation

1. Create a new project and import one video with audio, one standalone audio file, and one image.
2. Keep a short local `whisper-cli.exe` and compatible model `.bin` available for the automatic-caption items. TinixCut does not bundle either file.
3. Save the project to a new folder so the reopen and missing-media checks use a disposable copy.

## Project, template, and social-video workflow

| Check | Expected result |
|---|---|
| Change 16:9 to 9:16, then 1:1 and 4:5 | Preview canvas and default export dimensions follow each chosen ratio. |
| Set Fit, Fill, and a canvas background on a video | Preview and exported frame use the selected placement and background. |
| Add each built-in text template, edit its text/style, then undo/redo | Text remains editable; undo/redo restores the exact state. |
| Save, close, and reopen | Canvas settings and text/template properties persist. |

## Captions

| Check | Expected result |
|---|---|
| Import a small SRT or VTT file | A dedicated text track appears and each cue is editable. |
| Apply a caption track style/template | Only the selected caption track changes. |
| Export captions, then import the exported file into a copy | Cue text and timing round-trip. |
| Add an audio/video clip, select it, click **✨ Auto captions** | The dialog opens only for a selected audio-capable clip. |
| Supply valid local Whisper CLI/model paths and choose a language | Job reports progress, completes, and creates an `Auto Captions` text track. |
| Check captions against the selected clip's trim and speed | Text starts within the selected clip and follows its visible timeline timing. |
| Undo then redo the generated result | The whole generated caption transaction disappears and returns together. |
| Start a longer transcription and click **Cancel** | Job reports cancellation and leaves no partial text track. |
| Enter an invalid tool or model path | The dialog shows a clear failure and leaves the project unchanged. |

## Playback, edits, and export

| Check | Expected result |
|---|---|
| Play for more than 10 seconds, pause, seek, return to start/end | Video and audio remain synchronized and preview does not turn black. |
| Mute/unmute the active audio/video clip | Audible output changes immediately without stopping preview. |
| Add/remove Cross Dissolve and Dip to Black; change a supported audio crossfade duration | Valid edits work, undo/redo works, and insufficient handles show a specific error. |
| Use Half, Normal, and Double speed | Preview and exported timing follow the chosen speed. |
| Export a short project containing video, audio, captions, template text, an effect, and a transition | Output plays with expected picture, sound, timing, text, and transition. |
| Rename or move one imported file, reopen the project, then relink it | Missing-media state is explicit; relinking restores the intended asset only. |

## Report format

Send results in this compact form:

```text
Phase 26
- Canvas/templates: PASS/FAIL
- Caption import/style/export: PASS/FAIL
- Auto captions: PASS/FAIL/BLOCKED (CLI/model available: yes/no)
- Playback/seek/mute: PASS/FAIL
- Transitions/crossfade/speed: PASS/FAIL
- Export: PASS/FAIL
- Save/reopen/missing-media relink: PASS/FAIL
- Failures: exact action + visible result
```

