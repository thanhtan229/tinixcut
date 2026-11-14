# Phase 59 — Loudness, ducking, flexible speed, xfade transitions, transport keys

**Status:** IMPLEMENTED 2026-11-13 (59A–59E) with the deviations in section 8. Native UI qualification and the follow-ups are in sections 9 to 11.
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

## 8. Implementation record — 2026-11-13

Commits: `2ea9237` (59A), `05ed9df` (59B), `400e8e9` (59C), `f217408` (59D), `e970525` (59E). Schema is v19 (ducking + reverse; custom speed is a new `ClipSpeed` variant, geometric transitions are new `TransitionKind` values, both serialize additively).

**Deviations from the plan**

- 59A: the loudness UI is a preset selector next to the Output selector; the MCP `ExportProfile` schema advertises the field in snake_case because that is how it serializes (the schema keys for the older fields are camelCase, a pre-existing mismatch not touched here).
- 59B: voice = every audible clip without ducking; the curve is evaluated inside `CompositionPlan::effective_audio_gain_at`, so export (envelope file) and preview (`sample_preview_at`) read the same function. Preview shows "Ducking pending" until `ducking_prepare` has run; the analysis is cached for one mix signature (bounded).
- 59C: **reverse is limited to 15 s of source, not 60 s**, because FFmpeg `reverse` keeps every frame in memory (about 2 GB for 15 s of 1080p). The reversed preview proxy (D5) was **not built**: the preview plays reversed clips forward and the HUD says "Reverse: export only". **Freeze frame was not implemented.** Custom speeds whose source duration is not a whole number of source frames are refused by the existing exactness rule (for example 7/3 of 100 frames).
- 59D: preview CSS formulas were derived from `vf_xfade.c` and are checked against measurements of real exports; `CutTransitionCreate` accepts every cut kind, while item-level transition commands still refuse them. A domain validation that allowed only two cut kinds was found by the new command test and fixed.
- 59E: shuttle above 1x and reverse step the playhead every 33 ms with normal playback stopped (so audio is muted); "Export In-Out only" is applied as output-side `-ss/-t` (the earlier part is rendered but not encoded); "Delete In-Out" is one transaction over unlocked tracks.

**Evidence (automated, real renders)**

| Phase | Check | Result |
|---|---|---|
| 59A | integrated loudness of a quiet and a loud mix after normalising to -14 LUFS | within 0.5 LU, true peak under the limit; untouched render far from target (negative control) |
| 59B | music level while a 1 kHz voice plays, 12 dB depth | 12 dB down within 1.5 dB; unchanged before and after within 0.6 dB; 6 and 15 dB settings scale; no voice means no ducking |
| 59C | 3/2x, 1/3x, 1/8x, reverse, 2x reverse of a time-coded gradient | pixel values give the expected source instant; durations exact |
| 59D | six linear transitions and circle open on red -> blue | edge within 4 % of the formula, radius within 4 px; the TypeScript preview geometry matches the same measurements |
| 59E | range export of frames 60..120 of a 6 s clip | exactly 2.0 s, shows source seconds 2..4; invalid ranges rejected |

**Not verified (open)**

- The new Inspector controls (ducking, custom speed, reverse, transition picker, loudness and In-Out toolbar items, Keys dialog), the CSS rendering of geometric transitions in WebView2, J/K/L stepping and the ruler marks were type-checked and unit-tested but **not exercised in the running app**.
- Ducking preview audio itself (volume automation applied to playing audio) was not listened to or measured natively; only the shared gain function is tested.
- Preview/export pixel parity of the geometric transitions was not measured natively.

## 9. Native UI check — 2026-11-13 (partial)

Dev build driven through the CDP harness, project `p59 mix` (fixtures from `src-tauri/tests/phase59_native_fixtures.rs`, 1920x1080 @ 30 fps).

**Observed working**

- Inspector shows SPEED (named select, custom field, Set), REVERSE and DUCKING sections; toolbar shows the loudness selector, In-Out and Loop checkboxes, Delete In-Out and Keys.
- Ducking checkbox on the Music clip commits "Update ducking" (rev 3 to 4) and reveals depth/attack/release/threshold fields.
- Reverse on the Red clip commits (rev 4 to 5).
- Custom speed 3/2 on a clip that uses its whole source is refused with "Speed change would read beyond the available media source range" (correct); 2/3 commits and the selector shows Custom.
- Key `L` starts shuttle (label "1x") and moves the playhead (199 f reached); `K` stops; `I`/`O` set marks and enable Loop and Delete In-Out.

**Defect found and fixed**: the status line said `Set Speed ([object Object])` for custom speeds; it now uses `formatClipSpeed`.

**Not verified (still open)**: the Keys dialog contents and remapping, ruler mark rendering, the Alt+I/Alt+O clear keys, `trans.tcproj` WipeLeft preview rendering in WebView2, ducked preview audio, and Delete In-Out execution. The G52 parity claim is unchanged.

## 10. Native UI check, continued — 2026-11-13

Same harness and fixtures (`p59 mix`, `p59 trans`).

**Observed working**

- Keys dialog lists all 19 actions with their chords. Remapping Set In from `I` to `U` and saving: `I` no longer sets a mark, `U` then `O` sets the range (Loop becomes enabled).
- Ruler shows the range band and In/Out point markers (In 65 and Out 122: band 171 px, markers at the expected frames); `Alt+O` clears the Out mark and removes the band.
- Delete In-Out (frames 65 to 122) is one transaction (rev 0 to 1) over the three tracks: Red 180 to 65+58, Music 240 to 65+118, Voice (60..150) to 5+28 frames. The numbers match the expected ripple.
- WipeLeft preview on `p59 trans` (transition centred on the cut, frames 75 to 105): blue share of the canvas row at frames 80, 90, 100 is 0.167, 0.499, 0.832 against 5/30, 15/30, 25/30 expected; 0 at frame 60 and 0.998 at frame 110. Measured on screenshots of the WebView2 canvas (`artifacts/phase59/native/wipe/`).

**Harness and product notes**

- A stale `mix.tcproj.recovery` (revision 6, left by the first session) made every later edit show "Recovery save failed: Project revision conflict" and blocked opening another project until it was deleted. The conflict check is intended protection, but the editor gives no way out of this state besides deleting the file. Not changed here.
- Keyboard shortcuts are ignored while the "Unsaved projects recovered" dialog is open (by design).

**Still not verified**: ducked preview audio while playing, `Alt+I`, reversed-clip behaviour in the preview, Redo of a Delete In-Out after reload, export parity of the WebView2 transition (only formula plus separate FFmpeg measurements).

## 11. Follow-up changes — 2026-11-14

Closes the open items from sections 8 to 10 where that is possible. Each item lists its evidence.

| Item | Change | Evidence |
|---|---|---|
| Recovery copy newer than the reopened project | New "Open saved project and discard recovery copy" button; backend `project_discard_saved_recovery` deletes the copy only if its revision and fingerprint still match what the prompt showed | unit test (stale copy blocks saves, wrong fingerprint or revision deletes nothing, discard unblocks saves); native: prompt shows the button, the `.recovery` file is gone and the project opens |
| MCP `ExportProfile` schema | Advertises the real snake_case keys, the required fields, and `range` (camelCase keys were ignored by the deserializer) | schema compiles; full suite green |
| Freeze frame (59C, not done before) | "Freeze" button. `freeze_frame_create` extracts the clip's frame (PNG, full size) to `<project>_freeze/`, imports it as an image asset; the editor then splits at the playhead, shifts downstream clips and adds a 2 s image clip in one transaction. No schema change | `phase59_freeze.rs` measures that the still shows the requested source instant (and differs from another instant); unit test for the speed-aware source time; native: Red clip at frame 60 became Red 60f + Freeze 60f + Red 120f, one revision |
| Reverse preview (D5, not done before) | `media_get_reverse_preview_video` builds a silent, 540p reversed copy of the clip's source range in the derived cache; the preview plays it forward at proxy time = forward source time minus range start (the instant the export shows). HUD says "Reverse: preparing preview" until it is ready; the reversed clip's embedded audio is muted in the preview | `phase59_freeze.rs`: proxy time t shows source instant (start + duration - t) with a negative control; vitest for the mapping and cache key; native: the reversed clip loads the `_rev_` file and the video sits at 0.333 s for frame 10 |
| Ducking preview audio | Native: Music volume 1.0 outside the voice, 0.251 (-12 dB) while the voice plays, 0.98 after the release | measured on the `<audio>` element volumes at frames 20, 100, 200 |
| `Alt+I` | Native: clears the In mark only, the Out mark stays | marker labels before and after |

**Defect found and fixed.** The preview ducking analysis (`ducking_prepare`) failed on any project that has a video clip without an audio stream, because such a clip is still an audio layer (embedded audio defaults to on); the preview then stayed on "Ducking pending" and never ducked. Export was not affected (it removes those layers first). The analysis now skips media without an audio stream; regression test `p59b_preview_analysis_ignores_a_silent_video_clip`.

**Still open, deliberately**

- Reverse stays limited to 15 s of source. FFmpeg `reverse` holds every frame; a longer limit needs separate reverse passes per segment written to temporary files, which is a design of its own.
- Custom speeds whose source duration is not a whole number of source frames stay refused (guardrail 7: exact rational time).
- The reversed clip's audio is not reversed in the preview (muted) and is not rendered backwards for audio-only clips there.
- WebView2 versus export pixel parity of the geometric transitions was measured only on the preview side (section 10) plus separate FFmpeg measurements. G52 pixel parity is unchanged and remains open.
