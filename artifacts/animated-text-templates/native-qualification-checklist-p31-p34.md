# Native Qualification Checklist — P34-AC-09 and Phase 31 Gate T1

**Status:** OPEN — not executed. Prepared 2026-10-27.
**Purpose:** Step-by-step native checks that remain open after the Phase 26 user-reported PASS.
**Rule:** Mark an item PASS only with observed evidence. Do not infer from automated tests.

Use a disposable project for every destructive or save step. Keep the original CapCut sample untouched.

---

## A. P34-AC-09 — CapCut importer native workflow

Acceptance (from `Plans/plan_phase34_capcut_template_import.md`): native importer workflow, source comparison ROI/frame report, failure tests, and regression/build/restart/runtime evidence PASS.

| # | Step | Expected result | Result | Evidence |
|---|------|-----------------|--------|----------|
| A1 | Launch TinixCut from the current dev build; confirm the app shows `Core Healthy`. | Runtime healthy. | ☐ | |
| A2 | Create a new disposable project. | Empty project opens. | ☐ | |
| A3 | Open the CapCut import dialog and select the documented sample draft. | Dialog shows inspection and resource report. | ☐ | |
| A4 | Confirm the reported conversion profile matches the documented profile (full conversion, `StaggeredCharacterIn`). | Profile matches `artifacts/animated-text-templates/phase-34/source-hashes-and-profile.json`. | ☐ | |
| A5 | Apply the template to the disposable project. | Text clip created on the timeline; no error banner. | ☐ | |
| A6 | Edit the text content, then undo and redo. | Content reverts and reapplies. | ☐ | |
| A7 | Save the project, close it, reopen it. | Applied template and edited text persist. | ☐ | |
| A8 | Export a short range to MP4. | Export completes; file plays. | ☐ | |
| A9 | Failure path: select an invalid or unsupported file. | Explicit error message; project unchanged. | ☐ | |
| A10 | Compare exported frames against the reference at the milestones in `phase-34/timing_frame_report.json`. | Differences recorded in a report. Do not claim parity unless the ROI tolerance is met. | ☐ | |

Store results under `artifacts/animated-text-templates/phase-34/native/` (screenshots, exported MP4, project copy, and a short result note).

---

## B. Phase 31 — Gate T1 native recreated sample

Acceptance (from `Plans/plan_phase31_text_animation.md`): P31-AC-01 through P31-AC-08. Native smoke must confirm content edit, preview, and export.

| # | Step | Expected result | Result | Evidence |
|---|------|-----------------|--------|----------|
| B1 | Open the recreated sample project in `artifacts/animated-text-templates/phase-31/recreated_sample_project.json` (use a copy). | Text clip shows the sample animation in preview. | ☐ | |
| B2 | Play and seek through the timeline: start, middle, end. | Animation motion visible; no freeze or "Unable to play media" banner for text. | ☐ | |
| B3 | Change the text to a short string, then a long string, then a string with Vietnamese diacritics. | Each variant renders without crash; motion preserved. | ☐ | |
| B4 | Export the sample. | MP4 produced; frames show animation. | ☐ | |
| B5 | Compare exported frames at `t_000`, `t_mid`, `t_end` with `phase-31/frame_t*.png`. | Differences recorded in `differences_vs_reference.md`. | ☐ | |
| B6 | Confirm undo/redo and save/reopen preserve the edit. | Edits persist. | ☐ | |

Store results under `artifacts/animated-text-templates/phase-31/native/`.

---

## Reporting

For each item, record PASS, FAIL, or BLOCKED with the evidence path. A FAIL or BLOCKED item keeps its gate open. Do not update `PROJECT_STATUS.md` until every row has a result.
