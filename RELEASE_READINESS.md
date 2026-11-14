# RELEASE_READINESS.md

> Release scope decision and evidence summary for TinixCut v0.1.0 (Windows, local desktop).
> Recorded 2026-11-12 by the Product Owner's delegate under the Product Owner's instruction to finish the repository without further manual qualification. Decisions below narrow **claims**; they never turn missing evidence into PASS.

## 1. Gate status

| Gate | Status | Basis |
|---|---|---|
| G54 preview/seek, UI response, RAM, proxy, offline source | **PASS** | Budgets locked before measurement; 10 min/100 clip and 30 min/300 clip. `Plans/phase54_preview_performance.md`. |
| G56 saved-project ownership | **PASS** | Native stale-save conflict, Save a Copy, MCP parity. |
| G50 timeline editing | PASS for the keyboard/command path; pointer drag not qualified | 2026-11-12 keyboard workflow, locked-track message, undo/redo, save/reopen, export duration/decode. |
| G51 audio | PASS for export; live meter not qualified | Export audio inside locked tolerance (gain/fade, speed, mute, detach). |
| G52 visual canvas | PASS for geometry, animation, reset, history, persistence; **pixel parity not claimed** | Layer positions within 3.6 px; SSIM 0.900–0.932 vs locked 0.92 (3 of 5 points below). |
| G53 captions | PASS for SRT round trip, preview/export visibility, unit suites; other native caption flows not re-run | `Plans/phase53_caption_transcript_editor.md`. |
| G48/G49 | Automated suites pass; native flows not re-run | 43 settings/recovery/persistence tests; earlier native smoke runs. |
| G55 AI editorial usefulness | **Not claimed** | Needs independent reviewer and additional real briefs (external). |
| G57 installer | Build, install, installed render, uninstall pass; **upgrade not claimed** | No authentic earlier installer exists (only 0.1.0). |

## 2. Release claims (what v0.1.0 may say)

- Local editing, export, MCP/CLI automation, captions (SRT/VTT), preview with optional seek-optimized proxies.
- Preview vs export agree on layer **geometry, timing and animation**; they are **not** guaranteed pixel-identical (different resampling).
- Playback performance qualified on the two published workloads (10 min/100 clip, 30 min/300 clip, Windows 11 reference host). 4K-via-proxy, dropped-frame and GPU figures are not published.
- Audio export fidelity qualified; live level meter is indicative only.
- Post-v0.1.0 additions (Phase 58, `Plans/plan_phase58_reference_driven_improvements.md`): per-clip 3D LUT (`.cube`) and chroma key verified against FFmpeg in export; read-only video scopes, YouTube chapter list, audio-based alignment proposals, and MCP safety annotations. Their preview paths (WebGL effect layer, scopes panel) are only partly natively qualified (see plan section 9) and export is authoritative.

## 3. Explicit non-claims (deferred, not hidden)

1. **AI edit-planning quality (G55)** is experimental; the feature ships as a reviewable draft with provenance and must not be marketed as quality-assured.
2. **Upgrade from an earlier installer** is untested; v0.1.0 is the first release, so no upgrade path exists yet. Test it when 0.2.0 is cut.
3. **Pixel-level preview/export parity** is not claimed (decision 2026-11-12, approved by Product Owner: threshold 0.92 stays locked and unrelaxed; export scale-filter A/B showed no gain, so the residual gap is in the preview path; revisit with a preview-renderer change in a later release).
4. **Pointer-drag editing** was not qualified; keyboard and command paths are.
5. **Phase 58 preview parity** for LUT/chroma clips: measured natively on 2026-11-13 (shader math exact on the real GPU; preview vs export luma SSIM 0.945-0.975 on natural footage) but the strict rule of staying within 0.01 SSIM of the no-effect baseline is not met (WebGL canvas path costs ~0.03 SSIM). Pixel parity for effect clips is not claimed.
6. Physical calibrated A/V sync, clean-VM install and hardware encoders remain optional per the 2026-11-03 decision.

## 4. Automated test status (2026-11-12)

Frontend 184/184, typecheck, production build, `cargo fmt`. Rust: library 317 (the long-project scalability benchmark is timing-sensitive and can fail when run in parallel with other suites; it passes alone), and every integration suite passes when run individually, including `phase28_text_renderer_poc` under `CARGO_PROFILE_TEST_OPT_LEVEL=3` as documented. Four integration tests and one fixture had drifted from the current export contract (export size must match the canvas; a new `background_corner_radius` field) and were updated. Also run in the final pass: `phase16_research_poc` (12/12), `phase17_performance_benchmarks` (1 passed; its 5 benchmark cases are `#[ignore]` in source), and `phase35_agent_template_generation` (31/31 after fixing the agent validator, which rejected `backgroundCornerRadius` even though the domain supports it; the earlier "> 35 min" was the same failing run hanging on error paths). The release NSIS build was rebuilt and a silent install, installed render with full decode, and uninstall passed on this commit.

## 5. Reproducing the evidence

Native runs use `scripts/qualification/native/` (see its README); Rust and frontend suites run with `cargo test` and `pnpm test`. Evidence artifacts are under `artifacts/` (untracked by repository convention).

## 6. Phase 59 additions (2026-11-13)

Export loudness targets, per-clip ducking, custom speed (1/8x-8x) and reverse, geometric cut transitions, and In/Out range export are verified on real FFmpeg renders (`Plans/plan_phase59_audio_speed_transitions_transport.md` section 8). Their UI paths (Inspector controls, preview of geometric transitions, J/K/L shuttle, ruler marks, shortcut dialog) are only partly natively qualified (see plan section 9). Reverse plays forward in the preview and is limited to 15 s of source; freeze frame is not implemented.

## 7. Phase 59 follow-up (2026-11-14)

Freeze frame and a reversed preview copy now exist; recovery-copy discard, the MCP export schema, preview ducking with silent video clips, and `Alt+I` were fixed or checked natively (`Plans/plan_phase59_audio_speed_transitions_transport.md` section 11). Reverse stays limited to 15 s of source and its preview copy is silent. G52 pixel parity is still open.

## 8. Phase 59 follow-up 2 (2026-11-14)

Reverse now works up to 60 s of source (pieces of 15 s through scratch files); the editor fits a clip to the nearest exact length when a fractional speed needs it. These are verified by render and unit tests, not natively. G52 pixel parity is still open and needs a preview compositor project (`Plans/plan_phase59_audio_speed_transitions_transport.md` section 12).

## 9. G52 pixel parity (2026-11-14)

Export fixes: video layers at odd pixel positions were drawn 1 px up/left of the preview (4:2:0 `overlay` floors to even), and the overlay position commands could run frames ahead of the overlay; both are fixed and covered by `phase60_overlay_position`. New **Exact frame** toggle in the preview: while paused, the frame is rendered by the export compiler (SSIM 0.927-0.992 against the export at DPR 1 and 2 on a natural-footage fixture). Playback remains the browser's approximation (SSIM 0.80-0.97 on detail-rich footage), and frames inside cut transitions keep the live view, so **do not claim pixel parity for playback**. G52 disposition is a Product Owner decision (`Plans/plan_g52_preview_export_parity.md` section 12). The next NSIS build must include these export changes.

**G52 disposition (2026-11-14):** closed for paused frames with **Exact frame** on (locked method passed at 20 of 20 points on two fixtures; Reset, undo/redo, save/reopen verified); playback and frames inside transitions are not pixel-parity claims. Release notes must say so. See `Plans/phase52_visual_canvas_keyframes.md`.

**Open item found 2026-11-14:** exports of keyframed clips differ between runs (47-67 of 450 frames below SSIM 0.97 on a two-track fixture, also on the code before this work). Fix is in progress in a separate session; do not claim reproducible exports of animated clips until it passes the two-run comparison in `Plans/plan_g52_preview_export_parity.md` section 12.3. G52's claim for paused frames refers to the intended export output.
