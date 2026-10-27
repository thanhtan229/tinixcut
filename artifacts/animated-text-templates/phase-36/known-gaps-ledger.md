# Phase 36 — Known Gaps & Deferred Qualification Ledger

**Document Version:** 1.0.0  
**Status:** COMPLETE WITH DEFERRED QUALIFICATION  
**Date:** 2026-10-27  

TinixCut strictly adheres to the principle of honest qualification: **untested native items, clean VM environments, and hardware setups are never marked as PASS without empirical evidence.**

The table below itemizes the complete ledger of deferred qualification items across all phases through Phase 36:

| Gap ID | Origin Phase | Category | Item | Current Status | Empirical Evidence & Justification | Remediation Path |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **GAP-01** | Phase 12, 16, 26 | Native GUI Acceptance | Interactive Windows GUI qualification | **PASS (user-reported 2026-10-27, Phase 26 checklist)** | Automated tests (Vitest 81 tests, Rust integration 200+ tests) pass 100%. Running in headless agent environment prevents manual human interaction in a desktop window. | Follow [Plans/huong_dan_test_phase26_tieng_viet.md](Plans/huong_dan_test_phase26_tieng_viet.md) during human verification session. |
| **GAP-02** | Phase 12, 26 | Offline Packaging | Clean-VM installer verification | **DEFERRED** | NSIS installer `TinixCut_0.1.0_x64-setup.exe` (57.54 MiB) successfully built and verified with bundled Gyan FFmpeg 9.0.2 and Roboto-Bold font. Execution on an isolated air-gapped machine without dev tools not run. | Run installer on a fresh Windows 10/11 VM without Node/Rust/Git installed and verify startup via `app_health_check`. |
| **GAP-03** | Phase 5, 17, 26 | Rendering / Export | Direct GPU Hardware Encoder (NVENC/AMF/QSV) | **DEFERRED** | Software CPU encoding (`libx264`) 100% verified and benchmarked at 4.92× RTF. Direct native GPU hardware encoder matrix requires physical multi-GPU test harness. | Execute Phase 26 hardware encoder checklist on machines with discrete NVIDIA/AMD/Intel GPUs. |
| **GAP-04** | Phase 34 | Template Import | AC-09 Native CapCut Import Modal Observation | **DEFERRED** | Backend draft inspection, resource resolution, ZIP extraction, and transactional apply pass 48/48 integration tests. Live Tauri WebView dialog click-through deferred. | Exercise the CapCut Import button in the running GUI using the documented sample file. |
| **GAP-05** | Phase 35 | Agent Automation | Embedded AI Provider Adapter & Native Staging UI | **DEFERRED** | External CLI and MCP interfaces are 100% functional and verified with 31 integration tests. In-app embedded LLM API key management and chat panel deferred to v2. | External agents use MCP (`tinixcut mcp`) or CLI (`tinixcut template ...`). In-app panel planned for future milestone. |
| **GAP-06** | Phase 15, 35 | Documentation / QA | MCP tool surface reconciliation | **RESOLVED** | ADR-005 and the current server contract list all 12 tools; Phase 15 acceptance coverage now checks the complete current surface. | No further action. |
| **GAP-07** | Phase 31, 29 | Text Rendering Parity | CapCut glow not represented in rich-text domain | **ROADMAP PHASE 32+** | CapCut frame 31: about 92k halo px vs about 30k in Tinix; no glow field in Phase 29 domain. | Scheduled in Phase 32+ backlog (Plans/plan_animated_text_templates.md section 11). |
| **GAP-08** | Phase 31 | Text Placement Parity | Vertical baseline offset vs CapCut | **ACCEPTED INTENTIONAL DIFFERENCE** | Decision: keep Tinix anchor. Baseline offset +6 to +14 px measured; not a defect. | No action; re-measure only if placement convention changes. |
