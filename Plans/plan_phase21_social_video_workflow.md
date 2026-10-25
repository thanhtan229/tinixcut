# Phase 21 — Social Video Workflow

**Status:** IMPLEMENTED  
**Scope:** Canonical canvas presets and a parity-safe path to social-video composition.

## Objective

Make 16:9, 9:16, 1:1, and 4:5 practical project formats without creating a preview-only canvas setting or an export-only override. The canonical `ProjectSettings` canvas must remain the source of truth for composition, persistence, history, automation, preview, and default export.

## Implemented slice 21.1 — Canvas presets

- `ProjectSettingsSet` changes canvas dimensions through the Command Service.
- Project settings validate positive, even H.264-compatible dimensions up to 8192 pixels.
- The desktop toolbar exposes 16:9, 9:16, 1:1, and 4:5 presets.
- Preview derives its displayed viewport from the canonical canvas aspect ratio.
- The default export profile inherits canonical project dimensions, frame rate, and audio sample rate.
- Command history regression coverage proves change, undo, redo, and invalid-input rejection.

## Implemented slice 21.2 — Media placement

- Clips carry an explicit canonical `Fit` or `Fill` placement value, separate from their manual transform/crop data.
- Imported source dimensions, placement, and canvas dimensions compile into the shared normalized composition plan.
- Preview and FFmpeg export consume the same normalized placement geometry.
- Existing clips migrate safely to `Fit`, and changing the project ratio does not rewrite transforms.

## Implemented slice 21.3 — Canvas background and safe zones

- `CanvasBackgroundSet` persists a normalized `#RRGGBB` background through the Command Service with history support.
- The normalized composition carries the background to both preview and FFmpeg export.
- The preview renders title/action safe-zone overlays for supported social ratios. The overlays are local, non-persistent, and cannot change export semantics.
- Blur-fill remains deferred pending a bounded shared preview/export design.

## Implemented slice 21.4 — Explicit export presets

- The Export menu provides `Balanced`, `High quality`, and `Smaller file` quality presets.
- Each preset inherits the canonical project dimensions, frame rate, and audio sample rate; only CRF, encoder speed, and audio bitrate differ.
- The backend rejects a direct export profile whose dimensions or frame rate conflict with the project, and rejects requests that supply both a profile and a preset.
- No output choice mutates the editable project canvas.

## Acceptance criteria

- **P21-AC-01:** A canvas preset is a revision-checked command, survives save/reopen, and round-trips through undo/redo.
- **P21-AC-02:** Preview viewport, composition plan, and default export agree on each canonical canvas dimension.
- **P21-AC-03:** Invalid canvas dimensions fail before project mutation with an actionable error.
- **P21-AC-04:** Fit and fill produce the same normalized bounds in preview and export fixtures.
- **P21-AC-05:** Canvas background and title/action safe overlays do not alter project media timing or export semantics unexpectedly.
- **P21-AC-06:** Export presets are explicit and do not mutate the project unless the user sends a project-settings command.
- **P21-AC-07:** Targeted regression, full automated validation, build, and project-owned runtime restart pass. Native-desktop manual checks are batched into Phase 26.

## Non-goals

- Arbitrary canvas dimensions in the initial toolbar.
- Persisted FFmpeg filter graphs, decoder handles, WebView objects, or CSS layout state.
- Blur-fill until a shared preview/export representation is accepted.
- HDR, color grading, 4K realtime guarantees, automatic reframing, and AI crop tracking.
