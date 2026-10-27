# Phase 34 — headless verification (2026-10-27)

Scope: backend importer (inspect, convert, apply, save/reopen, undo/redo, offline render) on the user's real sample draft, run by the existing acceptance suite. No GUI.

- Command: `cargo test --test phase34_capcut_import` (in `src-tauri`)
- Result: **48 passed, 0 failed, 1 ignored** (the ignored test is the evidence writer), 127 s.
- Real sample used by the suite: `C:\Users\Admin\Downloads\__CAPCUT\CapCut Drafts\TINIX_CAPCUT_IMPORT_TEST\draft_content.json` (present on this machine; tests skip with a message if absent).

Coverage mapped to the native checklist (A = Phase 34 items):

| Item | Covered headlessly by | Status |
|---|---|---|
| A4 profile/family recognition | `test_p34_ac04_*`, `test_p34_ac02_*` | Automated PASS |
| A5 apply to project | `test_p34_ac06_*`, `test_p1_7_*` | Automated PASS |
| A6 undo/redo | `test_p34_ac06_*` | Automated PASS |
| A7 save/reopen | `test_p34_ac06_*`, `test_p34_ac07_*` | Automated PASS |
| A8 export (offline render) | `test_p34_ac07_*` | Automated PASS |
| A9 failure paths | `test_p34_ac01_*`, `test_zip_*`, `test_p1_8_*` | Automated PASS (backend errors) |
| A10 source comparison | `artifacts/animated-text-templates/phase-34/reference-comparison.json` | Measured; not parity (same causes as Phase 31: glow, placement convention, size) |

Not covered headlessly (GUI only, still required for P34-AC-09):
- A3: the import dialog shows inspection and resource report.
- A9: the dialog shows an explicit error for an invalid file.
- Confirming the importer button triggers the same backend path end to end.

Status: P34-AC-01..08 PASS per evidence; P34-AC-09 remains PARTIAL until the GUI-only items above are observed.
