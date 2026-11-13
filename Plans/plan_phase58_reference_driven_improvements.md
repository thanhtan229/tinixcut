# Phase 58 — Reference-Driven Improvements (MCP safety, LUT, chroma key, scopes, chapters, audio sync)

**Status:** IMPLEMENTED 2026-11-13 (58A–58F). Native preview-vs-export pixel parity for the WebGL effect layer was **not** run; see §8.
**Owner decision basis:** Product Owner chose to keep improving TinixCut rather than fork another editor, and allows copying code from MIT/GPL references for personal, non-distributed use.

## 1. Objective

Close the highest-value functional gaps found by comparing TinixCut with open-source editors. Reuse proven algorithms instead of inventing them, and keep every architectural guardrail in `AGENTS.md` §2.2.

## 2. Evidence (from source reading, not runs)

| Gap in TinixCut | Verified in TinixCut source | Reference to reuse | License |
|---|---|---|---|
| MCP tools carry no `readOnlyHint` / `destructiveHint` | 32 `#[tool]` entries in `src-tauri/src/modules/automation/mcp.rs`, no `annotations(` anywhere; rmcp-macros 3.5.0 supports `annotations(read_only_hint = …)` | blyacut `crates/mcp/src/schema.rs` (annotations on every tool, autonomous vs review mode) | GPL-3.0 → ideas only |
| No 3D LUT | `ColorAdjustments` has brightness/contrast/saturation/temperature only (`domain/mod.rs:545`) | Clypra `wgpu_compositor/lut_parser.rs` (315 lines, std-only `.cube` parser) | MIT → may copy with notice |
| No chroma key | none in `src` / `src-tauri/src` | FFmpeg `colorkey` + `despill` (bundled `resources/bin/ffmpeg.exe` has both); OpenCut-AI `lib/effects/definitions/chroma-key.ts` and Clypra `multi_track_blend.wgsl` for shader structure | MIT |
| No video scopes | none | Clypra `wgpu_compositor/scopes.rs` (CPU, histogram/waveform/RGB parade from RGBA bytes, serde-only) | MIT |
| Markers exist, no chapter export | `ProjectMarker` (`domain/mod.rs:1325`) | — | — |
| No audio-based clip alignment | none | OpenCut-AI `lib/multicam/multicam-sync.ts` (normalized cross-correlation at ~8 kHz, 60 s search window, proposes offsets without mutating) | MIT |

Already present, so out of scope: slip, roll, ripple delete, group move, markers, keyframes, speed, crossfades, captions.

## 3. Key design decision — preview effects layer

The preview currently applies color through an SVG `feColorMatrix` filter on the `<video>` element (`PreviewCanvas.tsx:1864`), and export applies the same matrix with `geq`. SVG filters cannot express a 3D LUT or a chroma key with tolerance and despill.

**Decision (recommended default):** add a **WebGL2 effect layer** in the preview, used only for clips that have a LUT or a chroma key enabled.
- It draws the clip's `<video>` frame into a `<canvas>` with a fragment shader that runs, in this order: **chroma key → color matrix → LUT**. Export uses the same order: `colorkey,despill → geq (matrix) → lut3d`. Document the order in `ARCHITECTURE.md` §17.1.
- The SVG matrix path stays unchanged for every clip without LUT/key, so existing parity evidence (POC2, G52) stays valid.
- WebGL state is transient and never persisted (guardrail 8).
- **No silent fallback:** if WebGL2 is unavailable or the context is lost, the layer shows an explicit "Effect preview unavailable" badge. It never shows the unprocessed frame as if it were processed.
- **Interpolation parity:** export uses `lut3d=interp=trilinear`, which matches WebGL2 hardware trilinear sampling of a `TEXTURE_3D`. Do not use `tetrahedral`.
- **Shader math must mirror FFmpeg:** port the exact formulas from FFmpeg `vf_colorkey.c` (RGB distance, `similarity`, `blend`) and `vf_despill.c` into the shader. This is the parity lever. G52 showed that "similar" math is not enough.

This adds a second preview raster path, which is a durable architectural decision. Update `ARCHITECTURE.md` §15 and §17 in the first step that introduces it (58B).

## 4. Phases (in order)

### 58A — MCP annotations and destructive-edit gate (small; ~0.5–1 day) — IMPLEMENTED 2026-11-13
1. Classify all 32 tools:
   - **Read-only:** `inspect`, `capabilities`, `dry_run`, `media_*`, `*_validate`, `*_compare`, `export_status`, `export_preflight`, `template_validate`, `template_preview`, `edit_plan_*` drafts that only return data.
   - **Mutating, non-destructive:** `apply`, `template_apply`, `export_start`, `project_create`. The last one refuses to overwrite.
   - **Destructive:** `save` (overwrites a file), `workflow_manifest_checkpoint` if it overwrites.
   - Set `idempotent_hint` where true, and `open_world_hint = false` everywhere (local only).
2. Add `annotations(...)` to each `#[tool]`. This is additive metadata and does not change any call contract.
3. **Destructive gate (opt-in, default off):** a server option `requireDestructiveConfirmation`. When it is on, `tinixcut_apply` refuses operation sets containing clip/track deletion unless they carry a `dryRunToken` returned by `tinixcut_dry_run` for the same base revision. Default off keeps existing clients working.
4. **Tests:**
   - `tools/list` asserts the expected annotation for every tool, so a new tool without a classification fails.
   - The gate rejects without a token, accepts with a matching token, and rejects a token from a stale revision.
- **Acceptance:** all 32 tools are annotated, the tests above pass, and the existing MCP suite still passes (49 passed / 3 ignored).

### 58B — Per-clip 3D LUT (~3–5 days, includes the WebGL layer) — IMPLEMENTED 2026-11-13
1. **Domain:**
   - Add `color_lut: Option<ClipLut { name, path, sha256 }>` on video/image clips.
   - The LUT file becomes an asset, so missing/relink semantics apply (guardrail 11).
   - **Step 0:** impact analysis of adding `MediaKind::Lut` (exhaustive matches, media library UI, probe, proxy and waveform jobs must skip it). If the blast radius is too large, use a project-level `lut_assets` catalog with the same fingerprint and relink fields instead. Record the choice before coding.
   - v1 strength is fixed at 1.0. Partial strength (split + blend) is deferred.
2. **Schema v18 migration:** additive, default `None`. Migration test plus v17 → v18 round trip. If 58C lands in the same release, ship one v18 migration carrying both fields.
3. **Parser:**
   - Port Clypra `lut_parser.rs` into `modules/media/lut.rs`, keeping the MIT header and adding an entry to a `THIRD_PARTY_NOTICES.md` (create it).
   - Validate: `LUT_3D_SIZE` between 2 and 65, domain min/max, finite values, entry count.
   - Unit tests: identity LUT, malformed files, size limits.
4. **Command:** `ClipColorLutSet { item_id, lut: Option<…> }`. It goes through the Command Service with validation, undo/redo and revision. It is exposed to CLI and MCP through the existing envelope, with no new mutation path (guardrails 5, 6, 18).
5. **Export:**
   - Append `,lut3d=file='<escaped path>':interp=trilinear` after the color-matrix `geq`, in both the static and animated branches of `composition/mod.rs`.
   - A missing LUT asset is a preflight error, never a silent skip.
6. **Preview:** introduce the WebGL2 layer (§3), loading the LUT as a `TEXTURE_3D` (RGB16F or RGBA8 as measured).
7. **Inspector UI:** choose or clear a LUT, show the missing-asset state. Read `UX_UI.md` §3 first.
8. **Parity tests:**
   - Level 3 offline: a synthetic colour-ramp image through FFmpeg `lut3d` vs a Rust reference trilinear implementation must give MAE ≤ 1.0.
   - Native: preview vs export with the existing capture scripts on a LUT fixture. Pass condition: MAE ≤ 4.0 (§17.2), and SSIM no worse than the same fixture without a LUT minus 0.01. Comparing against the no-LUT baseline keeps the known G52 resampling gap out of the LUT verdict.
   - Negative control: a deliberately different LUT must fail.
9. **Performance:** re-run the G54 UI-response subset with one LUT clip. The p95 ≤ 100 ms budget must still hold.
- **Acceptance:** all of the above pass, `ARCHITECTURE.md` §15/§17 are updated, undo/redo, save/reopen and relink round trips are tested, and MCP `apply` matches the manual command.

### 58C — Per-clip chroma key (~2–3 days, reuses the 58B layer) — IMPLEMENTED 2026-11-13
1. **Domain:** `chroma_key: Option<ChromaKey { key_color: [u8;3], similarity, blend, despill: Option<DespillType>, despill_mix }>`, validated ranges, in the same v18 migration if released together.
2. **Command:** `ClipChromaKeySet`, same rules as 58B.
3. **Export:** `colorkey=color=0xRRGGBB:similarity=…:blend=…` then optional `despill=type=green|blue:mix=…`, placed before the color matrix. The layer must already be in an alpha-capable format (the existing `format=yuva420p` / `rgba` stages). Verify the alpha survives to `overlay`.
4. **Preview:** a shader port of the colorkey and despill math. Composite with premultiplied alpha over lower layers.
5. **UI:** colour field and eyedropper. The eyedropper samples the WebGL canvas with `readPixels`. Check first that the asset-protocol video does not taint the canvas; if it does, sample from the export single-frame path instead.
6. **Tests:**
   - Unit: validation, undo/redo.
   - Offline: the shader math reproduced in Rust vs FFmpeg on a synthetic green-screen frame, alpha MAE ≤ 2/255.
   - Native: parity on a key fixture with the same rule as 58B, plus a negative control.
- **Acceptance:** same structure as 58B.

### 58D — Video scopes, read-only (~2–3 days) — IMPLEMENTED 2026-11-13
1. **Frame source:** the authoritative export composition for one frame (a new internal single-frame render through the existing composition, `-frames:v 1`, output to the derived cache). Do not use the preview, so scopes show what will be exported.
2. **Analysis:** port Clypra `scopes.rs` (MIT, with notice) to compute histogram, luma waveform, RGB parade and vectorscope data from RGBA.
3. **Exposure:** a Tauri query command and a read-only MCP tool `tinixcut_frame_scopes`. There is no project mutation, so the Command Service is not involved.
4. **Job behaviour:** debounced on playhead change, bounded and cancellable (guardrail 14). Results live in the disposable cache.
5. **UI:** a scopes panel drawn on canvas from the data. Read `UX_UI.md`.
6. **Tests:**
   - Synthetic frames (solid colours, ramps) produce known histogram and waveform bins.
   - A superseded request is cancelled.
- **Acceptance:** the tests pass, and the panel stays responsive within the G54 UI budget while scrubbing.

### 58E — Chapter export from markers (small; ~0.5–1 day) — IMPLEMENTED 2026-11-13
1. Markers become a YouTube chapter list.
   - Validate the platform rules: first chapter at 00:00, at least 3 chapters, each ≥ 10 s.
   - Report violations instead of silently fixing them.
2. Optionally embed MP4 chapters in export through an FFmpeg `ffmetadata` input.
3. Read-only MCP tool and CLI subcommand.
4. **Tests:** formatting, rule violations, an embedded-chapter ffprobe check.

### 58F — Audio-based clip alignment (gated; ~3–5 days) — IMPLEMENTED 2026-11-13
1. Rust port of the OpenCut-AI cross-correlation algorithm:
   - Decode mono PCM at 8 kHz through the media abstraction.
   - Use a normalized cross-correlation with a ±60 s window.
   - Evaluate FFT-based correlation for speed. A new dependency (e.g. `rustfft`) needs a justification under `AGENTS.md` §21. The reference uses an O(n·lag) loop.
2. **v1 is alignment only:**
   - A read-only proposal (offset and confidence per clip relative to a reference clip).
   - Applying it uses the existing `ClipGroupMove` commands, so no new domain entity and no schema change.
   - Multicam angle switching and nested sequences are deferred.
3. **Tests:** a synthetic pair with a known offset is recovered within one frame, and low-confidence pairs are reported rather than applied.
- **Gate:** start only after 58A–58D are accepted.

## 5. Explicitly out of scope

- Forking another editor.
- Porting Clypra's wgpu compositor or native surface.
- Embedding MLT.
- Nested/compound sequences.
- Multicam clip entity.
- Stabilization.
- LUT strength below 1.0.
- HDR/BT.2020 colour pipelines.

## 6. Cross-cutting rules

- Every project mutation goes through the Command Service. UI, CLI and MCP share it.
- FFmpeg stays behind `composition` / `media`. File paths in filter strings are escaped by the existing helpers. The frontend never builds FFmpeg arguments.
- Copied MIT code keeps its copyright header and is listed in `THIRD_PARTY_NOTICES.md`. GPL sources are used for ideas only, so the repository licence stays a free choice.
- Each phase updates `PROJECT_STATUS.md` once, at its checkpoint. `RELEASE_READINESS.md` changes only when a phase is accepted.
- **Regression set per phase:** `cargo test --lib`, the affected integration suites, the frontend suite, typecheck, `cargo fmt`, and the release build. Native parity runs use `scripts/qualification/native/`.

## 7. Decisions recorded as defaults (change before the phase starts if needed)

| # | Decision | Default |
|---|---|---|
| D1 | Destructive MCP gate | Opt-in, default off |
| D2 | LUT storage | **Decided 2026-11-13:** per-clip `ClipLut { name, path, sha256 }` (no new `MediaKind`: 119 `MediaKind::` uses would need audit); a missing file is an explicit preflight/preview error and `ClipColorLutSet` with a new file is the relink |
| D3 | LUT strength in v1 | Fixed 1.0 |
| D4 | Effect order | chroma key → color matrix → LUT |
| D5 | LUT interpolation | Trilinear in both preview and export |
| D6 | Scopes frame source | Export composition, not preview |
| D7 | Multicam v1 | Alignment proposal + existing move commands only |

## 8. Implementation record — 2026-11-13

Commits: `27e2c17` (58A), `fb39eda` (58B+58C), `bfbf302` (58D), `6d9bd6a` (58E), `9ff4aea` (58F).

**Deviations from the plan above**

- LUT storage (D2): per-clip `ClipLut { name, path, sha256 }`, no new `MediaKind`. A missing or changed file fails export preflight and shows an explicit badge in the preview; setting a new file is the relink.
- Chroma key and LUT share one schema bump (v18) and one WebGL layer. The key is evaluated **before scale/rotate** (right after crop) so keyed alpha is resampled with the picture and rotation padding stays transparent; the color matrix and LUT follow, matching the pipeline order in `ARCHITECTURE.md` §17.1.
- Chroma keying uses FFmpeg `colorkey` + `despill` (RGB distance), not `chromakey` (YUV). The preview reference was derived from the FFmpeg 8 source (`vf_colorkey.c`, `vf_despill.c`).
- Scopes are computed from the export composition of the live project; text overlays are excluded and labelled. Added MCP statistics tool `tinixcut_frame_scopes`.
- Added MCP tools `tinixcut_lut_probe`, `tinixcut_project_chapters`, `tinixcut_audio_align` (tool count 32 -> 36, all annotated read-only). The destructive-edit gate stays opt-in via `TINIXCUT_MCP_REQUIRE_DESTRUCTIVE_CONFIRMATION=1`.
- MP4 chapter embedding (optional in 58E) and applying alignment proposals automatically were not built: proposals are applied with the existing clip-move commands.

**Evidence (automated)**

- LUT: FFmpeg `lut3d=interp=trilinear` vs the Rust reference sampler, MAE <= 1.0 and max error <= 3 levels on identity, graded and channel-swap LUTs; export turns red into blue with the swap LUT; negative controls fail as designed (`tests/phase58_lut_parity.rs`).
- Chroma key: FFmpeg output vs reference formulas, RGB and alpha error <= 1 level for hard/soft edges and green/blue despill; a keyed top clip shows the track below in a real export (`tests/phase58_chroma_parity.rs`). The TypeScript preview reference is checked against 4 sets of measured FFmpeg vectors (`src/components/__fixtures__/chromaKeyGolden.json`).
- Alignment: a 1.5 s offset is recovered within one frame on decoded audio; unrelated audio is not confident (`tests/phase58_audio_align.rs`).
- MCP: annotation contract for every tool, destructive gate (`tests/phase15_mcp_acceptance.rs`).

**Not verified (open)**

- The WebGL layer's pixels were not compared with an export frame in the running app (no native run; vitest cannot create a WebGL2 context). The shader math is reviewed against the formulas and unit-tested through the TypeScript reference, but real preview-vs-export parity (MAE <= 4.0) for LUT/chroma clips, WebGL2 availability inside WebView2, and the `texImage2D(video)` CORS behaviour on the asset protocol remain to be qualified with `scripts/qualification/native/`. Until then the preview badge states when the effect cannot be drawn, and export is authoritative.
- Scopes panel and Inspector controls were type-checked and unit-tested for their pure parts only; they were not exercised in the native UI.
- G54 UI-response budget was not re-measured with a LUT clip.
