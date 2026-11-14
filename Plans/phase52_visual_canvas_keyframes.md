# Phase 52 — Visual canvas and keyframe editing

**Status:** IMPLEMENTED WITH NATIVE QUALIFICATION DEFERRED — canvas geometry and SDR color controls use canonical commands and persisted domain data. Native edit/history/save-reopen and preview/export parity remain open for the final blocker pass.
**Updated:** 2026-11-04
**Plan:** [Product completion, Phases 48–57](plan_product_completion_phases48_57.md)

## Baseline

- The Inspector already exposes numeric position, uniform scale, rotation, opacity, crop and keyframe/easing controls.
- Timeline keyframes already support selection and drag-to-move through `ClipKeyframeMove`; do not duplicate that model.
- Presets were previously rendered with different CSS and FFmpeg filters. SDR adjustments now use one documented affine RGB matrix in preview and export; exact rendered parity still requires native fixture qualification.

## Implemented slice: move directly on canvas

- A selected visible layer has an accessible center handle. Drag updates a transient preview position and commits one `ClipTransformSet` Command Service operation on release.
- Arrow keys move by one project pixel; Shift+Arrow moves by ten. Pointer deltas are converted from fitted viewport pixels into canonical project pixels.
- Ctrl+Arrow scales both axes proportionally by 5%; Ctrl+Shift+Arrow uses 10%. Alt+Arrow rotates by 1 degree; Alt+Shift+Arrow uses 10 degrees. Scale is clamped to 0.05–20 and preserves the existing non-uniform scale ratio.
- If PositionX or PositionY is animated anywhere on the selected item, the drag adds/updates that property's keyframe at the current timeline frame in the same command envelope as the transform, preserving the visible value and a single undo step.
- Canvas keyboard scale/rotation uses one `ClipTransformSet` plus current-frame keyframes for already animated channels in one command envelope. `canvasTransformFromKeyboard` has unit coverage for increments, non-uniform scale preservation, clamping and modifier rejection.
- Scale and rotation pointer handles are positioned around the selected layer bounds. Pointer movement updates a transient preview; pointer-up commits one transform command envelope (including already animated channels at the playhead), while pointer-cancel discards the transient change. Scale drag uses an exponential horizontal response, preserves non-uniform scale ratio, and clamps to 0.05–20; rotation drag changes 0.5 degrees per horizontal pixel.
- Rich/template text handles are omitted during pointer transform because the shared native text renderer bakes transforms into its raster output; use its canonical Inspector/keyframe controls until transient raster preview is supported.
- Inspector `Center position` uses that same position-aware command path. It preserves non-uniform scale and rotation and writes current-frame keyframes for animated position channels.
- `canvasPositionFromPointerDelta` has focused tests for scale conversion and invalid scale handling.

## Implemented slice: safe crop editing

- Inspector exposes Top/Right/Bottom/Left crop percentages for Video and Image clips plus Reset; draft values commit on blur, while unchanged fields do not emit commands.
- Four accessible crop handles are shown for selected Video/Image layers. Dragging previews nondestructive crop in screen space converted to the layer's rotated coordinates; releasing commits one `ClipCropSet`, cancel discards. Focused handles accept arrow keys (Shift for ten-pixel steps) through the same geometry function.
- All crop changes go through `ClipCropSet`. Command Service rejects non-finite/out-of-range values and opposite edges that remove the entire visible width or height, including edits received through automation.
- Rust regression covers valid crop, negative/NaN values, and horizontal/vertical over-crop rollback; the project remains at its prior revision on rejection.

## Implemented slice: SDR color controls

- Schema v17 adds neutral-default `ColorAdjustments` (brightness -1..1, contrast 0..2, saturation 0..2, temperature -1..1); the v16→v17 migration initializes existing items without changing their appearance.
- Inspector exposes four controls and Reset for Video/Image clips. Changes use `ClipColorAdjustmentsSet`, so validation, revision, undo/redo and MCP capability discovery use the existing Command Service path.
- Preview SVG `feColorMatrix` and export FFmpeg `geq` share one gamma-encoded RGB affine formula: preset contrast/saturation, user contrast/saturation, centered contrast, brightness offset, then a bounded red/blue temperature gain. Alpha is preserved. LUTs, HDR and masks remain out of scope.
- Composition order is crop → scale/rotation → color effect → opacity/composite, including transition branches. No renderer graph is persisted.
- Native UI must still compare preview/export pixels at representative values and verify undo/reset/save/reopen. Until then, Phase 52 parity remains OPEN.

## Verification and remaining work

- TypeScript, production build, full frontend suite (137/137), focused canvas geometry (9/9), Phase 52 Rust command/history, color-matrix and crop tests (3/3), v16→v17 migration, FFmpeg `geq` syntax smoke, `cargo check`, format and `git diff --check` pass. Build retains the minified bundle warning (533.16 kB).
- Native drag/keyboard, one-step undo/redo, save/reopen and preview/export parity are not verified in this environment; batch native qualification in the final blocker pass.
- Source audit confirms keyframe selection/drag-move and Inspector easing controls already exist; do not add a duplicate model. Fit/Fill already exists in the Inspector. Native checks remain for canvas/keyframe behavior, effect bypass/reset, persistence and preview/export parity. Other align/reset actions must preserve non-uniform transform values.

### Native canvas and preview smoke — 2026-11-04

- With the imported 1080×1920 video on Track 1, changing Canvas aspect ratio from 16:9 Landscape to 9:16 Vertical committed at rev 24. The native screenshot showed the clip rendered in a 1080×1920 portrait preview.
- Undo restored 16:9 Landscape. A separate no-clip cycle also verified Redo to 9:16 and Undo back to 16:9. Final canvas state is 16:9.
- This is a native preview/canvas check only; no 9:16 export was produced, so export parity at portrait dimensions remains unverified. Other visual effects and keyframe workflows remain open.

### Isolated current-candidate canvas/history check — 2026-11-04

- On the isolated current candidate's blank, unsaved project, Canvas aspect ratio changed from 16:9 Landscape to 9:16 Vertical; the native UI displayed 1080×1920 and committed revision 1.
- Undo/Redo repeatedly returned the displayed preset and preview dimensions to the corresponding 16:9/1920×1080 and 9:16/1080×1920 states. The final in-memory state was restored to 16:9 Landscape (revision 6); the project was not saved.
- This confirms the native canvas preset/history path on an empty project only. Layered composition, keyframes/easing, effect bypass/reset, save/reopen, and preview/export pixel parity remain open.

### Native two-layer transform, keyframe, save/reopen and export smoke — 2026-11-04

- On isolated candidate PID 4140, selected the disposable video clip through its accessible keyboard target. Position X changed 0 to 40; Undo restored 0 and Redo restored 40. Added a PositionX keyframe at offset 30f/value 40; Undo removed the keyframe and Redo restored it.
- Added a Text Track and text clip. Position Y changed 0 to 55; Undo/Redo restored the respective values. Added a PositionY keyframe at text offset 0f/value 55; Undo removed the keyframe and Redo restored it.
- Saved revision 27 as `artifacts/phase52/disposable/phase52_native_layers.tinix`, reopened it in TinixCut, and confirmed the video and text transform values through the native Inspector. The project JSON retains both keyframes. The source fixture hash did not change.
- Native MP4 export completed. ffprobe reports H.264/AAC, 1920x1080 at 30 fps, 450 frames, 15 seconds; full FFmpeg decode exits 0. The representative 2-second export frame shows the video and rendered text; this is a visual smoke only, not quantitative preview/export parity.
- Evidence: `../artifacts/phase52/phase52_native_layers_qualification.json`, project and MP4 under `../artifacts/phase52/disposable/`.
- Remaining: keyframe easing/cut parity, color effect bypass/reset, exact preview/export frame comparison, portrait export parity, and broader two-layer project workflows.
### Native color reset and history smoke — 2026-11-04

- On the reopened two-layer disposable project, selected the video clip. Brightness changed from 0 to 0.15; Undo and Redo restored the corresponding slider values.
- `Reset color` returned brightness to 0. Undo Reset restored 0.15; a final Undo returned it to 0. No post-test save was made, so the saved fixture remains neutral.
- This qualifies native color adjustment history/reset for one slider. It does not qualify rendered preview/export pixel parity or all color channels.

### Phase 52 automated gate recheck — 2026-11-05

- Re-ran `cargo test --manifest-path src-tauri/Cargo.toml --lib phase52` with the isolated Cargo target: all 3 color-matrix, command/history and crop-validation tests passed.
- This confirms the current automated slice; native keyframe/easing, effect bypass, exact preview/export comparison and portrait export checks remain open.

### Native preview/export parity follow-up — 2026-11-05

- Compared the native screenshot at timeline frame 60 (2.0 s) with the exported MP4 frame extracted at the same timestamp. The native preview shows the 1080×1920 source fitted as a portrait layer inside the 1920×1080 canvas. The export frame shows the same market scene enlarged and cropped horizontally, plus a narrow strip at the top. Saved project settings are `Fit`, top crop 5%, position X 40; source hash is unchanged.
- Treat this as an observed preview/export geometry mismatch, not a proven root cause. The static POC2 12/12 pass does not cover this project-specific path. Export metadata and full decode pass but do not close visual parity.
- Keep G52 **OPEN / parity FAIL OBSERVED**. Next diagnostic: reproduce with a minimal one-layer fixture and inspect the generated composition plan/filtergraph before any renderer change.
- Evidence: `artifacts/phase52/phase52_preview_export_parity_mismatch_20261105.json`, preview `artifacts/phase52/disposable/phase52_preview_frame60_crop5_20261105.png`, export frame `artifacts/phase52/disposable/phase52_native_layers_crop5_frame60.png`.
### G52 animated Fit geometry parity retest — 2026-11-05

- Root cause confirmed in two parts: the keyframed export schedule used canvas dimensions instead of source-aware Fit/Fill dimensions; its animated branch also rotates into a diagonal square output but positioned that padded output using the unrotated layer bounds. The combination stretched/cropped the portrait source and displaced the layer.
- `ExportAutomationSchedule` now derives scaled bounds through `CompositionVisualLayer::rendered_size` and centers the even-padded diagonal rotation box before applying Position X/Y.
- Regression test `animated_portrait_fit_uses_source_aware_cropped_dimensions` passes. For source 1080×1920, canvas 1920×1080, Fit, top crop 5%, and PositionX keyframe 40, generated commands specify scale 640×1080 and overlay x=372/y=-88.
- Re-rendered the saved disposable revision 28 fixture to `../artifacts/phase52/disposable/phase52_keyframed_portrait_geometry_fixed_20261105.mp4`. ffprobe: H.264, 1920×1080, 30 fps, 450 frames, 15.000 s (exit 0); full FFmpeg decode exit 0. MP4 SHA-256: `F85CB6317B3B2FB8FF9B1F3369B6EE5106C190474FDC1E5D0DF4C578722356AA`. Matched frame 60: `../artifacts/phase52/disposable/phase52_keyframed_portrait_geometry_fixed_frame60_20261105.png`, SHA-256 `BC9C745E618E4B3C79F73496E8562E94A83732F2F0843126D6AEE32CC462FF7A`.
- Visual comparison at frame 60 now shows the same centered portrait composition as the recorded native preview, instead of the prior enlarged/cropped image. This is a qualitative comparison against the saved pre-fix native screenshot; native recheck on the rebuilt runtime is still blocked because Computer Use is unavailable in this session.
- G52 is **OPEN**: this targeted preview/export geometry mismatch is corrected and independently rendered, but native verification on the updated runtime, easing/cut parity, color effect bypass/reset render parity, and broader layer/effect coverage remain.

### G52 native recheck attempt — 2026-11-05

- The standard workspace TinixCut (PID 12440, Core Healthy) was showing the disposable G51 project. The intended G52 fixture exists and its current SHA-256 is `D14535F3049456AC789D33D69220639862DA96C4FB9AA04F2A18B5609E000F4E`; the referenced Hoi An fixture still hashes to `EF2400D263622DA266D0A378B59BA0290ED7EB6475EDAE46E239D8F110B66A5E`.
- The native Open dialog was opened through the exact accessible `📂 Open` control. UIA found the File name edit but `.SetFocus()` returned exactly `Target element cannot receive focus`; sending `WM_SETTEXT` did not produce filename readback. The dialog was canceled through its exact native Cancel control; no project was opened, edited, or saved.
- **G52 remains OPEN.** The corrected export frame and regression test are existing evidence, but a fresh native preview recheck on the updated runtime and remaining easing/cut, bypass/reset and broader-layer parity are still unqualified. No retry was made after the focus/readback failure.

### G52 locked parity tolerance — 2026-11-12 (set before measurement)

Chosen by the Product Owner on 2026-11-12 before any measurement below. Do not relax after seeing results.

- **Preview sample:** CDP screenshot of the preview canvas element in the native app at device pixels, at the target frame after all video layers report `seeked` and text overlays are idle.
- **Export sample:** the same timeline frame extracted from the exported MP4, scaled (bicubic) to the preview sample's size.
- **Metric 1:** SSIM on luma over the full canvas ≥ **0.92**.
- **Metric 2:** for each layer, the centroid of its rendered region differs by ≤ **1 %** of the canvas width (measured on a per-layer isolation render where needed).
- **Sample points:** at a keyframe, mid-easing, and on both sides of a cut, on a fixture with at least two layers.
- **Reset:** after changing an effect and invoking its Reset, preview and export must again match the original sample within the same tolerance. The product has no effect *bypass* control; only Reset is qualified.

### G52 native parity run against the locked tolerance — 2026-11-12

- **Fixture** (built through the Command Service, saved rev 35): portrait video layer (Fit, crop top 5 %) with PositionX 40→300 and Scale 1.0→1.5 EaseInOut over frames 30–150, split at frame 210 with the right half slipped to source 0; text layer `G52 Hội An – kiểm tra` with PositionY 55→−200 EaseIn.
- **Bug found and fixed:** whenever keyframed scale exceeded 1, export ignored PositionX and cropped the layer (frame 90: centre +11 instead of +170; frame 150: −15 instead of +300). The animated branch's `rotate` output (`hypot(iw,ih)`) is evaluated once at the first frame, but overlay positions were computed per frame for the current box. Each animated layer now uses one fixed rotate box sized for its largest scaled frame, shared by the filter graph and the schedule. Regression: `animated_scale_uses_one_rotate_box_for_the_largest_frame`; Rust lib 317/317, `export_automation_poc` 8/8.
- **Parity after the fix** (keyframe 30, mid-easing 90, end keyframe 150, cut 209/210): layer centroids match within 1.6–3.6 px (limit 19.2 px) — **PASS**. Full-frame luma SSIM is 0.908 / 0.900–0.902 / 0.928 / 0.932 / 0.912 with both Balanced and High quality exports — **FAIL at 3 of 5 points** against 0.92. Frame alignment was verified (frame N is the best match among N±2); the residual difference is edge/texture detail from different resampling paths, not layout.
- **Reset:** Brightness 0 → 0.3 through the native slider, then **Reset color**: preview is identical to the baseline (SSIM 1.000), and the post-reset export frame is bit-identical to the original export. Undo/Redo across the edit restored 0.3 / 0 with increasing revisions. The product has no effect *bypass* control.
- **Save/reopen:** saved through the UI and reopened; keyframes, easing, text and color values are preserved.
- **Disposition: G52 OPEN.** Geometry, animation, cut, reset, history and persistence pass; full-frame SSIM misses the locked threshold at 3 of 5 points. Per the gate, effects without parity must not be enabled for release — this miss is not specific to an effect, so the next decision is whether preview should resample like export or the release claims should exclude pixel-level parity. Evidence: `artifacts/phase52/disposable/g52_parity_20261112/g52_qualification_20261112.json`.

### G52 parity closure plan — 2026-11-12 (proposed, not implemented)

**Evidence from re-analysing the 2026-11-12 run** (scripts in `tmp/g52diag/`, inputs are the recorded PNGs; no new native run):

- The video region causes 86–96 % of the SSIM deficit. Text-only isolation SSIM is ≥ 0.992 at every point, and the background is ≈ 0.99.
- The preview capture is 1080×608 device pixels (720×405 CSS at DPR 1.5). `compare_parity.py` downsamples the 1920×1080 export to that size, so the export is resampled twice and the preview once. At scale 1.0 (frames 30 and 210), the preview has 22 % more horizontal gradient energy than the export.
- One preview frame was already captured at native canvas resolution (`dpr_native_res/preview_f90.png`, 1920×1080). Against the unscaled export, the worst point (frame 90) scores **0.938**, compared with 0.900 at preview size. The other four points have not been captured at native resolution.
- The export carries no colour tags (`color_space/primaries/transfer/range = unknown`), which matches the 1–2 level mean luma offset.
- CRF 18 does not change SSIM, so encoder compression is not the cause.

**Decision required before step 1 (Product Owner):** measure at canvas resolution (1920×1080 for this fixture), comparing preview and export without resampling either one. The 0.92 / 1 % thresholds stay unchanged. This corrects the method, it does not relax the threshold. The preview-size number must still be reported next to it. If this is not accepted, steps 2–3 still go ahead, and G52 closes only if preview-size SSIM reaches 0.92.

**Steps**

1. **Measurement tooling** (`scripts/qualification/native/`):
   - `capture_preview.py`: capture the canvas rect at `scale = canvasWidth / rectCssWidth` through `Page.captureScreenshot` clip. This reuses the `dpr_native_res` path, which becomes the default, and records the scale in `preview_capture.json`.
   - `compare_parity.py`: compare at canvas resolution, resample only when the sizes differ, add per-region deficit shares (video, text, background), and emit both the canvas-resolution and preview-size SSIM.
   - Add a synthetic self-check: identical images give 1.0, a shifted layer gives the expected centroid error.
2. **Export colour tagging** (`src-tauri/src/modules/composition/mod.rs`, export args):
   - Tag SDR output as BT.709 limited (`-colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv`).
   - Make the RGB→YUV conversion explicit so the tags are truthful.
   - Add a unit test on the arg list and an ffprobe assertion in one existing render acceptance test.
3. **Export resampling quality**, only if A/B shows a gain:
   - Render the G52 fixture with the `scale` filters on the default flags vs `lanczos+accurate_rnd+full_chroma_int`, using the same filtergraph through the test helper.
   - Adopt the variant only if native-resolution SSIM improves by ≥ 0.005 at the worst point and export time on the Phase 16/17 benchmarks grows by ≤ 10 %.
   - Implement as one constant used by all three `scale=` sites (lines ~1420/1703/1813), and update the `scale=768:486` assertion and `phase34_capcut_import.rs`.
4. **Native re-run (automated, no manual GUI):**
   - Rebuild, then re-capture all five points (30, 90, 150, 209/210) at canvas resolution, plus the text isolation.
   - Re-export Balanced and HQ, then recheck Reset color, Undo/Redo and save/reopen on the rebuilt runtime.
   - Record JSON under `artifacts/phase52/disposable/g52_parity_<date>/`.
5. **Regression:** lib tests, `phase52`, composition/export unit tests, the render suites `phase14/16/17/18/26/34/35` (Phase 28 with `CARGO_PROFILE_TEST_OPT_LEVEL=3`), `cargo fmt`, frontend suite (no frontend change is expected), and NSIS rebuild plus installed render smoke.
6. **Disposition and docs:**
   - **PASS** only if every point is ≥ 0.92 at the agreed resolution and centroids are ≤ 19.2 px.
   - Otherwise G52 stays OPEN, and v0.1.0 keeps the "no pixel parity claim" scope in `RELEASE_READINESS.md`.
   - Update this file, `RELEASE_READINESS.md` and `PROJECT_STATUS.md`.

**Out of scope:** changing the preview renderer (it is CSS-transformed `<video>`), an effect bypass control, and 9:16 portrait-canvas export parity (tracked separately above).

### G52 closure attempt result — 2026-11-12 (export scale A/B; G52 stays OPEN)

- The earlier "0.938 at canvas resolution" figure is **not valid evidence**: `dpr_native_res/preview_f90.png` and `upscale_check/preview_f90.png` are pixel-identical, i.e. the 1080 px preview capture upscaled. WebView2 does not render the canvas at a higher density, so there is no real native-resolution preview to compare against. Step 1 of the plan above is withdrawn.
- A/B of the export `scale` filters (all four sites, including the animated `scale@sc_N` path) with `flags=bicubic` vs `flags=lanczos`, same fixture and preview captures: SSIM 0.9079/0.9001/0.9275/0.9316/0.9118 vs 0.9073/0.8993/0.9271/0.9312/0.9111. Lanczos gives no gain (marginally worse), so the adoption rule (≥ +0.005) is not met. The experimental code was reverted; export is unchanged.
- BT.709 tagging was not implemented: sources are untagged too, tags do not change pixel values, and no evidence shows they affect SSIM.
- Conclusion: the residual difference comes from the preview path (browser video scaling/compositing), not from export resampling. It cannot be closed by an export change. **Disposition unchanged: G52 OPEN; v0.1.0 does not claim preview/export pixel parity.** Reopening requires a Product Owner decision to either relax the SSIM threshold or change the preview renderer (out of scope for v0.1.0).

### G52 disposition — approved by Product Owner, 2026-11-12

G52 stays **OPEN for pixel-level preview/export parity** and v0.1.0 ships with that non-claim. The locked SSIM ≥ 0.92 threshold is **not** relaxed. Geometry, animation, cut, Reset, history and persistence are qualified. Reopening for a later release requires changing the preview renderer so its video scaling matches export (new architecture/plan item), then re-measuring against the unchanged threshold.

### G52 disposition update — 2026-11-14 (approved in chat by the project owner)

Superseding the 2026-11-12 disposition. The locked tolerance (luma SSIM >= 0.92 at a keyframe, mid-easing and both sides of a cut; centroid <= 1 % of the canvas width; Reset returns to the baseline) is unchanged. Two things changed: the comparison geometry was corrected (the 2026-11-12 capture stretched the export by half a pixel vertically and included the viewport border), and a real export defect was fixed (4:2:0 `overlay` floored odd positions, moving layers 1 px up/left of the preview). A paused-frame renderer, **Exact frame**, shows the frame as the export compiler renders it.

**Claim now made:** with **Exact frame** on, a paused frame outside a cut transition is the exported picture (locked method, DPR 1 and 2, no shifts: original fixture 0.956-0.989 at all ten points, natural-footage fixture 0.927-0.992 at all ten points, centroids <= 3.6 px of 19.2; Reset identical to the baseline; undo/redo and save/reopen verified; reopened frames identical).

**Not claimed:** playback and frames inside a transition, which stay the browser's approximation (measured 0.80-0.97 on detail-rich footage with the live `<video>` view); reversed clips; text below a video layer. These limits belong in the release notes. G52 is therefore **CLOSED for paused frames** and **remains a documented limitation for playback**. Evidence and method: `Plans/plan_g52_preview_export_parity.md` sections 9-12; artifacts under `artifacts/phase60/g52/`.
