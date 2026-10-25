# Creator Expansion Research Program

**Project:** TinixCut
**Status:** ACTIVE — Phases 21–26 are authorized; native manual qualification is batched in Phase 26
**Purpose:** Identify the smallest reliable path from the current editor to a creator-focused product with reusable text templates, captions, and a few high-value editing workflows.

---

## 1. Product Direction

TinixCut should not attempt broad CapCut parity in one milestone. Its strongest near-term product position is a dependable local desktop editor for short-form content:

1. import, cut, arrange, caption, and export reliably;
2. add branded, reusable animated text quickly;
3. make common social-video edits without breaking preview/export parity;
4. keep every user and automation edit as normal editable project state.

The first expansion should be **Text Templates and Captions**, not a template marketplace, AI generation, or a broad effect catalogue.

## 2. Recommended Feature Order

| Priority | Product capability | Why it comes here | Deliberately excluded from this increment |
|---|---|---|---|
| P0 | Close existing deferred runtime/release checks | Prevents feature work from masking known preview/runtime uncertainty. | New architecture or feature work. |
| P1 | Reusable text-template system | Clear visible value, bounded rendering semantics, and uses existing text/keyframe foundations. | Downloadable marketplace, third-party template execution, arbitrary scripts. |
| P2 | Manual captions and SRT/VTT import | Completes a common creator workflow and provides the durable model needed before automatic captions. | Speech-to-text provider, karaoke word timing, cloud processing. |
| P3 | Basic social-video workflow | Aspect-ratio presets, safe zones, background/blur fill, and export presets. | HDR, 4K realtime guarantees, full color grading. |
| P4 | Small curated effect/transition pack | Adds perceived polish after template/export parity is proven. | Arbitrary FFmpeg graphs, plugin SDK, shader marketplace. |
| P5 | Speed controls and caption enhancement | High utility but raises timeline/preview/export complexity. | AI editing, tracking, masks, body effects, collaboration. |
| P6 | Automatic captions | Add bounded local transcription only after the existing caption model and cancellation path are proven. | Cloud transcription, karaoke timing, automated highlight styling. |
| P7 | Final integration and qualification | Consolidate manual creator workflow testing after the feature set stabilizes. | Repeated manual testing after every implementation slice. |

### Implementation authorization — 2026-10-09

The user authorized the complete Phase 21–26 roadmap. Automated regression, parity, build, and runtime checks remain mandatory after each coding slice. Native-window manual testing is intentionally accumulated into one complete Phase 26 pass, while known environment-dependent qualifications remain visible rather than being treated as passed.

## 3. Text Template Product Proposal

A template is a versioned, application-owned declarative preset. Applying one creates ordinary editable Text items and animation/keyframe state on the timeline. It must never persist executable JavaScript, a WebView object, an FFmpeg graph, a remote URL, or a Lottie document as the semantic source of truth.

### First pack

Ship six original templates, with a bundled template manifest and deterministic font fallbacks:

- Clean lower-third
- Bold headline
- Center pop
- Subtitle card
- Quote card
- Call-to-action

Each exposes a small, predictable parameter set: text, font/style variant, foreground/background color, screen anchor, scale, in/out duration, and optional accent color. Templates must work at 16:9 and 9:16, obey title-safe margins, and remain editable after application.

### Required semantics

- `template_id` and `template_version` identify the preset used at creation time.
- Resolved text, style, layout, and animation values are stored in the canonical project model so missing template packs cannot make a project unrenderable.
- A user may override supported properties without losing the original template identity.
- Template insertion, replacement, and parameter changes use the Command Service, revision checks, undo/redo, persistence, CLI, and MCP pathways.
- Preview and export consume the same resolved text/animation composition plan.

## 4. External Repository Research Queue

This program studies product ideas and test methods. Do not copy another editor's artwork, branded template packs, or product identity into TinixCut.

| Source | Research focus | Research question |
|---|---|---|
| [Clypra](https://github.com/AIEraDev/Clypra) | Native preview/compositor structure and tests. | How does its native preview/compositor boundary avoid playback regressions while the editor grows? |
| [OpenCut](https://github.com/OpenCut-app/OpenCut) | Creator interaction flow and caption/template UX. | Which creator workflows and panel patterns are understandable with minimal UI? |
| [OpenCut caption-model discussion](https://github.com/OpenCut-app/OpenCut/issues/898) | A community design discussion, not a TinixCut specification. | Can caption groups, word highlights, and restyling be represented as typed project primitives? |
| [OpenTimelineIO](https://github.com/AcademySoftwareFoundation/OpenTimelineIO) | Editorial and interchange concepts. | Which text/caption decisions should remain internal rather than be forced into interchange? |
| [lottie-web](https://github.com/airbnb/lottie-web) | Preview-motion experiment if selected. | Can limited motion assets be rendered deterministically enough for editor preview and export? |
| [Olive](https://github.com/olive-editor/olive) | Title/effect interaction ideas. | Which title/effect editing interactions are worth testing with users? |

## 5. Research Gates

### R1 — Feature evidence

Interview or observe at least five target users creating one short vertical video. Record the time and failure points for: importing media, trimming, adding headline text, adding captions, resizing to 9:16, and exporting. Turn repeated failures into ranked user stories.

### R2 — Template schema

Write a small proposed model and migration strategy for template identity, resolved properties, overrides, and missing/retired template versions. Confirm that templates neither bypass the Command Service nor introduce renderer-specific durable state.

#### Preliminary audit — 2026-10-08

The current model is a useful foundation but is not yet a template schema:

- `TimelineItem` persists `text_content`, transform, opacity, and generic keyframes; `ClipTextSet` and the generic keyframe commands already pass through the Command Service and history.
- The normalized composition plan carries text fields, and both preview and FFmpeg export consume that plan.
- `font_family`, `font_size`, and `font_color` are currently fixed defaults created by the composition layer rather than editable, persisted item state. The export path also hard-codes white text and its placement.
- There is no persisted background, padding, alignment, line-height, anchor, text-style object, template identity, template version, or override map.

The first implementation phase must therefore add a typed, versioned text-style payload to the canonical item model and propagate it through the composition plan before a template picker is introduced. A UI-only template library would not survive save/reopen reliably or preserve preview/export parity.

#### Proposed bounded vertical slice

1. Add `TextStyle` and `TextTemplateReference` to the canonical model, with migration defaults for existing text clips.
2. Carry resolved style values through composition, preview, and export; remove hard-coded text style/placement for supported text clips.
3. Add six built-in declarative presets: clean lower-third, bold headline, center pop, subtitle card, quote card, and call-to-action.
4. Add one command-backed template-apply operation that creates or updates normal Text items with resolved values, template identity, and keyframes.
5. Expose only editable text, color, anchor, scale, and in/out duration in the first inspector UI.
6. Prove create, change, undo/redo, save/reopen, preview, and export parity at 16:9 and 9:16.

### R3 — Six-template parity POC

Build a disposable proof of concept only after R2 approval. It must render six original templates at 16:9 and 9:16 in preview and export with deterministic frame comparisons at start, midpoint, and end of every animation.

### R4 — Captions model

Compare a simple caption item, caption group, and word-level timing model. Select the smallest representation that supports manual editing and SRT/VTT import while keeping karaoke and automatic transcription out of scope.

### R5 — Asset and font readiness

Create a template asset manifest with the intended font family, fallback chain, source hash, and render test result for every bundled font, icon, sound, image, and animation. Confirm that every template renders predictably with the chosen fallback when its preferred font is unavailable.

### R6 — Performance and failure budget

Measure template insertion, scrubbing, export, reopen, missing-font recovery, and undo/redo. Define clear fallback behavior: a missing font is reported and substituted only by an explicit user choice; a missing template definition still renders from the resolved project state.

### R7 — Implementation decision

Only after R1–R6, propose a numbered implementation phase with accepted product scope, a schema decision if necessary, acceptance criteria, migration plan, test matrix, and preview/export parity gate.

### P22 implementation decision — 2026-10-09

Phase 22 is bounded to four application-owned colour presets and one additional cut-owned transition. The decision follows an audit of the current `TimelineItem`, `Track.transitions`, normalized composition, CSS preview, and FFmpeg export paths:

- Effects require one typed `VisualEffectPreset` field on an existing visual item. A free-form filter string would violate the canonical-model boundary and make preview/export validation unbounded.
- The existing `CutTransition` collection already owns adjacent-video cut transitions. `DipToBlack` will extend its allowed kind set rather than introducing clip-level overlap state, a second timeline, or a renderer-owned transition object.
- FFmpeg documents both the `eq` colour controls used for the fixed export mappings and `xfade` transition options including `fadeblack`; see the [FFmpeg filter documentation](https://www.ffmpeg.org/ffmpeg-filters.html#eq) and [xfade documentation](https://www.ffmpeg.org/ffmpeg-filters.html#xfade).
- CSS and FFmpeg use separate colour-management implementations. Phase 22 can prove canonical preset/mapping parity and deterministic generated backend instructions, but it must not represent the result as byte-identical cross-platform pixels. Native visual qualification remains deferred to Phase 26.

The resulting approved plan is `Plans/plan_phase22_curated_effects_transitions.md`. It adds schema v8 migration, command/history/automation coverage, normalized composition propagation, bounded Inspector controls, and a focused regression matrix. It explicitly excludes arbitrary filters, LUTs, shaders, and marketplaces.

## 6. Stop Conditions

Do not advance to an implementation phase if any of these is true:

- the prototype requires a second timeline model, arbitrary script execution, or persisted backend-specific render graphs;
- preview/export frame comparisons diverge for supported templates;
- the required font fallback renders unpredictably in preview or export;
- adding templates destabilizes ordinary playback, seek, save/reopen, undo/redo, or automation;
- the team has not closed or explicitly retained the existing deferred runtime qualification gates.

## 7. Expected Deliverables

1. Product workflow evidence and prioritized user stories.
2. Repository research notes and a product-fit decision for every candidate.
3. Text-template and caption domain-model proposal.
4. Six-template preview/export parity POC report.
5. Template asset and font-fallback manifest.
6. A bounded implementation plan, only if all research gates pass.
