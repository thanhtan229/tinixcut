# Phase 46 — Brief-guided scene selection

**Status:** COMPLETE WITH LIMITS (2026-11-04). Planner, native UI, MCP and local-model paths qualified; default WebView profile issue and recovery snapshot behavior remain documented operational limits.

## Objective

Let a user provide an editing brief and optional target duration, then ask an explicitly selected local LM Studio model to suggest a subset and order of scenes from a Phase 44 visual-scene report. Keep the suggestion reviewable and reuse the Phase 45 revision-bound preview and Command Service apply path.

## Scope

- Send only the brief, scene IDs, timestamps, captions, tags and analysis rationale to the loopback model endpoint. Never send media bytes, source paths, asset names or the project document.
- Return selected scene IDs, their order, concise rationale, selected interval duration and optional target duration.
- Validate report freshness, source fingerprints, scene IDs, uniqueness, bounded count, brief/model/target limits and the resulting Phase 45 draft before returning a suggestion.
- Expose the same read-only capability in native UI and MCP. Applying remains a separate user-reviewed Command Service operation.
- Allow manual changes to the suggested selection/order before generating the preview.

## Non-goals

- Guaranteeing an exact target duration. Phase 46 uses whole detected scene intervals and reports their computed duration; exact trims are a separate intent.
- Automatically applying edits, asserting scene-caption truth, sending data to a remote provider, or adding a second edit reducer.

## Acceptance

- Valid local-model selection is a strict subset/order of unique supplied IDs; unknown, duplicate, empty and oversized responses fail closed.
- Stale report/source and invalid duration/model/brief are rejected; project remains unchanged.
- A mocked local endpoint proves privacy bounds and deterministic parsing; MCP schema/tool and native command expose the same result.
- Native UI can request a suggestion, review and manually adjust it, build the existing preview, then apply only through Phase 45's Command Service.
- Focused Rust/frontend validation passes; application runtime is restarted after final source changes and health-checked.
- `git diff --check` passes; evidence and limits are recorded here and in the roadmap/status snapshot.

## Verification limits

Real-model inference and the native click path are evidenced in the dated checkpoints below. The optional duration is a whole-scene estimate, not a hard duration constraint.

## Initial qualification checkpoint — 2026-11-04 (superseded by the native qualification below)

- **Rust planner:** mocked endpoint test passed privacy, scene-ID, source-fingerprint and no-mutation checks. Live `google/gemma-4-e4b` test passed and returned `scene-01 → scene-03`, totaling 10 seconds for an 8-second target. The generated rationale incorrectly implied the target was met; the dialog now computes and displays the `+2.0s` difference independently.
- **MCP:** live `tinixcut_edit_plan_scene_selection_suggest` acceptance passed against the local model. The disposable project's revision, tracks and assets remained unchanged. The test also verifies returned scene IDs belong to the supplied report.
- **Frontend/build:** typecheck, production build and 97 frontend tests passed. `cargo check --bin tinixcut`, `cargo fmt --check`, and `git diff --check` passed. Full machine-readable evidence: `artifacts/phase46/phase46_qualification_evidence.json`.
- **Runtime restart:** restarted using `pnpm tauri dev`. The new debug binary is PID 35292, started 2026-11-04 00:11:14 +07:00, SHA-256 `0C7C72B0C4AB16A12D13286260DB3C40D313938FFEB7914BCED1E961A982EDF1`; Vite PID 17456 serves `http://localhost:1420/` with HTTP 200. One stale prior workspace debug process (PID 3712) was stopped after confirming it descended from the replaced Tauri dev process. Paseo was left running.
- **Native UI (initial default-profile attempt):** BLOCKED because the window rendered black and exposed no app shell. This checkpoint was superseded by the isolated-profile native qualification below; the default profile remains an operational issue.

This was an interim status. The native flow was later qualified using an isolated fresh WebView profile; see the completion checkpoint below.

### Native UI qualification — 2026-11-04 (completed on isolated WebView profile)

- The default workspace WebView profile still rendered black. Without stopping or altering it, launched the same TinixCut binary with `WEBVIEW2_USER_DATA_FOLDER` pointing at a fresh temporary profile. Native UI rendered Core Healthy; screenshot and accessibility identified TinixCut. Existing recovery candidates were left untouched (the recovery prompt was deferred).
- Imported `artifacts/phase44/disposable/phase44_hoi_an_visual_fixture.mp4` into a new unsaved project. Source SHA-256 remained `EF2400D263622DA266D0A378B59BA0290ED7EB6475EDAE46E239D8F110B66A5E`.
- Native visual analysis returned three scenes at 00:02, 00:07 and 00:12. The local Gemma scene-selection suggestion chose all three (15.0 s) for an 8 s target; the UI independently reported `+7.0 s` and explained that target duration is advisory.
- Manually deselected scene-01 and scene-03, keeping scene-02 (5.0 s). `Tạo preview` showed 1 video clip, 2 validated operations and base revision 1, with project unchanged. `Apply draft` committed revision 2 with one track/clip and the preview showed the selected source scene.
- `Undo` returned to revision 1/zero tracks. Recovery save reported `A newer recovery revision 2 already exists`; no existing recovery copy was modified or removed. `Redo` restored revision 2/one track/clip. The project remained disposable and unsaved.
- Full machine-readable evidence is in `artifacts/phase46/phase46_qualification_evidence.json`. The downstream native export/decode is covered by Phase 45; this interaction did not repeat export.
- Operational limit: the original default-profile window remains black while the isolated fresh-profile instance is healthy. This qualification proves the feature workflow on the fresh profile; it does not repair or qualify the default profile. The temporary profile was not promoted to a permanent app setting.

**Phase 46 is COMPLETE WITH LIMITS.** Feature acceptance is met through the native suggestion → manual selection → preview → Apply/Undo/Redo flow. The default WebView-profile rendering issue and the stale-newer-recovery save rejection remain follow-up diagnostics; they do not invalidate the fresh-profile UI evidence.