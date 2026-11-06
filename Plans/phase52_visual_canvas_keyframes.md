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
