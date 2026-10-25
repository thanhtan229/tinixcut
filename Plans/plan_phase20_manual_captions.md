# Phase 20 — Manual Captions and SRT/VTT Foundation

**Status:** IMPLEMENTED — INTERACTIVE SMOKE OPEN  
**Scope:** Import and export timed manual captions through the existing Text item and Command Service paths.

## Objective

Make common subtitle interchange files useful in TinixCut without adding a second caption timeline model. SRT and WebVTT cues become ordinary canonical Text items on a dedicated Video track, so editing, persistence, preview, export, undo, and redo continue to use the existing project semantics.

## In scope

- Parse UTF-8 SRT and WebVTT files with BOM/CRLF normalization.
- Convert subtitle timestamps to the project frame rate using integer arithmetic.
- Reject malformed, empty, zero-duration, overlapping, oversized, or over-counted input with actionable errors.
- Import all valid cues as one Command Service transaction on a new `Captions` Video track.
- Export all non-empty Text items as SRT or WebVTT, inferred from the selected output extension.
- Add desktop UI actions for importing and exporting captions.
- Add parser, timestamp, validation, command-path, and serializer regression tests.

## Explicit non-goals

- Automatic speech-to-text or provider integration.
- Word-level timing, karaoke highlighting, styling extensions, positioning, or WebVTT metadata.
- Caption overlap support on one track. Overlapping input is rejected because the current track invariant forbids overlapping items.
- Changes to the canonical project schema. Existing `Text` items already provide the required durable representation.

## Acceptance criteria

- `*.srt` and `*.vtt` import into a dedicated Text-capable Video track.
- Imported cues appear at the correct project-frame positions and retain multiline text.
- Import is one undoable transaction; undo and redo restore the full import.
- SRT and VTT export round-trip timestamps and text within the project frame-rate quantization.
- Invalid input fails before project mutation and explains the reason.
- Existing test, typecheck, format, lint, build, and runtime restart checks pass.

## Implementation checkpoint — 2026-10-09

- Parser and serializer regression tests pass for SRT/VTT normalization, validation, multiline text, timestamp rounding, and extension handling.
- `pnpm run test:all`, `pnpm run typecheck`, `pnpm run build`, Rust format check, library Clippy, and Rust build pass.
- The project-owned `pnpm tauri dev` runtime was restarted after the final code change; Vite responds HTTP 200 and the current `tinixcut.exe` is running from `src-tauri/target/debug`.
- Interactive caption import/export smoke remains open because this session cannot independently control or observe the native TinixCut window.

## Deferred follow-up

Automatic transcription, caption styling controls, per-word timing, overlapping cues, and richer WebVTT features remain separate phases.
