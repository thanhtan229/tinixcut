# Phase 55 — AI content and duration edit plans

**Status:** Implementation and automated acceptance complete for the recorded planner scope; full Phase 55 quality gate remains OPEN for two additional approved real project/brief samples, an independent qualified reviewer, three-project scoring, and corresponding native review/save/reopen/export evidence.

## Scope delivered so far

- Scene-guided draft review now exposes source In/Out values for every selected scene.
- Defaults preserve the analyzed source range at microsecond precision. User edits are restricted to the analyzed scene interval and must form a positive range.
- Both local-AI scene suggestions and deterministic preview use the reviewed source ranges. The report passed to the native builder is copied; the original analysis report is not mutated.
- Scene draft preview now offers advisory or exact target duration. Exact mode allocates frames evenly across selected scenes, preserves at least one frame per scene, trims only scene tails, and refuses targets below the selected-scene minimum or above total source capacity. Advisory mode leaves reviewed ranges intact and reports the frame-derived duration delta.
- The same frame-based duration contract is available through the scene-draft MCP tool using optional `targetDurationFrames` and `exactDuration` fields; omission keeps the existing advisory/full-range behavior.
- The existing native planner still revalidates scene IDs, source fingerprint, asset media kind, source duration, project revision, frame quantization, and Command Service operations before Apply.
- Added structured `mustKeepSceneIds` / `mustOmitSceneIds` constraints to local scene suggestions across native UI, Tauri and MCP. The shared core validates unique/known/non-conflicting constraints on both model suggestions and manual preview plans, and rejects missing required or selected excluded scenes before returning a draft. The constraints and model/provider remain in the review suggestion result for provenance.

## Verification in this slice

- `sceneRangeDraft.test.ts`: default range preservation, in-bounds trim, input validation, and source report immutability.
- Rust scheduler tests: exact frame total/capacity invariants and explicit infeasibility errors.
- `phase55_structured_scene_brief_constraint_corpus`: 12 deterministic English/Vietnamese constraint cases (valid keep/omit, model selection violations, unknown/duplicate/conflicting/blank IDs) passes; local mocked-model test verifies the structured constraints are sent and retained.
- MCP acceptance verifies invalid must-keep IDs are rejected before inference, invalid manual selections are rejected, and a valid constrained preview retains the expected exact frame duration. TypeScript, 150 frontend tests and production build pass.
- MCP acceptance test proves a 240-frame exact request produces two 120-frame clips and leaves the saved project unchanged.
- Added `phase55_bilingual_brief_corpus_runs_through_scene_draft_planner`: 12 Vietnamese/English fixture briefs pass through the full deterministic scene-plan builder. The corpus covers advisory and exact frame totals, valid caption/audio intent guidance, source allowlists, required/omitted scenes, portrait canvas/title template, malformed constraints/ranges, unavailable media, and infeasible exact-duration bounds. Every case confirms planning leaves the project unchanged; successful exact cases verify the sum of video-clip frames.
- Invalid model-returned IDs and malformed intent bounds remain covered by separate focused inference/validation tests; this deterministic corpus is not a claim about model quality.
- Frontend suite, TypeScript, production build, native cache tests and native `cargo check` results are recorded in `PROJECT_STATUS.md` for the combined checkpoint.
- Native UI verification is deferred: WebView has logged `Cannot read properties of undefined (reading 'invoke')` for Tauri IPC, and Computer Use is unavailable in this agent session. The normal target executable remains locked by the pre-existing TinixCut PID 33680, which was left untouched. After the latest code change, the isolated runtime rebuilt from `%TEMP%\tinixcut-phase55-runtime-target`; TinixCut PID 40120 started at 15:11 and workspace Vite PID 23584 remains active. `localhost:1420` returns HTTP 200. Computer Use is not available in this session, so native Apply/Undo/save/reopen/export are not claimed.


### Phase 55 canvas and title-template draft slice — 2026-11-04

- Added four explicit canvas presets (16:9, 9:16, 1:1, 4:5) to the scene-draft review. A changed preset is represented by `ProjectSettingsSet` in the same validated edit plan as the scene clips, so preview remains read-only and Apply/Undo stay on the Command Service path.
- Passed the preset through the Tauri preview command and MCP `tinixcut_edit_plan_scene_draft` contract. The UI now shows output dimensions in the review.
- Fixed a wire-format mismatch found by the MCP acceptance test: Rust's inferred enum names omitted underscores (`portrait9x16`), while clients send explicit snake-case names (`portrait_9x16`). Each enum variant now declares its stable serialized name.
- Verification: four-preset serde/dimension unit test PASS; scene planner rejects malformed ranges and offline sources, accepts video-only media, and validates title/template operations; MCP scene draft acceptance PASS including 1080×1920 canvas, title and template operations, and incomplete-pair rejection; native `cargo check` PASS; TypeScript PASS; 150 frontend tests PASS; production build PASS (existing >500 kB bundle warning remains).
- Optional opening title and one of six built-in text templates now create a title overlay clip in the same edit plan. Pair validation rejects missing text/template; title is limited to 240 UTF-8 bytes with control characters rejected. The title duration is at most three seconds and no longer than the video draft.
- Native Apply/Undo, save/reopen and exported aspect-ratio observation remain unverified because the WebView IPC/Computer Use blocker persists.
### Phase 55 editorial quality rubric — 2026-11-04

- Added [editorial rubric](phase55_editorial_quality_rubric.md) with anchored 1–5 criteria for brief relevance, continuity/order, and text readability; technical validity/export is reported separately.
- Exploratory review of three sampled frames in the existing Phase 45 artifact scored relevance 4/5, continuity 2/5, readability 4/5 (10/15). It flags the sunset → early morning → night ordering and previously observed incorrect “sunset” model caption. This is one reviewer/sample only; the 3-project/second-reviewer target remains open.
- Added an exact 100-frame draft test at 25 fps; two scene intervals are scheduled as 50+50 frames. Exact 240-frame MCP case at 30 fps remains independently covered.

### Phase 55 allowed-source policy — 2026-11-04

- Added `allowedAssetIds` to the shared scene-selection constraints across the native review UI, Tauri preview/suggestion commands, and both MCP scene tools. An empty list remains backward-compatible and means all report sources are allowed.
- The UI defaults to all report sources and lets the editor exclude a source. Excluding one also removes its scenes and per-scene keep/omit constraints from the current draft.
- The local model receives only captions, tags, ranges, and scene IDs belonging to allowed assets. Asset IDs and names, paths, media bytes, and disallowed-scene data are not sent. The JSON schema enumerates only permitted scene IDs.
- The deterministic preview builder independently validates unique/known asset IDs, required-scene compatibility, and selected-scene membership; direct MCP/native callers cannot bypass the policy.
- Added regression coverage for unknown/duplicate/blank allowlists, filtered model request payloads on a two-source fixture, rejected out-of-policy preview selections, MCP schema discovery, and invalid asset IDs.
- Added optional caption and audio intent fields to the local suggestion flow. The planner forwards these bounded text hints for scene ranking, records them in suggestion constraints, and explicitly instructs the model not to claim it created subtitles/music or audio edits. The review UI labels them as guidance only.
- Intent validation rejects blank, over-500-byte, or control-character values. A local mocked-model test verifies both intents reach the planner alongside only allowed-source scenes; a validation test covers malformed inputs.
- Made provenance visible at both analysis and draft review: source ranges are identified as TinixCut analysis; caption/tag and relevance are identified as one-frame model suggestions, and relevance is explicitly not described as a probability. Transcript evidence is not part of this scene planner yet.
- Verification: allowlist unit test, two-source private-payload planner test, MCP scene-plan acceptance, MCP JSON Schema acceptance, `cargo check --bin tinixcut`, TypeScript, 150 frontend tests, production build, `cargo fmt --check`, and `git diff --check` pass. The planner regression test now parses the user payload and enforces exact top-level and per-scene key allowlists, while asserting no transcript fields are sent. Production build retains the existing >500 kB chunk warning.
- Runtime watcher rebuilt the workspace candidate after the Rust change. The app still logs the known WebView IPC error `Cannot read properties of undefined (reading 'invoke')`; UI interaction is not claimed. Separate standard-target TinixCut PID 33680 remains untouched.
## Still required for G55

- Optional local-AI opening-title suggestion is implemented as review-only text (opt-in, 240-byte/control-character bounds, no project mutation); choosing the suggestion and a built-in template remains an explicit human action. Broader brief semantics remain limited: caption/audio intent is planner guidance only and does not create synchronized subtitles, narration, music, or audio edits.
- Exact and advisory duration now have planner coverage at 23.976 and 29.97 fps with unequal source capacities; broader capacity distributions and three-project editorial review remain. See [non-integer scheduler evidence](../artifacts/phase55/phase55_non_integer_exact_duration_evidence.json).
- Provenance boundary is now audited: metadata and FFmpeg heuristics, model-generated one-frame claims, user caption/audio guidance, native duration/operation calculations, and the absence of transcript input are distinguished in `phase55_provenance_audit.json`. This documents where data comes from; the captions/order still need human accuracy and quality review. Invalid/stale model ranges must fail clearly.
- The 12-brief planner corpus now covers the deterministic plan boundary with a silent-video fixture. Intent-bound failures and invalid model outputs are verified by separate targeted tests; add them to a live-provider corpus only when the provider can be started safely. Provenance accuracy and editorial quality remain separate from technical planner validity.
- Three native review/edit/Apply/Undo/Redo/save/reopen/export workflows and an independent editorial-quality rubric.

## Next action

Extend the editorial rubric across three separately reviewed projects and arrange an independent second reviewer; the provenance boundary audit is complete and recorded. Continue native review/edit/apply/history/save/reopen/export on independent disposable projects when the native runtime permits. Exact and advisory scheduling now preserve their respective semantics at 23.976 and 29.97 fps with asymmetric scene capacities. The 12-brief deterministic planner corpus already passes. Native review/edit/apply/history/save/reopen/export remains queued for the final blocker pass; caption/audio intent remains guidance-only.

### Payload allowlist regression and runtime checkpoint — 2026-11-04

- Strengthened `local_scene_suggestion_is_bounded_private_review_only_and_validates_ids`: it parses the JSON sent to the local model, enforces the exact top-level and scene-field allowlists, and rejects any transcript text in the request. Targeted Rust test: **1 passed**; `cargo fmt --check` and `git diff --check` passed.
- Restarted the workspace-owned `pnpm tauri dev` runtime after the Rust change. Latest candidate PID 30056 runs from `%TEMP%\tinixcut-phase55-runtime-target\debug\tinixcut.exe`, SHA-256 `B67E65F2B2EE3EC59C96F1CB79F6C69CB48BD55A188234FCEBE79F2A5F000D54`; Vite responds HTTP 200. Vite emitted `invoke` errors before the Tauri window launched; the native window subsequently rendered `Core Healthy` and passed the basic track/history smoke recorded in Phases 48 and 50. Full native AI review/apply/save/reopen/export remains open. Standard TinixCut PID 33680 and Paseo were left untouched.

### Exhaustive exact-duration capacity cases — 2026-11-04

- Added a bounded exhaustive regression test over every 1–4-scene capacity vector with each scene capacity in 1–6 frames, and every feasible target for each vector: **16,329 schedules**.
- Each schedule must total the exact target, allocate at least one frame per selected scene, stay within every source capacity, and keep allocations of non-saturated scenes within one frame of each other.
- The focused test passed **1/1**, and all edit-plan module tests passed **27/27** with 2 opt-in LM Studio tests ignored in the isolated Phase 55 Cargo target. `cargo fmt --check` and `git diff --check` pass. This extends the prior 23.976/29.97 fps asymmetric examples with dense small-capacity coverage; it does not replace native review or multi-project editorial assessment.

### Native AI scene-search and draft review — 2026-11-04

- On the disposable unsaved UI project using `phase44_hoi_an_visual_fixture.mp4`, local model `google/gemma-4-e4b` completed scene search for “Hoi An street scenes with people and lanterns”. It returned three suggestions at 00:02, 00:07 and 00:12, with one-frame model descriptions/tags and the warning that motion between representative frames may be missed.
- Draft review exposed all three source intervals (00:00–00:05, 00:05–00:10, 00:10–00:15), allow-source and must-keep/must-omit controls, and editable In/Out. The local model recommended all three in order. The UI stated relevance scores are not calibrated probabilities and that the project had not changed.
- Preview reported three selected clips, 15.00 seconds, validated operations, and the current project revision. Apply created the `Scene Draft` track; Undo removed it, Redo restored it, and a final Undo returned to the pre-Apply state. The existing Track 1 remained present throughout. The disposable project was not saved.
- This is one native fixture/provider run, not a multi-project editorial-quality gate. Save/reopen/export and three-project rubric remain open.
- During a separate caption-editor text-field probe, the WebView became black and its accessibility subtree collapsed to pane nodes. Restarting only the workspace candidate restored `Core Healthy`; its recovery prompt was closed with `Later` and no recovery copy was restored/discarded. Current candidate PID 31756 runs from the isolated temporary target; `http://localhost:1420` returned HTTP 200. The normal workspace executable remains locked by untouched TinixCut PID 33680, and Paseo was not affected.
- The Phase 54 100-clip workload was not opened: the common file picker did not expose an editable filename field through UIA; SendKeys corrupted the typed path, and the dialog was canceled. No workload file was modified and no G54 seek metric is claimed.

### Optional AI opening-title suggestion — 2026-11-04

- Added opt-in `suggestOpeningTitle` to the shared scene-selection constraints and MCP/Tauri contracts. The local model receives only the brief, optional intents, and already-allowed scene captions/tags/ranges. The response may include `openingTitleSuggestion`; it remains review-only and never creates or applies a clip.
- The native review dialog displays model provenance and offers an explicit “Dùng tiêu đề này” action to copy the suggestion into the editable title field. A built-in text template must still be selected before preview; Apply remains a separate Command Service action. Existing clients default to no title request.
- Native boundary rejects unrequested titles, titles over 240 UTF-8 bytes, and control characters. Test coverage: bounded/private mock inference **1/1**; title validation **1/1**; MCP request/output schema **1/1**. Current MCP acceptance after the change: **49 passed, 0 failed, 3 ignored**; `cargo check --bin tinixcut`, TypeScript, 150 frontend tests, production build, Rust format and `git diff --check` pass. Build retains the >500 kB chunk advisory.
- This does not claim title accuracy or native UI qualification. Computer Use remains unavailable; title suggestions still require user review. No project is mutated by the suggestion request.
- Runtime watcher applied the native change: candidate PID 30752, binary SHA-256 `9E25F3A9F46DB214AA860B846B45A2FE3AFA00F873F732275EF25DD199F378CF`, built 19:21:19 and started 19:21:20; Vite PID 12444 returns HTTP 200. Standard TinixCut PID 33680 and Paseo were left untouched.
### Live local-model title suggestion check — 2026-11-04

- Ran the opt-in live-provider test against local `google/gemma-4-e4b`: **1 passed**. The model returned `Hoi An Morning Serenity`; selection stayed read-only and source/project state was not changed.
- The model chose scene intervals totaling 10 seconds for an 8-second target and its rationale incorrectly said 15 seconds. The draft UI displays the validated 10-second duration and delta; model rationale is untrusted text and does not determine the duration.
- Runtime watcher restarted the workspace candidate after the final Rust test-source change: PID 26680, binary SHA-256 `DC62B0D48E1B362C96844DD6B3014FEBE8F63B416F8914BA92BB5867177C8655`, binary mtime 19:26:59, process start 19:27:12; Vite PID 12444 returns HTTP 200. Standard TinixCut PID 33680 and Paseo remained untouched.

### Phase 55 provenance boundary audit — 2026-11-04

The implemented scene-analysis and scene-selection boundaries were checked against source and targeted tests. Evidence: [`phase55_provenance_audit.json`](../artifacts/phase55/phase55_provenance_audit.json).

| Data or decision | Provenance | Use and limits |
|---|---|---|
| Asset identity/name | Local TinixCut project metadata | Used to map the report to source assets and label the UI; asset names/paths are not sent to vision inference or scene-selection inference. |
| Video duration/stream kind | FFprobe metadata | Native validation and analysis bounds only; duration truncation is reported. |
| Candidate cuts and source intervals | FFmpeg scene-candidate analysis plus native interval/sampling algorithms | These are heuristic source ranges, not semantic event boundaries. |
| Scene caption, tags, relevance and rationale | Selected local vision model, from one bounded representative still per sampled interval | Unverified claims; relevance is ranking only, not probability. No audio or transcript is used. |
| Caption/audio intent | Explicit user-provided text | Ranking guidance to the scene-selection model. It is not inferred transcript/audio evidence and creates no subtitle/music edit. |
| Scene ordering, rationale and optional opening title | Local selection model | Review-only suggestions. The native layer recomputes selected duration and validates constraints/ranges before producing commands. |
| Exact/advisory duration and edit operations | Native planner and Command Service | Frame totals, source capacities, revision/fingerprint and operations are validated outside model prose. |
| Transcript evidence | Caller-supplied source-timed excerpts associated with a current project asset fingerprint | MCP accepts optional transcript evidence. Native validation checks allowed asset identity, current source fingerprint, analyzed-time bounds, text/control limits, and scene overlap before inference. Only overlapping excerpt text and source times are sent; asset IDs, names, paths, and fingerprints are stripped from the model payload. The response records asset ID/fingerprint/used-segment count without copying transcript text. ASR/user text remains unverified ranking evidence, never ground truth. |

Verification rerun in the isolated Phase 55 Cargo target: visual-scene unit tests **9/9**, bilingual deterministic planner corpus **1/1**, bounded/private model payload **1/1**, and caption/audio intent validation **1/1**. This closes the provenance-*boundary* audit only; it does not establish model factual accuracy or satisfy the three-project/independent-review/native workflow gates.
### G55 continuation checkpoint — 2026-11-05

- Re-ran the deterministic structured-constraint and bilingual brief planner corpus from the current worktree: **2 passed, 0 failed** (`cargo test --lib phase55_`). The existing exhaustive exact-duration scheduler evidence remains recorded above.
- Automated planning checks leave the source project unchanged and validate constraints; they do not measure editorial usefulness or substitute for real brief/project review.
- G55 remains OPEN for native review/edit/apply/history/save/reopen/export and for two additional approved project/brief samples with an independent second reviewer. Those human review inputs are not available in this session.
### Local-model integration recheck — 2026-11-05

- Confirmed the already-running local LM Studio endpoint on `127.0.0.1:1234` returned HTTP 200 and reported `google/gemma-4-e4b`. No model service was started or reconfigured.
- Ran the two previously ignored, read-only MCP tests against that local model: `test_p41_mcp_opt_in_local_ai_draft_is_validated_and_read_only` and `test_p46_mcp_local_scene_selection_suggestion_is_read_only`; **both passed (1/1 each)**. Each test asserts the saved disposable project remains unchanged; outputs are schema/source bounded, and the first test also verifies explicit failure for an unavailable model.
- This is automated local inference evidence. It does not replace the separate editorial review rubric, additional approved real briefs/projects, or native Apply/Undo/save/reopen/export checks.

### G55 optional transcript ranking evidence — 2026-11-05

- Added optional `transcriptEvidence` to `tinixcut_edit_plan_scene_selection_suggest`. Existing callers omit it and retain the previous payload contract.
- The native planner rejects stale source fingerprints, unavailable or disallowed assets, non-video sources, malformed/out-of-range segments, control text, and over-limit evidence before contacting the local model. Scene inputs receive only time-overlapping excerpt text; model guidance explicitly treats it as unverified. The request omits asset IDs, asset names, paths and source fingerprints. Suggestion provenance returns source ID/fingerprint and the count of unique-use excerpts, not transcript text.
- Focused fake-server test verified the scene/time mapping, privacy boundary, stale fingerprint rejection and invalid range rejection. Focused Rust planner test passed **1/1**; Phase 55 corpus passed **2/2**; MCP tool schema acceptance passed **1/1**. A real stdio invocation of the workspace binary returned 32 tools and exposed both `transcriptEvidence` input and `transcriptEvidenceSources` output. `cargo fmt --check`, `cargo check --bin tinixcut`, and `git diff --check` passed.
- The workspace Tauri dev watcher rebuilt and started TinixCut PID **19956** after the last source edit; its window responds. Vite PID **9112** returns HTTP 200. No process was manually stopped. Computer Use is unavailable, so no native UI interaction is claimed for this change.
- This implements the optional transcript ranking/provenance slice only. G55 remains **OPEN** for the required two additional approved real project/brief samples, rubric scoring on three distinct projects, an independent second qualified reviewer, and native workflow coverage across three projects.
### Native scene draft qualification — 2026-11-05

- On a disposable project using a copied synthetic 15-second Hoi An fixture, local Gemma analyzed three scenes and suggested order scene-03 → scene-02. The native review displayed the model's 8-second advisory target against the core-computed 10-second draft (+2 s); model rationale was not treated as authoritative.
- Native preview left the project unchanged; Apply created one Scene Draft track with two 150-frame clips; Undo removed it and Redo restored it. The project was saved to a separate copy and reopened after a workspace runtime restart at revision 4.
- The toolbar Save callback bug discovered during this workflow was fixed by wrapping the callback so React's click event cannot become the forceSaveAs argument. Regression behavior was verified natively after restart: Save atomically updated the existing copy and did not open Save a Copy. TypeScript, all 165 frontend tests, production build, and git diff --check passed.
- Native export completed in 2521 ms. ffprobe reports H.264/AAC, 1920×1080 at 30 fps, 300 video frames, 10.0 seconds, AAC 48 kHz stereo; full FFmpeg decode exits 0. Export SHA-256: 9D453210ECE451ED236209ADC9AA3A6FDC33B6892488C742FFEBC35FB5145934. Source fixture SHA remained EF2400D263622DA266D0A378B59BA0290ED7EB6475EDAE46E239D8F110B66A5E. Full evidence: artifacts/phase55/phase55_native_ui_workflow_evidence_20261105.json.
- This closes one native synthetic-project workflow, not the G55 gate. Two additional approved real project/brief samples, three-project rubric coverage, and an independent second qualified reviewer remain unprovided.

### G55 independent project-material and reviewer availability audit — 2026-11-05

- Inventoried every non-disposable `.tinix` sample outside phase artifacts. The only real project corpus is `artifacts/media-samples/hoi-an-morning/`: the landscape and portrait project variants use the same three still images and ambient WAV (asset hashes recorded in `artifacts/phase55/phase55_sample_availability_audit_20261105.json`). `hoi-an-one-day.mp4` and the Phase 44/55 15-second video fixture share SHA-256 `EF2400D263622DA266D0A378B59BA0290ED7EB6475EDAE46E239D8F110B66A5E`; they are the same Hoi An corpus, not independent samples. Other available videos are generated test fixtures.
- Do not count landscape/portrait exports or repeated briefs over this single corpus as two additional independent real-project samples. Original project/media hashes were read only and remain untouched.
- **G55 remains OPEN / external input required:** the gate needs two additional approved real project/brief samples and an independent qualified editorial reviewer. The three-project rubric target cannot be truthfully scored without those inputs. Existing automated 12-brief corpus, local-model tests and one native synthetic workflow remain valid but do not substitute for the missing gate evidence.

### Runtime IPC provenance correction — 2026-11-05

The earlier note that native WebView logged `invoke` errors is superseded. The errors were captured from a standalone Paseo browser tab without Tauri internals, as proven by `artifacts/phase54/phase54_invoke_warning_provenance_20261105.json`. A direct capture of the current workspace TinixCut shows Core Healthy. This does not alter G55's external review/sample gate or its remaining native workflow evidence.