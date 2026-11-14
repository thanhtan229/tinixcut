# Plan — G52 preview/export pixel parity

**Status:** Steps 0–1 EXECUTED 2026-11-14 (§9): the corrected measurement passes on the original fixture. Step 3 (§10) failed on a second fixture; Step 2 (§11) found and fixed a real export defect (overlay flooring) that lifts it to 3 of 5 points, and shows the remaining gap is the browser's video downscale. G52 stays OPEN; Step 4 or a Product Owner decision is needed. Drafted 2026-11-14, revised after a geometry re-analysis (§1.3–1.5). No product code was changed to write this plan; the evidence comes from measurements on recorded artifacts and from reading reference repositories.

**Gate being closed:** G52 in `Plans/phase52_visual_canvas_keyframes.md`. The locked tolerance (set by the Product Owner on 2026-11-12, before any measurement) stays exactly as it is:

- luma SSIM over the full canvas ≥ **0.92** at a keyframe, mid-easing and both sides of a cut;
- per-layer centroid difference ≤ **1 %** of the canvas width;
- Reset returns to the baseline.

The locked *method* says: "the same timeline frame extracted from the exported MP4, scaled (bicubic) to the preview sample's size". This plan keeps bicubic and the thresholds. What it corrects is the definition of the preview sample: the sample must contain exactly the canvas, at the canvas aspect ratio, and nothing else (§1.3–1.4). It does not fit shifts per sample.

---

## 1. What is known

### 1.1 Recorded result

Fixture `g52_parity` (rev 35): portrait video layer with PositionX 40→300 and Scale 1.0→1.5 (EaseInOut), a cut at frame 210, and an animated text layer. Luma SSIM, preview capture against the export frame resampled to the capture size:

| Frame | 30 | 90 | 150 | 209 | 210 |
|---|---|---|---|---|---|
| SSIM | 0.912 | 0.904 | 0.930 | 0.934 | 0.915 |

Centroids pass (1.6–3.6 px of a 19.2 px limit). Text-only isolation is ≥ 0.992. The video region carries 86–96 % of the deficit.

### 1.2 The comparison filter is not the cause

Changing the filter that resamples the export frame (bicubic, bilinear, area, lanczos, nearest; `ffmpeg ssim`, luma) moves SSIM by about 0.01 (0.890–0.927 over all five points). A 0.3–0.8 px Gaussian blur or sharpening does not help either (0.905–0.935). The two images do not differ in softness.

### 1.3 The comparison stretches the export vertically

The preview canvas is 720 × 405 CSS px at device pixel ratio 1.5, so the canvas covers 1080 × **607.5** device pixels. The capture is 1080 × 608 and the export (1920 × 1080) is resized to 1080 × 608, a vertical scale of 0.56296 against 0.5625 horizontally. The export is therefore stretched by half a pixel from top to bottom relative to the preview. This matches the diagnostic that the best vertical shift grows from the top to the bottom of the frame (−0.25 / −0.50 / −1.00 px for the top, middle and bottom thirds).

Re-analysis on the recorded PNGs, with the export resampled **uniformly** at the real scale (0.5625 on both axes, bicubic, origin at the capture origin) instead of being stretched to 608 rows. Nothing is fitted; the scale comes from the CSS size and the device pixel ratio:

| Frame | 30 | 90 | 150 | 209 | 210 |
|---|---|---|---|---|---|
| Current method (stretched) | 0.912 | 0.904 | 0.930 | 0.934 | 0.915 |
| Uniform scale, full capture | **0.942** | **0.942** | **0.963** | **0.968** | **0.945** |
| Uniform scale, 6 px border excluded | 0.945 | 0.947 | 0.966 | 0.971 | 0.949 |

Script: `_ref/g52_geom.py` (outside the repository; it is moved to `scripts/qualification/native/` in Step 1). Every point is above 0.92, with a margin of at least 0.022. **Most of the recorded miss comes from the comparison, not from how the preview draws the video.** This contradicts the 2026-11-12 conclusion that the residual came from browser video scaling.

### 1.4 The capture also contains editor chrome

`.canvas-viewport` is `box-sizing: border-box` with a `1px` border and a `4px` border radius (`PreviewCanvas.tsx`, inline style). `capture_preview.py` clips to `getBoundingClientRect()`, so the capture includes the border: column 0 and the last column of `preview_f30.png` have the border colour (luma ≈ 46), and the corners are rounded. The export has neither.

There is also an open question about where the layers start. CSS predicts that absolutely positioned layers start inside the border, at (1.5, 1.5) device px, and that the composition (computed for 720 × 405) is clipped by 2 CSS px on the right and bottom. The data does not agree: an origin of (1.5, 1.5) scores 0.778 at frame 30, against 0.945 for (0, 0). Until a static target settles this (Step 0), the origin is unknown. If CSS is right after all, the preview **crops 2 CSS px (≈ 5 canvas px) of the right and bottom edges**, which is a small product defect of its own.

### 1.5 What remains after the geometry correction

A diagnostic search (bicubic shift of the export, ±1.5 px in 0.25 px steps, fitted per sample) still finds a better match at about dx +0.5, dy −0.5 px (0.976 at frame 30). This is a residual sub-pixel offset of unknown origin. It is a **diagnostic, not a correction**, and it is not needed to pass: the uniform-scale numbers already pass without it. It is a follow-up (Step 2), not a gate condition.

### 1.6 Other facts that bound the work

- With no effect, natural footage, static: preview versus export has luma SSIM **0.991** (Phase 58 §10).
- The WebGL canvas path used for LUT/chroma loses 0.025–0.035 SSIM against the `<video>` path even with an identity LUT (Phase 58 §10). Moving all video to that path is **not** a safe default.
- Browser and FFmpeg decode saturated, untagged synthetic content differently (Phase 58). Parity fixtures use natural BT.709 footage, or tagged synthetic content.
- Export-side changes were already tried: lanczos instead of bicubic changed nothing.
- Seek latency (G54) is still failing (p95 2556 ms against 500 ms), so any design that renders a frame through FFmpeg on every seek is expensive.

### 1.7 Governance

On 2026-11-12 the Product Owner approved: "Reopening requires changing the preview renderer so its video scaling matches export, then re-measuring against the unchanged threshold." That condition rested on the conclusion that §1.3 now contradicts. Closing G52 through a corrected measurement, without a renderer change, is a different route, so it **needs an explicit Product Owner decision** (Step 1, decision point). The thresholds are unchanged either way.

---

## 2. What other projects do

Read from the cloned repositories under `_ref/`.

| Project | How preview and export relate | Lesson for TinixCut |
|---|---|---|
| **Clypra** (Tauri + wgpu) | One wgpu renderer for preview and export, with a required audit of five consumers (editor, native preview, browser, export, persistence) for every media/effect change. | Parity by construction needs one renderer; costly, and TinixCut's export is FFmpeg (guardrail 16). The audit discipline is cheap and worth copying. |
| **Mono Cut** (Rust + FFmpeg) | Preview plays pre-rendered regions produced by the export compiler, cached by render identity. | Parity by construction through the same FFmpeg graph at lower resolution; playback is not live. |
| **blyacut** (Rust + WebView) | HTML5 `<video>` preview documented as approximate, plus an exact `preview_frame` render of one frame through the export path. | A parity frame is a cheap, honest tool for the paused state and for QA. |
| **OpenCut-AI** (web) | Canvas/WebGL compositor for preview; separate export path; no pixel-parity claim. | A second compositor does not match an FFmpeg export by accident. |
| **Remotion** | Preview and render use the same React tree in the same browser engine. | Not applicable: TinixCut's export is FFmpeg. |

Conclusion: parity by construction needs one renderer or the export compiler reused for preview frames. Given §1.3, neither is needed to meet the locked threshold on the current fixture; Option C stays available as a fallback.

---

## 3. Options

| Option | What it does | Cost | Verdict |
|---|---|---|---|
| **A. Correct the measurement geometry** | Capture exactly the canvas (no border, no corners), at a device geometry where the canvas maps to whole pixels at its own aspect ratio; compare with bicubic at a uniform scale. Registration self-check with controls. | Small (scripts only). | **Do first.** Offline numbers already pass (§1.3). |
| **A′. Fix the viewport border, if Step 0 confirms the 2 px crop** | Draw the frame around the canvas without taking space from it (e.g. `outline` or a wrapper), so the composition is not offset or cropped. | Small UI change. | Only if Step 0 shows the crop or offset. |
| **B. Locate the residual sub-pixel offset** | Find the cause of the ≈0.5 px residual (§1.5): CSS transform origin and rounding, export `overlay` integer positions, scale rounding to even sizes. | Small to medium. | Follow-up; not required for the gate unless Step 1 fails. |
| **C. Parity frame (paused-state renderer)** | Render the current frame through the export compiler at preview size while paused; playback stays on `<video>`. | Medium. | Only if Step 1 fails at the corrected geometry. |
| **D. Region pre-render** | Play pre-rendered regions from the export compiler. | Large. | Out of scope for v0.1.x. |
| **E. One WebGL compositor for all layers** | Replace the CSS/SVG layer stack. | Very large; WebGL video path measured 0.03 worse. | **Rejected.** |
| **F. Relax the threshold** | — | — | **Not permitted.** |

Guardrail check: A and B change tools and conventions only. A′ is a presentational CSS change. C consumes the same `CompositionPlan`, keeps FFmpeg behind the media/render abstraction (guardrail 16) and puts no renderer state into the project (guardrail 8).

---

## 4. Plan

Every step has a go/no-go. Nothing below a failing step starts. The verdict rule in Step 1 is fixed **before** any new capture is taken.

### Step 0 — Registration self-check (tools only)

**Goal:** know exactly where the composition sits in the capture before trusting any SSIM.

1. Registration target fixture, built through the Command Service: a high-contrast grid with a ruler and corner marks, once as an **image** layer and once as a **video** layer (a short lossless-ish H.264 encode of the same grid, tagged BT.709), each at scale 1.0, no keyframes, filling the canvas.
2. Capture at the current geometry and measure, by phase correlation with a sub-pixel peak, the scale and the offset of the grid in x and y, per third of the frame. Also record the outermost visible grid line on each edge, to settle whether 2 CSS px are cropped (§1.4).
3. **Controls:** identical pair → SSIM 1.000 and offset 0; a deliberate 1.0 px shift → detected as 1.0 ± 0.1 px; a deliberate 0.1 % vertical stretch → the top-to-bottom drift of §1.3.
4. **Go/no-go:** the chain is accepted when the controls behave as stated. The measured origin and scale become the capture geometry used in Step 1; they come from the static target, never from the parity fixture.
5. If the grid shows the 2 px crop or a 1.5 px offset, record it as a product defect and do A′ before Step 1 (it changes what the preview shows, so it must be fixed before measuring).

### Step 1 — Capture and compare at exact geometry (Option A)

1. `capture_preview.py`: clip to the composition area found in Step 0 (inside the border), and hide the border radius during the capture (measurement-only style, like the HUD). Record the device metrics, the clip and the origin next to each capture; abort if the PNG size differs from the expected size.
2. Capture at two device pixel ratios where the canvas maps to whole pixels: **DPR 1** (720 × 405) and **DPR 2** (1440 × 810), through `set_dpr.py`/`Emulation.setDeviceMetricsOverride`. Both scales (0.375 and 0.75) are uniform.
3. Compare against the export frame resized with **bicubic** (the locked filter) to the same size. No shift, no per-sample fitting. Report the old DPR 1.5 / 1080 × 608 number and the §1.3 uniform-scale number as additional columns.
4. Five sample points of the existing fixture, plus the Step 0 target.
5. **Verdict rule (fixed now):** a point passes only if it is ≥ 0.92 at **both** DPR 1 and DPR 2; centroids ≤ 1 % at both.
6. **Decision point:**
   - all points pass → present the result to the Product Owner, asking to accept the corrected measurement as closing G52 for this fixture (§1.7). Continue with Step 3 regardless; G52 closes only with that approval;
   - otherwise → Step 2.

### Step 2 — Residual offset (Option B), only if Step 1 fails or as a recorded follow-up

1. With the Step 0 target, measure the residual translation and scale for the video layer alone, then with Fit, crop and scale applied one at a time.
2. Inspect in this order: (a) preview CSS transform origin and rounding of `left/top`, scale and rotate; (b) export `scale` rounding to even sizes, `rotate` box and `overlay` integer positions; (c) crop edges; (d) the half-pixel convention of the bicubic upscale at scale 1.5.
3. Fix the side that is wrong, with a regression test asserting a measured offset ≤ 0.25 px at export size. If export changes, run the render suites (`phase14/16/17/18/26/34/35`) and `animated_scale_uses_one_rotate_box_for_the_largest_frame`.
4. If Step 1 passed, this step is optional and is recorded as a follow-up, not a gate condition.

### Step 3 — Second fixture and the full gate

Run the whole gate (keyframe, mid-easing, both sides of the cut, Reset, undo/redo, save/reopen) on a second fixture built from **natural BT.709 footage** with two video layers, and on the original fixture, with the Step 1 method and verdict rule. Record JSON under `artifacts/phase52/disposable/g52_parity_<date>/`, with device metrics, the self-check results and the controls.

### Step 4 — Parity frame, only if Steps 1–2 leave a real gap (Option C)

1. `preview_parity_frame(frame, width)` renders one frame through the export compiler at preview size from the same `CompositionPlan` (`-frames:v 1`), returns PNG bytes, cached by `(project revision, frame, width)` with a small bound.
2. Shown when the playhead has been paused for a short delay; the live `<video>` view returns on play or scrub. A visible "exact frame" indicator.
3. Go/no-go on cost: ≤ 300 ms per frame at preview size; seek (G54) unaffected because the live path is untouched.
4. The honest claim becomes: paused frames are rendered by the export compiler; live playback is an approximation measured at SSIM X.

### Step 5 — Regression, documents, disposition

1. Tooling-only outcome (Steps 0, 1, 3): the qualification scripts run on the recorded artifacts and the controls pass; no product suites are needed. If A′, B or C changed product code: Rust lib and every integration suite, the frontend suite, `cargo fmt`, and an NSIS rebuild with an installed render smoke when export changed.
2. Update `Plans/phase52_visual_canvas_keyframes.md` (including the correction of the 2026-11-12 conclusion), `RELEASE_READINESS.md`, `PROJECT_STATUS.md`; `ARCHITECTURE.md` only if Step 4 is built.
3. **Disposition rule:** PASS only when every sample passes the Step 1 verdict rule on both fixtures **and** the Product Owner accepts the corrected measurement. The report carries the old-geometry numbers, the new numbers and the controls. Anything else leaves G52 OPEN.

---

## 5. Acceptance and controls

- Locked thresholds and the bicubic filter unchanged. The verdict rule (both DPR 1 and DPR 2) is fixed before capturing.
- The capture geometry comes from the static target (Step 0), never from the parity fixture.
- Registration self-check passes (identical = 1.000; 1.0 px shift detected; 0.1 % stretch produces the drift).
- No fitted shift in any verdict; fitted shifts appear only in diagnostics, labelled as such.
- At least two fixtures, one on natural BT.709 footage.
- Results reported at all geometries so the effect of the correction is visible.

## 6. Risks

| Risk | Handling |
|---|---|
| The Product Owner does not accept a measurement-only closure | Report the numbers; G52 stays OPEN; Step 4 is the renderer-side route. |
| DPR 1 or DPR 2 renders differently from the user's DPR 1.5 | Both must pass; the DPR 1.5 uniform-scale number is reported too. |
| `setDeviceMetricsOverride` changes the layout or the canvas fit | Record metrics; abort the capture when the PNG size or the Step 0 origin is not as expected. |
| Step 0 confirms the 2 px crop | Fix A′ first; it is a real preview defect independent of SSIM. |
| Pressure to close the gate | The disposition rule forbids closing on a fitted or relaxed result, or without the approval in §1.7. |

## 7. Effort

| Step | Estimate |
|---|---|
| 0 Self-check and target fixture | 0.5–1 day |
| A′ Border fix (if confirmed) | 0.25 day |
| 1 Exact-geometry capture and compare | 0.5 day |
| 2 Residual offset (if needed) | 1–3 days |
| 3 Second fixture and full gate | 1 day |
| 4 Parity frame (if needed) | 3–4 days |
| 5 Documents (tooling-only outcome) | 0.5 day |

Expected path given §1.3: Steps 0, 1, 3 and 5, about 2.5–3 days, plus a Product Owner decision. Steps 2 and 4 are contingencies.

## 8. Not in scope

A second compositor for all layers (Option E), pre-rendered playback regions (Option D), an effect bypass control, 9:16 portrait-canvas export parity (tracked separately), and any change to the locked tolerance.

---

## 9. Results of Steps 0–1 (2026-11-14, native app, dev build)

Tooling: `make_registration_grid.py`, `registration_fit.py`, `capture_dpr.py`, `compare_exact.py` in `scripts/qualification/native/`; project writer `src-tauri/tests/phase60_g52_registration.rs`. Evidence: `artifacts/phase60/g52/` (untracked).

### Step 0 — registration of the capture chain

Controls on the grid image: identity offset 0.000; a 1.0 px shift measured 0.999 px; a 0.1 % vertical stretch measured 1.00097; a 1080×608 resize measured y-scale 0.56296 against 0.5625 in x. Resolution about 0.03 px.

Grid video at 720×405 CSS (border box), measured in the real preview:

| DPR | x scale (expected) | x offset | y scale (expected) | y offset | canvas end |
|---|---|---|---|---|---|
| 1 | 0.37500 (0.375) | +0.06 px | 0.37516 (0.375) | +0.11 px | 720.06 × 405.28 |
| 1.5 | 0.56250 (0.5625) | +0.02 px | 0.56290 (0.5625) | +0.03 px | 1080.01 × 607.96 |
| 2 | 0.75000 (0.75) | +0.02 px | 0.75061 (0.75) | −0.02 px | 1440.03 × 810.64 |

- **The §1.4 hypothesis is refuted:** the composition starts at the border-box origin (offset ≈ 0) and fills the 720×405 box. There is no 1.5 px inset and no 2 px crop, so **Option A′ is not needed**.
- **Horizontal scale is exact.** The vertical scale is 0.04–0.08 % high at DPR 1 and 2 (0.3–0.6 px over the height). Residuals after the linear fit are ≤ 0.1 px, so this is a real, small vertical overshoot of the preview (about 0.3 CSS px), not noise. At DPR 1.5 the screenshot additionally stores 607.5 px in 608 rows. Cause not located; it belongs to Step 2 as a follow-up.
- The 1 px (device) border is painted over the outer edge of the composition. The locked tool already trims 3 px.
- Controls limitation: only the video-layer grid was measured; the image-layer variant of the target was not.

### Step 1 — locked method at exact capture size, no shifts

Original fixture (`g52_parity.tinix`, rev 35), preview captured at DPR 1 (720×405) and DPR 2 (1440×810) with the viewport border radius and shadow removed during the capture; export frames from the 2026-11-12 run of the same project revision; export resized by bicubic to the capture size; luma SSIM from `compare_parity.ssim` (Gaussian 11, 3 px margin) unchanged:

| Frame | 30 | 90 | 150 | 209 | 210 |
|---|---|---|---|---|---|
| Recorded 2026-11-12 (DPR 1.5, 1080×608) | 0.912 | 0.904 | 0.930 | 0.934 | 0.915 |
| **DPR 1** | **0.9667** | **0.9478** | **0.9356** | **0.9409** | **0.9706** |
| **DPR 2** | **0.9769** | **0.9456** | **0.9729** | **0.9785** | **0.9819** |

Centroid error (limit 1 % of 1920 = 19.2 px): DPR 1: 1.7 / 2.7 / 0.7 / 0.8 / 0.4 px; DPR 2: 1.0 / 2.8 / 1.7 / 1.7 / 1.0 px.

**Verdict under the rule fixed before capturing (≥ 0.92 at both DPR 1 and DPR 2, centroids ≤ 1 %): PASS at all five points, minimum 0.9356.** No shift or fit was used.

### What is not yet done

- Step 3 (second fixture on natural BT.709 footage; Reset, undo/redo, save/reopen re-run with this method) is not done.
- The export frames were reused from 2026-11-12, not re-exported; the project revision (35) and media are the same.
- Dev build, not the installed build.
- **G52 is not closed.** §1.7 requires the Product Owner to accept the corrected measurement; the disposition rule in Step 5 also requires the second fixture.

---

## 10. Step 3 — second fixture (2026-11-14): FAIL

Fixture `artifacts/phase60/g52/nat.tcproj`: two natural 1920×1080 clips remuxed with BT.709 tags (`nat_a`, `nat_b`); V1 = A (frames 0–210, PositionX 0→200 and Scale 1.0→1.4 EaseInOut over frames 30–150) then B at scale 1.2 (frames 210–450); V2 = a 0.4-scale overlay of A (frames 30–180, PositionX 600→−600). Same method as Step 1 (DPR 1 and 2, no shifts, locked luma SSIM, bicubic):

| Frame | 30* | 90 | 150 | 209 | 210 |
|---|---|---|---|---|---|
| DPR 1 | 0.612 | 0.584 | 0.815 | 0.838 | 0.950 |
| DPR 2 | 0.500 | 0.761 | 0.727 | 0.767 | 0.995 |

\* the capture at frame 30 reported "not settled" (seek still in flight), so that column is not a valid measurement. The other four are.

**Verdict under the fixed rule: FAIL** (frames 90, 150 and 209 fail at both DPRs). Frame 210 (clip B at a static scale 1.2, no keyframes) passes at 0.95 / 0.995, so the failure is tied to the **animated** clip, not to natural footage or scaling in general.

Diagnostics (fitted per sample, labelled as such, never used in a verdict): the best global shift of the export is +0.5 px / +0.5 px (frames 150, 209) and +1.0 / +1.0 px (frame 90) at DPR 1, and restores SSIM to 0.92 / 0.96 / 0.89. The best-matching export frame for the preview at frame 90 is frame 90–91 (0.584 / 0.599), so the preview is not a whole frame off in time.

Animated grid control (`gridanim.tcproj`, same keyframes): the fitted scale agrees between preview and export at the keyframes (1.000 and 1.400 at frames 30 and 150) and at frame 90 (1.2000 preview, 1.1987 export). The export fit has a residual of 1.1–1.7 px against 0.02–0.05 px in the preview; offsets could not be compared because the periodic grid makes the absolute offset ambiguous. Unresolved: whether this residual is a real rounding effect of the export's animated scale/overlay path or a limit of the fit.

**Consequences**
- The Step 1 pass on the original fixture does **not** generalise. G52 stays OPEN; the Product Owner decision in §1.7 is moot until this is resolved.
- Step 2 (real residual offset on animated transforms) is now required: check the export's animated `scale@sc_N` and `overlay` rounding against the preview's CSS transform at mid-easing, using a non-periodic registration target (a grid with a unique marker) and the fitted offset in canvas pixels.
- Not done: Reset, undo/redo and save/reopen on this fixture (they were qualified on the original fixture on 2026-11-12), frame 30 recapture.

---

## 11. Step 2 — real offset found and fixed (2026-11-14)

**Cause (export).** In 4:2:0, FFmpeg's `overlay` floors x and y to even pixels (chroma alignment). Every video layer placed at an odd pixel position was drawn 1 px up and left of where the preview draws it, on the static path and on the keyframe-automation path. Evidence: an exported frame at scale 1.0 and position 0 was shifted (−1, −1) against its source; the automation schedule itself was exact (frame 90: 2304×1296, x −483), and `rotate` placed the frame exactly (measured with nearest-neighbour scaling for sizes 640–3086); only `overlay` moved it. `format=yuv444` on that overlay keeps the requested position (0 shift at an odd offset against −1 without it).

**Fix.** `overlay` of video layers composites in `format=yuv444` (`composition/mod.rs`); the encoder converts to 4:2:0 as before. Regression test `tests/phase60_overlay_position.rs` (static, keyframed, even control): passes with the fix; without it the odd-position tests measure (146.00, 64.03) against (147.00, 65.00). `export_automation_poc.rs` had baked the floored row (134) into an assertion and used detection thresholds tuned for exact black; its string expectations and thresholds were updated (green level 4 for a nearly dissolved clip, a 16-level bound for "no double render"), the 8 tests pass. Every other lib and integration suite passes (33 suites). Not done: the static rotate path (`rotw`/`roth` output boxes) and the transition overlays may have the same flooring; not touched.

**Effect.** Animated grid (`gridanim`), SSIM at DPR 2: 0.764 / 0.858 / 0.833 → **0.963 / 0.980 / 0.958** (frames 30 / 90 / 150), and the fitted-shift diagnostic collapsed from (0.75, 1.0) px to (0, 0–0.25).

Fixture 2 (natural footage), same method as Step 1, recaptured with every frame settled:

| Frame | 30 | 90 | 150 | 209 | 210 |
|---|---|---|---|---|---|
| Before the fix (DPR 1 / 2) | invalid | 0.584 / 0.761 | 0.815 / 0.727 | 0.838 / 0.767 | 0.950 / 0.995 |
| **After, DPR 1** | 0.9158 | 0.8002 | 0.9415 | 0.9655 | 0.9498 |
| **After, DPR 2** | 0.9043 | 0.9668 | 0.9286 | 0.9547 | 0.9946 |

**Verdict under the fixed rule: still FAIL** at frame 30 (0.916 / 0.904, 0.004–0.016 short) and at frame 90 at DPR 1 (0.800; DPR 2 passes at 0.967, and the neighbours 89 / 91 score much lower, so this point is sensitive to which video frame the browser shows). Frames 150, 209 and 210 pass at both DPRs.

**What remains is not export geometry.** At frame 30 the lower part of the canvas shows only clip A at scale 1.0 and position 0. Its export matches the source frame at SSIM 0.9936 (so export fidelity is fine), while the preview scores only 0.87–0.90 against the source frame itself, whichever filter shrinks the reference (bicubic or box) and at both DPRs. The preview plays the full-resolution file (no proxy). The gap is therefore the browser's own video downscale of detail-rich footage, which no export change can close. This is the "browser video scaling" cause recorded on 2026-11-12, now isolated on footage where the export is exact.

**Consequences.** G52 stays OPEN. What is left to decide: (a) Step 4, a parity frame rendered by the export compiler when the playhead is paused (an honest paused-frame claim; playback stays a browser approximation), or (b) a Product Owner decision on the claim and the measurement. The corrected measurement of Step 1 plus this export fix is worth keeping either way. Not rerun: Reset, undo/redo and save/reopen on this fixture; the installed build and an NSIS rebuild (the export change must be included in the next build).

### 11.1 Follow-up findings (2026-11-14, after commit 0e4c6a4)

- **Transition branches had the same flooring.** `overlay@ov_tr_l/r` also floored odd positions; `p60o_transition_branches_keep_an_odd_position_too` measures (146.03, 64.15) against (147.00, 65.00) without the 4:4:4 option and passes with it. **Uncommitted** (see below). Text-overlay ROI positions (`overlay=x=ROI…:format=auto`) may floor as well; not measured.
- **Side effect to resolve before shipping the 4:4:4 overlay.** `test_poc_v4_existing_crossdissolve_coexistence` (two inputs, sendcmd on a clip that joins an overlay) now fails intermittently: Clip B sits at x=93 instead of 90 at frame 135, i.e. a command from three frames later was applied early. Measured: 0 failures in 40 runs before the change, about 4 in 50 with it. A moving layer in a single-input export is stable (8 runs, same pixel). Likely mechanism: `sendcmd` fires when a frame passes its own input chain, while the downstream `overlay` may still be consuming earlier frames of the other input; a slower overlay widens that window. The race pre-dates the change; the 4:4:4 conversion makes it visible. Options: keep a 4:4:4 base through the whole graph so no per-overlay conversion is needed, drive overlay x/y from frame-time expressions instead of `sendcmd`, or revert the 4:4:4 overlay and only round positions to even.

**Resolution (same day).** The overlay position commands now fire on the overlay's *main* input (`LayerVideoAutomation::overlay_command_file`), which advances in lockstep with the overlay; scale/rotate/opacity commands stay on the layer chain. `test_poc_v4_existing_crossdissolve_coexistence`, instrumented the same way, passes 40 of 40 runs (the 4:4:4 overlay without this change failed about 4 in 50, and a 4:4:4 base for the whole graph failed 8 in 40, so it was dropped). Transition branches also composite in 4:4:4 (`p60o_transition_branches_keep_an_odd_position_too`). All lib and integration suites pass (33).
