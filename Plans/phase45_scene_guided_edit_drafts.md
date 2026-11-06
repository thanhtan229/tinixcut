# Phase 45 — Scene-guided editable drafts

**Status:** COMPLETE WITH LIMITS — all five gates passed for the bounded local fixture scope (2026-11-03)
**Parent:** `Plans/plan_editor_mcp_ai_roadmap.md`
**Depends on:** Phase 44 local visual scene reports; existing revision-bound `EditPlan` and Command Service.

## Objective

Let a user or MCP client select timestamped scenes from a Phase 44 report and create a reviewable rough cut from those source ranges. The plan must be validated against current project media and revision, and all edits must continue through the canonical Command Service.

## Scope

- Accept a versioned visual scene report and an explicit ordered list of scene IDs.
- Revalidate report schema, selected scene ranges, asset identity, online state and source fingerprint against the current project.
- Build one new video track with gapless `ClipCreate` operations that use the selected source-time intervals.
- Return a revision-bound `EditPlanDraftReceipt`; do not mutate the project during preview.
- Expose equivalent native UI and MCP previews. Apply remains a separate user action through the existing Command Service; undo/redo use existing history.
- Display the source timestamps, model captions and unverified-description warning during scene selection/review.

## Gates

1. Rust tests cover valid interval mapping, report/source validation, missing/stale media, duplicate/unknown scene IDs, invalid time ranges and dry-run validity without project mutation.
2. MCP tests cover scene-plan tool schema, read-only behavior and revision-bound output.
3. Native UI can select/order scenes, preview the draft, cancel without mutation, apply, undo and redo on a disposable project.
4. A fixture-based export from an applied plan passes ffprobe and full decode; the input fixture stays unchanged.
5. Rust format/tests, MCP acceptance, frontend typecheck/tests/build pass; restart and verify the workspace TinixCut runtime.

## Non-goals

- Automatically selecting scenes without explicit user/caller selection.
- Applying model captions as subtitles, face recognition, claim of calibrated relevance, cloud inference, or edit operations outside Command Service.
- Trimming arbitrary source intervals to a duration supplied in natural language.

## Qualification result — 2026-11-03

**Gates 1–2: PASS.** Rust tests cover scene interval mapping, source/report revalidation, subset model responses, malformed/unknown values and a revision-bound read-only MCP receipt. Focused MCP test `test_p45_mcp_scene_edit_plan_is_revision_bound_and_read_only` passed. The preview's base revision was 1, listed four validated operations for three clips, and confirmed reordered scene IDs `scene-02`, `scene-01`, `scene-03` without changing the project.

**Gate 3: PASS on a disposable native project.** Scene analysis returned three local Gemma suggestions at 2.5s, 7.5s and 12.5s. The UI showed model captions and the unverified-description warning. Cancel before Apply left revision 1/zero tracks unchanged. Apply committed revision 2 with one video track/three clips. Undo restored revision 1/zero tracks; Redo restored revision 2/one track/three clips. Existing recovery snapshots were preserved; an older recovery revision was rejected while a newer revision 2 existed, then Redo saved revision 2.

**Gate 4: PASS.** TinixCut native Export UI created `artifacts/phase45/disposable/phase45_scene_guided_draft.mp4` in Balanced mode. ffprobe reports H.264/AAC, 1920×1080, 30 fps, 450 frames and 15.000 seconds. Bundled FFmpeg full decode exited 0. Human review of frames at 1.5s, 7.5s and 13.5s confirmed three distinct scenes in the applied order. The source fixture remained byte-identical (SHA-256 `EF2400D263622DA266D0A378B59BA0290ED7EB6475EDAE46E239D8F110B66A5E`). Detailed evidence: `artifacts/phase45/disposable/phase45_native_ui_export_evidence.json`.

**Gate 5: PASS.** `cargo fmt --check`, 9 visual-scene unit tests, the visual-scene edit-plan unit test, and the targeted MCP acceptance test passed. Frontend typecheck, tests (97/97) and build passed earlier on this UI candidate. TinixCut remained Core Healthy; workspace Vite returned HTTP 200. Code was not changed during this qualification turn, so restart was not applicable. `git diff --check` is recorded at the workspace checkpoint.

**Limits:** only the specified 15-second disposable synthetic fixture and local Gemma model were qualified. Model descriptions can be wrong (Phase 44's first-scene morning/sunset error remains documented); human review is required. No general scene-selection accuracy, broad codec/scale, installed hardware encoder, clean-VM or physical A/V claim is made. User has designated physical A/V calibration, clean VM and installed hardware encoder as optional claim-specific qualifications, not blockers to this roadmap.