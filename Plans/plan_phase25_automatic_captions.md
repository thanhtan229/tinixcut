# Phase 25 — Automatic Captions

**Status:** IMPLEMENTED — automated verification complete; one end-to-end native smoke remains for Phase 26

## Scope

TinixCut adds an opt-in, fully local automatic-caption workflow for one selected audio or video clip. The user supplies paths to a compatible `whisper-cli.exe` and model `.bin`; neither executable nor model is bundled or persisted in the project. The paths and selected language are retained only in local UI preferences.

## Execution path

1. Validate the selected online audio/video clip and the supplied local tool paths.
2. Submit one bounded `Transcription` job, with status and cancellation exposed to the UI.
3. Use the existing FFmpeg boundary to create a disposable 16 kHz mono WAV.
4. Run the local Whisper CLI with JSON output, polling for cancellation.
5. Parse provider millisecond offsets, clip them to the source range, map them through trim and fixed speed to canonical project time, then discard overlaps caused by rounding.
6. Create an `Auto Captions` text track and commit ordinary Phase 20 Text clip commands in one Command Service transaction. The history entry therefore supports the normal undo/redo path.
7. Reject a stale result when the selected clip or underlying media changed while the job ran. Remove all temporary WAV/JSON files on success, cancellation, and error.

## Constraints

- Audio leaves neither the device nor the local process tree started by TinixCut.
- There is no hidden caption project model, direct mutation path, cloud provider, model download, karaoke timing, or automatic styling in this phase.
- Only one transcription job runs at a time. Re-running after a terminal result is permitted.
- A missing executable, missing model, unsupported selection, provider failure, cancellation, or stale result is reported explicitly.

## Acceptance evidence

- Whisper JSON parsing and trimmed/speed-mapped timing have focused Rust coverage.
- Job completion can be submitted again, while active work remains deduplicated.
- TypeScript typecheck, production frontend build, Rust test suite excluding the known non-terminating POC5 matrix test, native Rust build, and diff whitespace check pass.
- Phase 26 retains the native end-to-end smoke: select a clip, choose a compatible local Whisper CLI/model, generate captions, inspect timing/text, then undo/redo.
