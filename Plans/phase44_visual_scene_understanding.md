# Phase 44 — AI visual scene understanding

**Status:** COMPLETE WITH LIMITS — bounded local visual analysis, native UI and MCP were exercised on a disposable fixture; model accuracy remains unqualified beyond that fixture.
**Parent roadmap:** `Plans/plan_editor_mcp_ai_roadmap.md`
**Architecture:** local-first, derived media analysis, and all edits through the existing Command Service. No project schema change is expected.

## Objective

Let a user or MCP client ask TinixCut what is visually present in selected source media, receive timestamped scene descriptions, and use those results as evidence when planning an editable draft. Keep inference local by default and never send source paths to the model.

## Scope

1. Reuse bounded FFmpeg scene detection and thumbnail extraction. Sample at most 32 representative frames across at most 8 selected video assets; cap each frame to 512 px on its longest side and bound request/response size and inference time.
2. Add a local LM Studio vision adapter. Send only explicitly selected frame bytes and the user's query to `127.0.0.1`; no cloud provider, source path, project JSON, audio, or unselected media is sent.
3. Return a versioned, read-only report with asset IDs, source-time ranges, generated captions/tags, and query relevance. Treat model descriptions and relevance as suggestions, not verified facts or calibrated probabilities.
4. Cache derived reports against source fingerprint, prompt/algorithm version, model ID, query, and sampling settings. Cache deletion must not affect projects; changed input/configuration must miss the cache.
5. Expose the same analysis through MCP and a native UI flow. The UI shows selected media, query, progress/cancel, timestamped results and actionable provider/unsupported-model errors. Analysis never mutates the project or creates timeline clips.
6. Preserve the output as a usable input for later edit planning. Applying edits from scene results is a later phase and remains Command-Service-backed.

## Acceptance gates

### Gate 44A — bounded local analysis

- Deterministic tests cover scene interval construction, representative frame selection, sampling limits, invalid/missing media, malformed model JSON, unknown/duplicate scene IDs, empty results, timeout/cancel and cache invalidation.
- MCP tests prove the tool is read-only, revision-stable and does not return local paths or frame bytes. Requests target loopback only.
- On the local Gemma 4 E4B vision model, a disposable fixture with visually distinct shots produces a complete timestamped result. A human checks captions against the frames; report observed mismatches and do not claim general accuracy from the small fixture.

### Gate 44B — native workflow

- On a disposable project, select media, enter a visual query, run and cancel analysis, then view results with timestamps and descriptions. Provider-offline, non-vision model and malformed-response errors are actionable.
- Native UI and MCP return equivalent normalized report semantics for the same input/configuration. The project revision and serialized bytes remain unchanged.
- `cargo fmt --check`, focused Rust/MCP tests, frontend typecheck/test/build pass; runtime is restarted per `DEVELOPMENT_WORKFLOW.md`, and TinixCut health plus the local frontend are verified after the last code change.

## Non-goals

- Scene-level trimming, clip creation, reordering, automatic assembly or applying edits.
- Cloud inference, automatic upload, face/identity recognition, safety judgments, calibrated confidence, exhaustive action recognition, continuous video ingestion, or analysis beyond configured duration/frame limits.
- Changes to canonical project schema or persistence of model-generated descriptions as project semantics.

## Research basis

- LM Studio's official OpenAI-compatible endpoint supports chat completions with text and images; image input can be supplied as base64 data URLs. See `RESEARCH.md` Phase 44 entry.
- Google's Gemma documentation describes Gemma 4 vision tasks including image captioning, visual question answering and multi-image reasoning. This supports feasibility; it does not guarantee correctness on TinixCut footage.
- At planning time the host has 64 GiB RAM, an RTX 3090 and a local `google/gemma-4-e4b` download (6.33 GB); LM Studio API was not running and no model was loaded. A live run must be performed after explicitly starting/loading the local model; do not silently fall back to a remote provider.

## Qualification result — 2026-11-03

**Gate 44A: PASS WITH LIMITS.** The visual-scene module uses a fixed LM Studio loopback endpoint and sends the query plus resized JPEG frame bytes. It does not put media paths, display names, asset IDs, project JSON, audio, or unselected media in the model request. Sampling is capped at 8 selected videos and 32 frames, frames at 512 px / 512 KiB, request JSON at 8 MiB, response at 256 KiB, and inference at 120 seconds. Input/report validation rejects malformed, missing, duplicate and out-of-range model results. Rust tests cover representative sampling, intervals, input bounds, path-free request construction, malformed/unknown/duplicate/empty model results, cache separation, timeout, cancel while reading the response, and oversized responses. The MCP acceptance tool-list contract also passed.

Local inference used `google/gemma-4-e4b` through LM Studio with the disposable 15-second Hoi An fixture. It produced three timestamped scenes at 2.5, 7.5 and 12.5 seconds. A human frame check found the first result incorrectly called morning light “sunset”; the other two captions broadly matched the inspected frames. This small fixture confirms the workflow and format, not general caption accuracy. The UI labels scores as model rankings and shows the warning that results are unverified suggestions.

**Gate 44B: PASS WITH LIMITS.** Native UI completed the same query/model/fixture and displayed the three timestamped results. A separate in-flight analysis was cancelled from the UI and reported cancellation. MCP `tinixcut_media_visual_scenes` returned the same normalized timestamps, captions, tags, rationales, ordering and relevance rankings for the same query/model/sampling limit. The native test project remained at revision 1 with zero tracks; analysis did not add clips or change the timeline. The disposable native project was unsaved, so there were no serialized project bytes to hash; the read-only MCP call likewise accepted no project path. Computer Use observed TinixCut Core Healthy and the project Vite server returned HTTP 200.

Evidence: `artifacts/phase44/disposable/phase44_visual_scene_e2e_2026-11-03.json`, the copied fixture and inspected frame PNGs in the same directory. The E2E report records the fixture SHA-256 and the observed first-caption mismatch.

Verification after the final Rust transport change: `cargo fmt --check` PASS; the 9 focused Rust visual-scene/cache tests PASS; `pnpm run typecheck` PASS; frontend tests PASS (97/97); `pnpm run build` PASS; `git diff --check` PASS. The project runtime was restarted from the updated source and its PID/health are recorded in the current `PROJECT_STATUS.md` checkpoint.

Limit: scene descriptions and rankings remain fallible and require review. This phase does not create or apply edits from detected scenes; that is a later phase.

### Parser robustness follow-up — 2026-11-03

The native Gemma run returned fewer scene descriptions than sampled frames. The parser now accepts a non-empty subset of requested frames, still rejects unknown/duplicate IDs and malformed values, and reports omitted frames as an explicit warning. The prompt/schema asks the model for a valid subset when it cannot describe every frame. Nine focused visual-scene tests pass, including subset omission count and malformed-response rejection. This improves resilience without treating omissions as full coverage or increasing confidence in captions. Phase 45 native workflow reuses this report; its qualification is recorded in `Plans/phase45_scene_guided_edit_drafts.md`.