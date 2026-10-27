# Phase 36 — Compatibility & Support Matrix

**Document Version:** 1.0.0  
**Status:** QUALIFIED / RECONCILED  
**Date:** 2026-10-27  

---

## 1. Provenance Classification & Fidelity Tiers

TinixCut distinguishes 5 distinct provenance tiers for animated text templates:

| Tier | Provenance Name | Description | Fidelity | Parameter Binding | Persistence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **T1** | **Native Template** | Created in TinixCut or built-in (`.tinixtpl` / catalog). | 100% Full | Full dynamic schema bindings | Lossless JSON (`schema_version: 11`) |
| **T2** | **Recreated Native** | Native recreation of external motion styles (e.g. CapCut sample recreation). | 100% Full | Static + animated keyframes | Lossless JSON (`schema_version: 11`) |
| **T3** | **Full Converted** | CapCut Scope Option 1: evidenced `StaggeredCharacterIn` family. | Exact visual parity ($\text{SSIM} \ge 0.995$) | Converted to native parameters | Full project round-trip |
| **T4** | **Partial Converted** | CapCut drafts with unsupported animation curves or complex styles. | Text editable; style/motion approximated | Partial overrides | Losses recorded in `ConversionReport` |
| **T5** | **Baked Video Reference** | Unconvertible 3D text meshes, proprietary shaders, external stickers. | Static raster / video playback | Read-only background | Reference media asset |

---

## 2. CapCut Template Import Compatibility Matrix

Following Product Owner decision **Scope Option 1**, full fidelity is guaranteed for the evidenced `StaggeredCharacterIn` family, while other families gracefully degrade with itemized loss reports.

| Family / Feature | Evidenced Sample | Editable Text | Style Fidelity | Motion Fidelity | Overall Status | Notes & Fallback Behavior |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **StaggeredCharacterIn** (Slide + Bounce/Rotation) | `TINIX_CAPCUT_IMPORT_TEST` | **YES** | **FULL** | **FULL** | **FULL PASS** | 29 graphemes, 21-char vertical slide + 3-char bounce/rotation, 118° gradient, 2 outer strokes, 2 shadows. Bit-for-bit parity. |
| **FadeIn / Simple Slide** | Standard Titles | **YES** | **FULL** | **FULL** | **FULL PASS** | Mapped directly to TinixCut native `FadeIn` / `SlideInLeft` presets. |
| **Typewriter Effect** | Caption Typewriter | **YES** | **FULL** | **FULL** | **FULL PASS** | Grapheme-level typewriter unit evaluation. |
| **Curved / Path Text** | Wave Text | **YES** | **PARTIAL** | **APPROX** | **PARTIAL** | Curved baseline converted to horizontal line; loss itemized in `ConversionReport`. |
| **3D Text Extrusion** | 3D Shiny Gold | **NO** | **BAKED** | **BAKED** | **PARTIAL / BAKED** | Imported as baked video reference layer; pure 2D Rust engine does not render 3D meshes. |
| **Audio-Beat Shake** | Music Video Beat | **YES** | **FULL** | **STATIC** | **UNSUPPORTED** | Dynamic audio-reactive triggers deferred to post-v1; text imported as static title. |

---

## 3. Agent CLI & MCP Tool Surface Reconciliation

### 3.1. Tool Count Reconciliation (ADR-005 vs Phase 35)

In Phase 15, `ADR-005` established the initial authoritative MCP server with **8 tools**:
1. `tinixcut_get_capabilities`
2. `tinixcut_inspect_project`
3. `tinixcut_dry_run_commands`
4. `tinixcut_apply_commands`
5. `tinixcut_save_project`
6. `tinixcut_export_start`
7. `tinixcut_export_status`
8. `tinixcut_export_cancel`

Phase 35 introduced the **Agent Template Generation** subsystem, adding **4 new authoritative tools**:
9. `tinixcut_template_validate` (`template_draft_validate`)
10. `tinixcut_template_preview` (`template_draft_preview`)
11. `tinixcut_template_apply` (`template_draft_apply_and_save`)
12. `tinixcut_template_save` (`template_draft_save_to_catalog`)

> [!NOTE]
> **Reconciliation Status:** The active MCP server now exposes **exactly 12 tools**.  
> The stale "exactly 8 tools" text in `ADR-005` line 150, `Plans/plan_phase15.md` lines 37, 80, 158, and the assertions in `src-tauri/tests/phase15_mcp_acceptance.rs` (lines 247, 280, 1834) are documented as known documentation/test reconciliation items for the repository owner to update.

### 3.2. CLI and MCP Operational Equivalence

All template operations provide deterministic, bit-for-bit identical behavior across both interfaces:

| Operation | CLI Command | MCP Tool | Semantics |
| :--- | :--- | :--- | :--- |
| **Capabilities** | `tinixcut template capabilities` | `tinixcut_get_capabilities` | Returns schema version, limits, and supported presets. |
| **Schema** | `tinixcut template schema` | `JsonSchema` for `TemplateDraft` | JSON Schema 2020-12 definition of `TemplateDraft`. |
| **Validate** | `tinixcut template validate <draft>` | `tinixcut_template_validate` | Strict schema + domain + resource validation. |
| **Preview** | `tinixcut template preview <draft> <out>` | `tinixcut_template_preview` | Renders candidate draft to PNG with concurrency cap $\le 4$. |
| **Apply** | `tinixcut template apply <proj> <item> <draft>` | `tinixcut_template_apply` | Applies draft via Command Service transaction with revision fencing. |
| **Save** | `tinixcut template save <draft>` | `tinixcut_template_save` | Saves validated draft into template catalog. |

---

## 4. Hard Validation Limits & Bounds

To ensure memory safety, deterministic execution, and real-time performance on Windows, the following strict bounds are enforced across all layers:

- **Maximum Template Layers:** $\le 16$
- **Maximum Text Slots:** $\le 8$
- **Maximum Graphemes per Slot:** $\le 512$
- **Maximum Outer Strokes:** $\le 4$
- **Maximum Drop Shadows:** $\le 4$
- **Maximum Gradient Stops:** $\le 8$
- **Maximum Concurrent Agent Worker Threads:** $\le 4$
- **Default Agent Job Timeout:** $15.0\text{ seconds}$
- **Bounded Glyph Cache Ceiling:** $256\text{ MiB}$
