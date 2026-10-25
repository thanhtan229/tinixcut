# Phase 24 — Caption Enhancement

**Status:** IMPLEMENTED  
**Schema impact:** None. Schema v9 remains active.

## Objective

Make repeated captions faster to style while preserving the existing canonical Text-item model, the Command Service mutation path, and shared preview/export typography.

## Scope

- Retain captions as ordinary `ItemKind::Text` timeline items created and edited by Phase 20.
- Add atomic Command Service operations that apply either a valid `TextStyle` or a resolved, versioned built-in template to every Text item on one target track.
- Return the target track as the affected scope and reject a missing track or one with no Text items.
- Add Inspector controls for applying the current style or one selected template to all text clips on the selected clip's track.
- Preserve existing SRT/VTT import/export and existing preview/export text-style behavior.

## Non-goals

- New persisted caption, subtitle, or word-timing model.
- Speech-to-text or automatic caption generation.
- Karaoke, word highlighting, animated captions, or external template packs.
- Arbitrary font loading or renderer-specific style payloads.

## Acceptance evidence

1. A single track-scoped style command changes every Text item on that track and no item on another track.
2. A single track-scoped template command resolves and records the selected template identity for every Text item on that track.
3. Both operations reject an empty target track without advancing the project revision.
4. The Inspector exposes batch choices only for tracks with more than one text clip.
5. Typecheck, frontend tests/build, Rust formatting/build, and the focused command regression test pass before runtime restart.

## Manual qualification

Phase 26 will cover native selection, batch-style/template behavior, undo/redo, save/reopen, preview, and export together with the other deferred Windows workflows.
