# P19–P29 Cumulative Snapshot — Inventory & Evidence Cross-Check

Date: 2026-10-25
HEAD: `e6462544b6956054ee50af40727d44e92121487b` (Phase 18 commit)
P29 code fingerprint: `a4f00b9f4f481568d51085ee678eb986d209ce016a0aa11ec0539a83e2e00bc3`
Working-tree status hash: `3f1d077691d76be15e53e8d630639ebb31ed9e24f1b2cdd40d6dc31d75855f72`

**This commit is a cumulative baseline for Phases 19–29. It must NOT be labelled "P29 only".**
HEAD is the Phase 18 commit; Phases 19–28 were never committed, so their changes and P29 are
interleaved in the same files. A buildable/verifiable per-phase commit chain cannot be produced
from this working tree without reconstructing history.

## 1. Snapshot scope (126 files, ~3.8 MB including text evidence)

| Area | Files | Notes |
| :--- | :--- | :--- |
| Rust product source | `src-tauri/src/**` (modified + new modules) | domain/command/composition/persistence/automation + P19–P28 modules |
| Rust tests | `src-tauri/tests/**` | Phase 14–18 suites, Phase 28 POC, Phase 29 domain + independent QA |
| Frontend | `src/**` | Phases 19–26 UI, preview sync, P29 type mirrors |
| Config | `Cargo.toml`, `Cargo.lock`, `package.json`, TS/Vite configs | pre-existing Phase 19–28 dependency work only |
| Fixtures | `src-tauri/fixtures/` (2 files, 513 KB) | P28 font/media fixtures |
| Plans | `Plans/*.md` | Phase 19–36 plans, Phase 26 qualification guide |
| Docs | `ARCHITECTURE.md`, `PLAN.md`, `PROJECT_STATUS.md`, `RESEARCH.md`, `phase_status.md`, ADR-007 | includes P29 checkpoint and ADR-007 |
| Evidence (text) | `artifacts/animated-text-templates/phase-27/**`, `phase-28/*.md|*.json|*.csv`, `phase-29/**` | contract, reports, evidence summaries |
| Research evidence (text/project) | `artifacts/capcut-*/**` except binary media | pre-P27 CapCut POC reports/commands/projects |

## 2. Phase → code → evidence cross-check (no re-run; candidate unchanged)

| Phase | Code present | Durable evidence |
| :--- | :--- | :--- |
| 19 Text templates | `modules/text_templates.rs`, `TextStyle`, schema v5 migration, Inspector controls | PROJECT_STATUS Phase 19 checkpoint, `Plans/plan_phase19_text_templates.md` |
| 20 Manual captions | `media/captions.rs` (SRT/WebVTT) | PROJECT_STATUS P20–25 summary, `Plans/plan_phase20_manual_captions.md` |
| 21 Canvas/aspect | `ProjectSettings.canvas_background_color`, migration v6→v7 | PROJECT_STATUS P21 checkpoint, `Plans/plan_phase21_social_video_workflow.md` |
| 22 Effects/DipToBlack | `VisualEffectPreset`, `TransitionKind::DipToBlack`, migration v7→v8 | PROJECT_STATUS P22 checkpoint, `Plans/plan_phase22_curated_effects_transitions.md` |
| 23 Speed | `ClipSpeed`, migration v8→v9 | PROJECT_STATUS P23 checkpoint, `Plans/plan_phase23_speed_controls.md` |
| 24 Caption enhancement | `TextTrackStyleSet`, `TextTrackTemplateApply` | PROJECT_STATUS Phase 24 checkpoint, `Plans/plan_phase24_caption_enhancement.md` |
| 25 Auto captions | `media/transcription.rs`, `AutoCaptionDialog.tsx`, `run_auto_caption_job` | PROJECT_STATUS Phase 25 checkpoint, `Plans/plan_phase25_automatic_captions.md` |
| 26 Qualification/transport | `components/previewVideoSync.ts`, preview fixes | `Plans/phase26_manual_qualification.md` (OPEN for native GUI), `Plans/huong_dan_test_phase26_tieng_viet.md` |
| 27 Contract/fixtures | none (docs/assets only) | `artifacts/animated-text-templates/phase-27/**`, `Plans/plan_phase27_template_contract.md` |
| 28 Renderer POC | `media/text_renderer_poc.rs`, `tests/phase28_text_renderer_poc.rs`, lib IPC command, editorApi client, ADR-007 | `artifacts/animated-text-templates/phase-28/**`, `Plans/plan_phase28_text_renderer_poc.md` |
| 29 Rich-text domain | `domain/text.rs`, v10 migration, 7 commands, composition carry-through, TS mirrors, CLI/MCP capability parity | `artifacts/animated-text-templates/phase-29/**`, `Plans/plan_phase29_rich_text_domain.md` |

All six migrations (`v4→v5` … `v9→v10`) are present. No Phase 19–28 evidence was found to be
missing from the tree; Phase 26 remains an open qualification item (by design).

## 3. Exclusion policy (must not be staged)

| Category | Excluded paths |
| :--- | :--- |
| Build output | `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/schemas/` (gitignored) |
| Generated benchmark output | `artifacts/animated-text-templates/phase-28/runs/**`, `phase-29/phase28-benchmark-current-*.csv` |
| Binary evidence | `artifacts/**/*.png`, `artifacts/**/*.mp4` (including phase-28 POC outputs and CapCut POC renders) |
| Auto-Recovery workstream | `tools/execution-guard/**`, `.auto-recovery/**` (main tree has none; separate branch) |
| Local state | `.recovery`, `*.tmp`, logs, editor files |

`artifacts/animated-text-templates/phase-28/benchmark-metrics.csv` **is** historical gate evidence
(not a generated run artifact) and is included. The run-scoped CSVs are excluded.

## 4. Safety checks performed

- Candidate fingerprint and status hash recomputed = frozen values (no drift).
- Secret scan over product/tests/plans/evidence: no private keys, API keys, tokens or passwords found.
- No `.env`, `.pem`, `.key`, `.pfx`, `.p12`, credentials or log files in the untracked set.
- `src-tauri/fixtures/` is 513 KB (2 files); no build artifacts.
- No Auto-Recovery files exist in the main tree (`tools/execution-guard` absent, no `.auto-recovery`).
- Commit remains **not created**.
