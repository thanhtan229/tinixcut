# Milestone Verification Checklist (Phases 27–36)

**Program:** Tinix Animated Text Templates  
**Authority:** Companion deliverable for Phase 27 baseline (`milestone-verification-checklist.md`)  
**Scope:** Actionable verification criteria for Gates T1, T2, T3, T4, and T5 across the implementation lifecycle.

---

## 1. Overview of Program Milestones

The Animated Text Templates roadmap is structured around five major integration checkpoints:

```text
Phase 27 (Contract & Baseline) 
  └─► Phase 28 (Renderer POC)
        └─► Phases 29-30 (Typography & Rich Appearance)
              └─► Phase 31 (Motion Engine) ───────────► [Milestone T1: Motion Prototype]
                    └─► Phase 32 (Layered Packs)
                          └─► Phase 33 (Inspector UI) ──► [Milestone T2: Full Native Authoring]
                                └─► Phase 34 (CapCut) ────► [Milestone T3: CapCut Parity Gate]
                                      └─► Phase 35 (MCP) ──► [Milestone T4: Headless Automation]
                                            └─► Phase 36 ──► [Milestone T5: Hardening & Release]
```

---

## 2. Milestone T1 — Core Motion Prototype Gate (End of Phase 31)

- **Target Phase:** Phase 31 (Motion Lifecycle & Character-Level Keyframing)
- **Goal:** Verify that typography, multi-stroke/shadow appearance, and grapheme stagger animations render deterministically across both preview and export.

### Verification Checklist:
- [ ] **T1-01 (Grapheme Invariant):** Vietnamese diacritics (`ắ, ế, ộ, ự`) and multi-codepoint sequences remain attached to base letters during all stagger, scale, and rotation transforms without splitting or visual tearing.
- [ ] **T1-02 (Motion Lifecycle):** In-animation, loop/steady hold, and out-animation transitions play in correct sequence with zero frame hitching.
- [ ] **T1-03 (Collision Policy):** When clip length is shorter than $D_{\text{in}} + D_{\text{out}}$, Command Service enforces the configured conflict policy (default validation reject or proportional scale if explicitly requested).
- [ ] **T1-04 (Preview vs Export Parity):** Rendered preview overlay frames and headless export frames achieve $\ge 0.985$ SSIM on text ROI regions.
- [ ] **T1-05 (Performance W1):** p95 frame rasterization time on reference host is $\le 16.0\text{ ms}$ for Workload W1 (120 graphemes, gradient, 2 strokes, 2 shadows).

---

## 3. Milestone T2 — Full Native Authoring & Pack Reuse (End of Phase 33)

- **Target Phase:** Phase 33 (Template Inspector, Live Editing & Visual Gallery)
- **Goal:** Full interactive editing in UI, multi-layer support, and standalone `.tinixtpl` template pack import/export.

### Verification Checklist:
- [ ] **T2-01 (Command Path Invariant):** All UI property edits (text, font size, stroke width, color gradient, motion duration) dispatch typed commands through the Command Service. Direct DOM/state tampering is strictly prohibited.
- [ ] **T2-02 (Undo / Redo Fidelity):** Every template modification, preset change, and template application can be cleanly undone and redone with zero state drift.
- [ ] **T2-03 (Interactive Gallery):** UI displays thumbnail/hover animated cards for all 6 built-in templates and imported packs without main-thread blocking or frame drops.
- [ ] **T2-04 (Pack Portability):** A `.tinixtpl` package exported from one project imports cleanly into another project with all font assets and layer hierarchies intact.
- [ ] **T2-05 (Structural Bounds):** UI and Command Service reject configurations exceeding hard limits (16 layers, 8 text slots, 512 graphemes, 4 strokes, 4 shadows) with user-friendly alerts.

---

## 4. Milestone T3 — CapCut Import Parity Gate (End of Phase 34)

- **Target Phase:** Phase 34 (CapCut Import Adapter & Asset Ingestion)
- **Goal:** Deterministic extraction and translation of CapCut desktop text templates into native Tinix template instances.

### Verification Checklist:
- [ ] **T3-01 (Offline & Security):** Import operates 100% locally from offline CapCut project cache (`draft_content.json`, `effectStyle.json`); absolutely no network requests and no execution of proprietary Lua code (`AnimScript.lua`).
- [ ] **T3-02 (Sample 01 Recreation):** User sample `TINIX_CAPCUT_SAMPLE_01` ("Tinix — Thử nghiệm tiếng Việt") imports with:
  - Linear gradient 118° (`#ffe35f` to `#ffffff`)
  - Stroke `#AD5100` width `0.03808219`
  - Two shadows (`#AD5100` and `#FFA758`)
  - Accurate stagger entrance matching the 22-char slide + 3-char bounce/rotation sequence
  - Text bounding box ROI SSIM $\ge 0.95$ against reference video frame `1011.mp4`.
- [ ] **T3-03 (Font Fallback):** If proprietary font `MPLUS1p-ExtraBold.ttf` is missing or uninstalled, engine falls back to `Noto Sans ExtraBold` with an explicit diagnostic warning in the project UI.
- [ ] **T3-04 (Unsupported Graceful Reporting):** Unsupported CapCut effects (e.g. 3D extrusions, particles) are logged in the conversion report without corrupting valid timeline tracks.

---

## 5. Milestone T4 — Headless Automation & MCP Gate (End of Phase 35)

- **Target Phase:** Phase 35 (Agentic & Headless Template Automation)
- **Goal:** CLI and MCP tools allow autonomous coding agents and scripts to author, inspect, and render animated text templates headlessly.

### Verification Checklist:
- [ ] **T4-01 (MCP Tool Set):** Existing typed CLI/MCP primitives (`inspect`, `capabilities`, `dry_run`, `apply`, `export`) handle template transactions cleanly over standard JSON-RPC without secret untyped mutations.
- [ ] **T4-02 (Headless Export Consistency):** Export executed via CLI/MCP generates identical pixel-perfect output to export initiated from the GUI.
- [ ] **T4-03 (Agent Error Handling):** Invalid MCP arguments or out-of-bounds text inputs return structured validation error messages with actionable hints.
- [ ] **T4-04 (Headless Performance W4):** Continuous 60-second export throughput achieves $\ge 1.5\times$ real-time speedup on reference host.

---

## 6. Milestone T5 — Hardening, Packaging & Release Gate (End of Phase 36)

- **Target Phase:** Phase 36 (Hardening, Golden Master & Final Acceptance)
- **Goal:** Full regression verification, memory leak qualification, installer package builds, and documentation completion.

### Verification Checklist:
- [ ] **T5-01 (Regression Suite):** 100% passing rate across all Rust cargo unit/integration tests and frontend vitest tests.
- [ ] **T5-02 (Memory & Leak Qualification):** Prolonged editing session (100+ template operations, continuous scrubbing, W4 export loop) stays strictly within the 256 MiB overlay cache ceiling without creeping heap leaks.
- [ ] **T5-03 (Package Footprint):** Final Windows installer package growth is $\le 35\text{ MB}$ over the Phase 26 baseline (59,085,922 bytes / 56.35 MiB / 59.09 MB).
- [ ] **T5-04 (Documentation Sync):** `PROJECT_STATUS.md`, `phase_status.md`, `ARCHITECTURE.md`, and user-facing release notes are completely aligned.
