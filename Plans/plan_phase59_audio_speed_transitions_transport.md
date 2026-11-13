# Phase 59 — Loudness, ducking, flexible speed, xfade transitions, transport keys

**Status:** PLANNED — not implemented. Drafted 2026-11-13.
**Basis:** second review of the reference repositories (Clypra, blyacut, OpenCut-AI, Mono Cut), recorded in the session of 2026-11-13. Only gaps confirmed in the TinixCut source are included.

## 1. Objective

Add the five improvements with the best value-to-risk ratio. Each must:

- keep every project mutation in the Command Service;
- keep FFmpeg behind `composition` / `media`;
- give preview and export the same semantics, or say explicitly where preview differs;
- carry a measured acceptance check plus a negative control, as in Phase 58.

## 2. Confirmed gaps (source evidence)

| Gap | TinixCut today | Reference |
|---|---|---|
| Loudness target on export | No `loudnorm` / `ebur128` anywhere in `src-tauri/src` | Presets in OpenCut-AI `lib/audio/loudness-types.ts`: YouTube/Spotify −14, Apple −16, EBU R128 −23, ATSC −24 LUFS |
| Automatic ducking | Only a static volume draft (`src/components/audioMixPlan.ts`, `duckOperations`) | blyacut `crates/engine/src/lib.rs` (~931–965): voice bus keys `sidechaincompress`, then `alimiter` |
| Speed | `ClipSpeed` = `Half` / `Normal` / `Double` (`domain/mod.rs:683`, 29 uses) | blyacut `video_timing()` (arbitrary speed, `reverse`); Mono Cut `docs/retiming.md` |
| Transitions | `TransitionKind` = FadeIn, FadeOut, CrossDissolve, FadeBlack, DipToBlack | FFmpeg `xfade`; Clypra wipe/zoom shaders |
| Transport | No J/K/L, no In/Out, no shortcut remapping | Mono Cut (`docs/features.md`, `docs/monitor-keyboard.md`) |

## 3. Key design decisions (recommended defaults)

| # | Decision | Default |
|---|---|---|
| D1 | Where the loudness target lives | Output property, an optional `loudness` field on `ExportProfile` (additive, serde default = off). Project data is unchanged. |
| D2 | Loudness method | Two-pass. Render as today, measure with `ebur128` (integrated + true peak), then re-encode audio only with `loudnorm` in linear mode using the measured values; video is stream-copied. This runs on the temp output before the atomic rename. |
| D3 | Ducking model | A **derived gain envelope**, not a live compressor. Voice activity is analysed from the "voice" clips (RMS in 50 ms windows → threshold → attack/release smoothing). The result is a per-sample gain applied to the ducked clips through the existing `LayerAudioAutomation` envelope path. Preview applies the same envelope. This keeps preview/export parity possible; a live `sidechaincompress` in export only could not be matched in preview. |
| D4 | Speed representation | `ClipSpeed` gains `Custom { numerator, denominator }` (rational, 1/8×..8×), plus a per-clip `reverse: bool` and a separate freeze-frame clip kind. The three existing variants remain and migrate unchanged. Schema v19. |
| D5 | Reverse playback in preview | HTML video cannot play backwards. A reversed proxy file is generated into the derived cache (disposable, keyed by source fingerprint + source range) and played forward. Export uses `reverse` / `areverse` on the clip's own trimmed range, bounded to 60 s per clip; longer clips are rejected explicitly. |
| D6 | Transitions | Add geometric kinds only: `Wipe{Left,Right,Up,Down}`, `Slide{Left,Right}`, `CircleOpen`. Export uses `xfade`. Preview uses CSS `clip-path` / `transform`, which are exact geometric functions of progress. Stylised shader transitions (glitch, zoom-blur) are out of scope. Bundled into schema v19 with D4. |
| D7 | In/Out | Transient UI state, not persisted. Used for loop playback, "Export In–Out range" (an optional `range` on the export request, additive) and "Ripple delete In–Out" (through the existing commands). |
| D8 | Shortcut remapping | Stored in app settings (`shortcuts`, serde default = built-in map), not in the project. Conflicts are rejected in the editor UI. |

## 4. Phases (in order)

### 59A — Export loudness target (~1–2 days)

1. `ExportProfile.loudness: Option<LoudnessTarget { integrated_lufs, true_peak_db }>`, with validation (−36..−5 LUFS, −9..0 dBTP).
   - Presets in the UI: YouTube/Spotify −14/−1, Apple Podcasts −16/−1, EBU R128 −23/−1, ATSC A/85 −24/−2, plus Custom.
2. `media::loudness`:
   - `measure(path) -> { integrated, true_peak, lra }` using `ebur128=peak=true` and parsing the summary.
   - `normalize(temp_output, target)`: two-pass `loudnorm=I=..:TP=..:LRA=..:linear=true:measured_I=..` and so on, with `-c:v copy`.
3. Export flow:
   - If measured loudness is already within ±0.3 LU, skip the second pass.
   - Silence (integrated −70 LUFS or no audio) is reported and left untouched; never amplify silence.
   - Cancellation and temp-file cleanup follow the existing export job rules.
4. Expose through MCP `tinixcut_export_start` (optional field) and the Export UI.
5. **Tests:**
   - Real renders of a quiet and a loud fixture each land within **±0.5 LU** of the target, true peak ≤ limit + 0.2 dB.
   - Off = bit-identical audio to today.
   - Silence is not amplified.
   - **Negative control:** asserting a −14 target against the unnormalized render fails.
   - Measurement must agree with an independent `ebur128` run on the final file.

### 59B — Ducking from voice activity (~3 days)

1. Domain: per audio/video clip `ducking: Option<Ducking { depth_db (1..30), attack_ms (5..500), release_ms (20..2000), threshold_db (−60..−10) }>`. The clip is the one that gets quieter; "voice" means every other audible clip not marked as ducked.
2. `media::ducking` (pure, unit-tested):
   - RMS windows → binary voice activity → attack/release one-pole smoothing → gain curve (dB → linear).
   - Cached in the derived cache, keyed by source fingerprints, ranges and parameters.
3. **Export:** merge the ducking gain with the clip's existing volume and fade envelope into the same `LayerAudioAutomation` envelope file. No new FFmpeg graph shape.
4. **Preview:** apply the same gain curve to the clip's audio element (GainNode automation, or `volume` per animation frame), sampled from the cached curve. If the curve is still computing, show an explicit "Ducking pending" state; never play unducked audio as if it were ducked.
5. **Tests:**
   - Synthetic voice bursts (tone) over music (noise). During voice, music RMS drops by depth ±1 dB; outside voice it stays within ±0.5 dB of the undocked level.
   - Attack and release times are within ±1 window.
   - Preview envelope vs export envelope: max difference ≤ 1 dB at 50 ms RMS (G51 tolerance).
   - **Negative control:** depth 0 equals no ducking.

### 59C — Custom speed, reverse, freeze frame (~4–5 days, schema v19)

1. Domain:
   - `ClipSpeed::Custom { numerator, denominator }` (reduced fraction, 1/8..8).
   - `reverse: bool` on video/audio clips.
   - Freeze frame as an image-like clip that references a source frame (`asset_id` + `source_frame`), rendered from the source and not from a cache file.
   - Migration v18 → v19 keeps existing enum values unchanged.
2. **Canonical time:** source duration = timeline duration × speed, computed in rational arithmetic (guardrail 7). Rounding policy documented and tested, as in Mono Cut `retiming.md`: timeline frame boundaries are exact, source in-point is preserved.
3. **Export:**
   - Video uses `setpts=PTS/speed`.
   - Audio uses chained `atempo` stages, each in 0.5..2.0 (up to three for 1/8..8). Pitch is preserved, as `preservesPitch` already does in preview.
   - Reverse uses `reverse` / `areverse` after `trim`, limited to 60 s per clip; longer clips are rejected in validation with a clear message.
   - Freeze uses `trim` of one frame + `loop` / `tpad`.
4. **Preview:**
   - Custom speed via `playbackRate`. HTML video supports 0.0625..16 in Chromium, but audio is muted outside 0.25..4 by the browser, so the UI states this.
   - Reverse via the reversed proxy (D5). Freeze via a still frame extracted into the derived cache.
5. **Commands:** extend `ClipSpeedSet` validation; add `ClipReverseSet` and `FreezeFrameInsert` (split at playhead + insert a still of N frames). Each with undo/redo, MCP/CLI and capabilities.
6. **Tests:**
   - Rational round-trips, plus overlap rejection when lengthening.
   - Export duration exact to the frame, and audio duration within one audio frame.
   - Reverse output equals the time-mirrored decoded source frames (frame-hash check).
   - Freeze output frames are identical.
   - v18 → v19 migration.
   - Preview: seek mapping for custom speed matches `sample_preview_at`.
   - **Negative control:** a 2× clip compared with the 1× expectation fails.

### 59D — Geometric `xfade` transitions (~3 days, same schema v19)

1. `TransitionKind` gains `WipeLeft`, `WipeRight`, `WipeUp`, `WipeDown`, `SlideLeft`, `SlideRight`, `CircleOpen`. Validation is the same as CrossDissolve (handles, duration).
2. **Export:** in the existing transition-group branch, replace the alpha blend with `xfade=transition=<name>:duration=d:offset=o` on the two normalized branches (same size, fps and format). Map our names to FFmpeg's (`wipeleft`, `slideleft`, `circleopen`, ...).
3. **Preview:**
   - `PreviewTransitionGroup` renders the incoming layer with CSS `clip-path: inset(...)` (wipes) or `circle(r at 50% 50%)` (circle).
   - Slides use `transform: translateX(...)` of both layers.
   - Progress comes from the existing group progress. The formulas mirror FFmpeg `vf_xfade.c` for these kinds (read the source as in 58C).
4. **Tests:**
   - Export at 25 / 50 / 75 % progress: the edge position of the incoming region is within 1 % of width (wipes, slides) or of the radius (circle) of the formula.
   - Native preview check with `capture_preview.py` at the same points, using the centroid / edge metric (not SSIM, given G52).
   - **Negative control:** WipeLeft measured against the WipeRight formula fails.

### 59E — J/K/L shuttle, In/Out, shortcut remapping (~2–3 days, frontend + settings)

1. **J/K/L:**
   - K pauses.
   - L plays forward; repeated L steps 1× → 2× → 4×.
   - J shuttles backward by seeking at 1× / 2× / 4× steps with audio muted. This is the Mono Cut approach, because HTML video cannot play backwards.
   - K+J / K+L step one frame.
2. **In/Out:**
   - I / O set marks at the playhead; Alt+I / Alt+O clear them. Marks are shown on the ruler.
   - Loop playback between them.
   - "Export In–Out range" passes an optional `range` to export; the backend trims the composition to that range, with a validated, additive request field.
   - "Ripple delete In–Out" builds split + ripple-delete operations through the Command Service in one transaction.
3. **Remapping:**
   - `AppSettings.shortcuts: Map<ActionId, KeyChord>`, with serde default = built-in map.
   - A settings dialog lists actions, captures a chord and rejects conflicts and reserved OS keys.
   - A pure `resolveShortcut(event, map)` is used by every global handler.
4. **Tests:**
   - Pure keymap resolution and conflict detection.
   - Shuttle rate state machine.
   - In/Out range validation and the ripple-delete operation plan, as a Command Service dry-run.
   - Export with range has exact duration.
   - Native: J/K/L and I/O through UIA key input when TinixCut owns the foreground.
   - Read `UX_UI.md` before the dialog work.

## 5. Explicitly out of scope

- A live sidechain compressor in export.
- Speed ramps (keyframed speed, Clypra `speed_ramp.rs`). They need a time-warp model; consider after 59C proves rational speed.
- Shader transitions (glitch, zoom-blur, film burn).
- Auto-reframe.
- Release packages / shorts generation.
- Effect recipes.
- Agent-eval suite (candidate Phase 60).
- MKV/ProRes intermediate export (small; can be added any time as an export preset).

## 6. Cross-cutting rules

- Schema v19 is introduced once (59C + 59D) with one migration and its tests.
- **Regression per phase:**
  - `cargo test --lib` and the affected integration suites (render suites at least: phase14/18/26/51/58).
  - Frontend suite, typecheck, `cargo fmt`, release build.
  - G54 UI-response subset on a production bundle when preview code changes.
- Native checks use `scripts/qualification/native/` with the Phase 58 fixture approach: natural BT.709-tagged footage, a no-effect baseline, and a negative control.
- Docs:
  - `ARCHITECTURE.md` §17 for the audio envelope (ducking), speed/reverse and transition semantics.
  - `RELEASE_READINESS.md` only when a phase is accepted.
  - `PROJECT_STATUS.md` once per phase.

## 7. Suggested order and estimate

59A (1–2 d) → 59B (3 d) → 59C (4–5 d) → 59D (3 d) → 59E (2–3 d).

59A and 59B are export/audio only and measurable without the UI. 59C and 59D share the schema bump. 59E is independent of the others.
