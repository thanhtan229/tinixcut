# Phase 19 — Text Templates and Caption-Ready Text Foundation

**Status:** COMPLETE WITH ACCEPTED FALLBACK AND DEFERRED ENVIRONMENT GATES
**Research basis:** `Plans/research_creator_expansion.md` R2 preliminary audit, `RESEARCH.md` creator-expansion reconnaissance, current Schema v4 model/composition audit
**Scope owner:** TinixCut local desktop editor
**Primary gate:** durable text-style semantics and preview/export parity

**Extension note — 2026-10-11:** This completed phase remains the basic six-preset foundation. Rich typography, character animation, layered packs, supported CapCut conversion and agent generation are planned separately in `plan_animated_text_templates.md` (Phases 27–36). Its accepted font fallback is a legacy compatibility constraint for that program, not evidence of rich-text parity or full animation support.

## 1. Objective

Add a small, durable text-template system that makes common creator text edits fast while preserving TinixCut's canonical Project Domain Model, Command Service, undo/redo, persistence, automation, and preview/export parity.

The phase delivers six built-in declarative templates:

1. Clean lower-third
2. Bold headline
3. Center pop
4. Subtitle card
5. Quote card
6. Call-to-action

Applying a template creates or updates an ordinary editable `ItemKind::Text`. The project stores the resolved style and layout values needed to render the item, so the project remains renderable if a template definition is retired or unavailable.

## 2. Evidence and decisions from research

The current model already supports text content, transform, opacity, generic keyframes, `ClipTextSet`, command history, and a normalized composition plan. That is enough foundation for this phase.

The current model does not persist font, color, background, padding, alignment, line height, anchor, template identity, or template version. The composition layer supplies fixed defaults, and the FFmpeg path hard-codes parts of text style and placement. A UI-only template picker would therefore lose semantics on save/reopen and could diverge between preview and export.

The implementation must first add typed text-style state and propagate it through the composition plan. External repositories are used as workflow and architecture references only; they do not define TinixCut's project schema or visual design.

## 3. Proposed domain model

### 3.1 `TextStyle`

Add a typed value object owned by the domain model. The first version should contain only properties required by the six built-in templates:

- `font_family: String`
- `font_size: f32`
- `font_weight: u16`
- `font_color: String`
- `accent_color: Option<String>`
- `background_color: Option<String>`
- `background_opacity: f32`
- `padding_x: f32`
- `padding_y: f32`
- `alignment: TextAlignment`
- `line_height: f32`
- `anchor: TextAnchor`

Use typed enums for alignment and anchor. Validate color format, finite positive dimensions, opacity range, line-height range, and supported font fallback values at the domain boundary. Reuse the existing transform for scale, rotation, and position animation; do not duplicate those fields inside `TextStyle`.

### 3.2 `TextTemplateReference`

Add an optional typed reference:

- `template_id: String`
- `template_version: u32`

Do not persist an untyped override map. Applying a template resolves its values into `TextStyle`, and later user edits change those resolved values while retaining the reference. A future “reset template” command can reapply the referenced version explicitly.

### 3.3 `TimelineItem`

Add:

- `text_style: Option<TextStyle>`
- `text_template: Option<TextTemplateReference>`

Text items created by the migration receive the default style and no template reference. Non-text items keep both fields absent. Resolved values remain on the item even when the referenced template is unavailable.

### 3.4 Schema migration

Increase the project schema from v4 to v5. Add a lossless v4→v5 migration that initializes default text style for existing text items and leaves non-text items unchanged. Loading must remain backward compatible with older JSON fixtures through the existing migration chain.

The migration must be covered by:

- v4 text item load and save;
- v4 non-text item load and save;
- current-schema round trip;
- malformed text-style validation;
- atomic-save and recovery behavior.

## 4. Composition and rendering contract

Extend the normalized composition layer so text layers carry the resolved `TextStyle` and optional template reference for diagnostics. Preview and export must consume the same normalized style values.

The preview renderer must use the style for font family, weight, color, background, padding, alignment, line height, anchor, and safe placement. The FFmpeg renderer must use the same normalized values and an explicit fallback path for unsupported font features. Do not persist FFmpeg filter syntax or renderer handles.

The first phase does not require animated color, gradients, per-word karaoke, arbitrary font installation, Lottie assets, or a general effect graph. Animation uses the existing transform/opacity/keyframe system and the template's bounded keyframe definitions.

## 5. Command and automation contract

All changes use the existing Command Service and revision/idempotency rules.

Add the smallest command set that supports the UI and automation:

1. `TextTemplateApply` — apply a built-in template to an existing text item or create a text item through the normal `ClipCreate` path followed by the same command-backed initialization.
2. `ClipTextStyleSet` — update one validated style payload as one undoable edit.
3. `ClipTextTemplateReset` — re-resolve the stored template version when the user requests reset.

If the existing command shape can represent creation without a dedicated command, reuse it rather than introducing a parallel text insertion path. The template application must produce a normal timeline item visible to existing selection, delete, move, save/reopen, undo/redo, CLI, and MCP paths.

## 6. Frontend work

Add a compact “Text templates” entry point in the existing editor shell. The first UI should provide:

- the six built-in template choices;
- text input;
- foreground/background/accent color controls;
- anchor selection;
- scale and in/out duration controls using existing transform/fade semantics;
- reset-to-template action;
- clear validation and unavailable-template errors.

The Inspector should edit the canonical `TextStyle` through commands. It must not hold template state as a frontend-only object. Keep the panel compact and use the existing TinixCut visual language from `UX_UI.md`.

## 7. Work sequence

### 19.1 Domain and migration

- Define `TextStyle`, `TextAlignment`, `TextAnchor`, and `TextTemplateReference`.
- Add defaults and validation.
- Bump schema to v5 and implement v4→v5 migration.
- Add domain and persistence tests.

### 19.2 Composition parity

- Carry resolved text style through the normalized composition plan.
- Remove fixed style and placement defaults for supported text items.
- Update preview and export consumers from the shared composition values.
- Add deterministic 16:9 and 9:16 frame fixtures.

### 19.3 Commands and history

- Add template apply, style update, and reset commands.
- Verify revision conflict, transaction rollback, idempotency, undo/redo, CLI, and MCP routing.
- Verify a template-created item behaves like an existing text item in all standard timeline operations.

### 19.4 Built-in template registry

- Implement a small application-owned declarative registry with stable IDs and versions.
- Keep registry definitions free of executable code, remote references, and renderer-specific payloads.
- Add explicit fallback behavior when a referenced template definition is missing.

### 19.5 UI

- Add the template picker and text-style controls.
- Add selection, loading, validation, and unavailable-template states.
- Preserve existing playback, seek, mute, inspector, and timeline behavior.

### 19.6 Qualification

- Run the targeted domain, command, persistence, composition, frontend, and export tests.
- Run the normal typecheck, frontend build, Rust format, Clippy, and Rust build checks.
- Restart the project-owned runtime after the final code change and verify the current build is loaded.
- Perform native Windows smoke tests for template insertion, editing, undo/redo, save/reopen, preview, and export.

## 8. Acceptance criteria

- **P19-AC-01:** Existing v4 projects load with valid default text styles and preserve all non-text data.
- **P19-AC-02:** New text items can be created through the existing command/history path.
- **P19-AC-03:** All six templates apply through commands and produce ordinary editable Text items.
- **P19-AC-04:** Template apply, style edit, reset, delete, and move support undo/redo with no duplicate history entries.
- **P19-AC-05:** Save/reopen preserves text content, resolved style, template ID/version, keyframes, and timeline placement.
- **P19-AC-06:** A missing template definition does not prevent rendering from resolved project state and reports a recoverable diagnostic.
- **P19-AC-07:** Preview and export use the same resolved text style at 16:9 and 9:16.
- **P19-AC-08:** Deterministic frame checks pass at start, midpoint, and end for every built-in template animation.
- **P19-AC-09:** Text remains inside the configured title-safe area at both supported aspect ratios.
- **P19-AC-10:** Inspector edits validate invalid colors, dimensions, opacity, and unsupported fonts without corrupting project state.
- **P19-AC-11:** CLI and MCP can inspect and apply the same template commands as the UI.
- **P19-AC-12:** Existing transport, seek, mute, audio crossfade, media relink, and export regression suites remain passing.
- **P19-AC-13:** Frontend typecheck/build, Rust format, Clippy, Rust build, and targeted test suites pass.
- **P19-AC-14:** The final project-owned runtime restart succeeds and the current build responds to its documented health/runtime check.

## 9. Non-goals and stop conditions

This phase does not include automatic captions, SRT/VTT import, word-level karaoke, AI generation, template marketplace, Lottie import, arbitrary plugin execution, animated gradients, masks, tracking, speed controls, or a full CapCut feature clone.

Stop implementation and return to research if the design requires a second timeline model, untyped script execution, persisted renderer-specific graphs, preview/export divergence, or a font fallback that cannot be made deterministic.

The deferred Phase 16–18 runtime and clean-VM qualifications remain explicitly tracked. They must not be silently reclassified as passed by this feature phase.

## 10. Required evidence before marking Phase 19 complete

Record the schema migration result, command/history results, parity fixture results, missing-template fallback behavior, native Windows smoke results, build/test output, and runtime restart/health evidence in `Plans/plan_phase19_text_templates.md` and `PROJECT_STATUS.md`.

Current implementation checkpoint:

- 19.1 domain and migration: implemented; schema v5 is active and v4 text items receive default styles.
- 19.2 composition parity: implemented for persisted style values in normalized composition, preview, and export paths.
- 19.3 commands and history: template apply, style set, reset, the core round-trip regression, and all six built-in template applications are covered by command tests. Automation capability exposure is covered; direct interactive CLI/MCP app-flow qualification remains.
- 19.4 registry: six versioned built-in templates and unavailable-template diagnostics are implemented.
- 19.5 UI: template picker, caption input, reset action, compact style controls, and a command-backed `+ Text` creation action are implemented; the user reports the native smoke workflow as PASS.
- 19.6 qualification: automated tests, typecheck, build, format, Clippy, native build, runtime restart, and HTTP health verification passed. The user reports PASS for native template insertion, picker/style/reset, undo/redo, save/reopen, preview, and export. Native Computer Use did not expose a window in this session, so the agent could not independently observe those steps.

Latest automated qualification evidence (2026-10-09):

- `pnpm run test:all`: PASS — 44 frontend tests; 214 Rust tests passed with 5 intentionally ignored benchmark tests.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: PASS.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib -- -D warnings`: PASS.
- `cargo build --manifest-path src-tauri/Cargo.toml`: PASS.
- `pnpm run build`: PASS.
- The project-owned `pnpm tauri dev` process tree was replaced after the final code changes; Vite responded with HTTP 200 and a fresh `tinixcut.exe` process was present.
- The FFmpeg export runner now drains child stderr concurrently. This fixes the export deadlock that occurred when the stderr pipe filled before the child process exited. The Phase 18 crossfade acceptance test now completes in under one second.

Known bounded limitation accepted for personal-use v1: the FFmpeg path uses its deterministic fallback font for rendering and does not yet expose every `TextStyle` field; font weight and line-height are not fully represented in the current drawtext fallback. Native typed `app_health_check`, independent preview cleanup, P16 GUI, clean-VM, and hardware-encoder gates remain deferred.
