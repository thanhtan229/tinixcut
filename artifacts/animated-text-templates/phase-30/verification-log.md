# Phase 30 — Verification Log (Rich-Text Rendering & Parity)

**Date:** 2026-10-25  
**Renderer version:** `TEXT_OVERLAY_RENDERER_VERSION = 1`  
**Status:** IMPLEMENTED WITH DEFERRED NATIVE QUALIFICATION

---

## 1. Commands executed

| Command | Exit | Result |
| :--- | :---: | :--- |
| `cargo fmt -- --check` | 0 | clean |
| `cargo build` | 0 | native build clean |
| `cargo test --test phase30_rich_text_rendering` | 0 | 10 passed, 1 ignored (evidence writer) |
| `cargo test --lib -- --skip test_gate_f_poc5_matrix_execution` | 0 | 162 passed, 1 filtered (known POC5 harness) |
| `cargo test --test phase29_rich_text_domain` | 0 | 13 passed |
| `cargo clippy --lib --tests` | 0 | no new library findings; only pre-existing warnings plus test-style lints |
| `pnpm run typecheck` | 0 | clean |
| `pnpm test` | 0 | 44 passed (4 files) |
| `pnpm run build` | 0 | production web build |
| `TINIX_PHASE30_EVIDENCE_DIR=... cargo test ... p30_write_evidence_artifacts -- --ignored` (opt-level 3) | 0 | regenerated this evidence bundle |

The known non-terminating `test_gate_f_poc5_matrix_execution` harness is still excluded per the
project baseline policy; it is not treated as PASS.

---

## 2. Acceptance evidence

### P30-AC-01 — Typography, layout, Unicode

- `line_height`, `letter_spacing`, alignment, `Wrap`, `AutoFit`, spans and per-span fill/font-size
  all change the rasterized ROI.
- Vietnamese NFC and NFD inputs (`Tiếng Việt`) produce **byte-identical** ROI PNGs after NFC
  normalization and UAX #29 segmentation.
- CJK input against the bundled face returns `TextOverlayError::MissingGlyphs`, never `.notdef`
  boxes.

### P30-AC-02 — Font lifecycle

- No `FontReference` → bundled Roboto-Bold, SHA-256
  `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334` (matches Phase 27 fixture).
- `asset_id` resolves an existing project asset and records `FontOrigin::Asset(id)`.
- Deleted asset file → `MissingFont`; non-font bytes → `InvalidFont`; unknown family → `MissingFont`.
- Export with an unbundled family fails before FFmpeg, so a final render never silently substitutes.

### P30-AC-03 / P30-AC-04 — Shared renderer parity

- `preview_overlay_at` (preview IPC seam) and `render_overlay_for_layer` (export seam) return the
  same ROI and identical PNG bytes for the same canonical layer; `parity-preview.png` and
  `parity-export.png` are equal.
- Rich items produce `overlay=x=…:y=…` filtergraph entries and **no** `drawtext`.
- Legacy text items without `text_rich` still produce the legacy `drawtext` filter unchanged.

### P30-AC-04 / P30-AC-05 — Style primitives and alpha

- Linear gradient, radial gradient, ordered multi-stroke, multi-shadow with separable blur,
  background box and per-span fill render deterministically (repeated renders are byte-identical).
- Straight-alpha composition over white produces no dark halo (`alpha-white.png`), and the same
  overlay over black and a color background is captured as `alpha-black.png` / `alpha-color.png`.
- Scale/rotation placement is baked into the overlay and stays inside the canonical canvas.

### P30-AC-06 — Cache, failure and cleanup

- Bounded mask cache (34 entries / 404,538 bytes in the benchmark run, ceiling 256 MiB) is
  versioned by font hash, glyph, size, stroke/shadow geometry and renderer version.
- `clear_cache()` rebuilds byte-identical output; a thicker stroke changes the ROI, proving stale
  prepared masks are not reused.
- Export materializes overlay PNGs under a disposable directory, removes that directory and the
  automation scratch directory on every exit path, and a real FFmpeg export (`export-smoke.mp4`)
  contains visible text pixels.

### P30-AC-07 — Performance

Measured on the development host with the documented optimized test profile
(`CARGO_PROFILE_TEST_OPT_LEVEL=3`):

| Measurement | p95 | Budget | Result |
| :--- | ---: | ---: | :--- |
| W1 buffer render (120 graphemes, gradient, 2 strokes, 2 shadows, wrapped 1600 px) | 12.61 ms | ≤ 16.0 ms | PASS |
| W1 preview packet (render + ROI PNG) same workload | 23.00 ms | ≤ 16.0 ms | **LIMIT** |
| Canonical one-line CapCut-style packet | 7.49 ms | ≤ 16.0 ms | PASS |
| W2 four-title packet | 19.83 ms | ≤ 33.3 ms | PASS |

The packet limit is PNG transport for an oversized multi-line ROI. Preview caches one overlay per
item per project revision (not per frame), so playback does not pay this cost repeatedly. This is
recorded as a documented limitation, not hidden.

### P30-AC-08 — Regression and deferred qualification

- Legacy text/captions/commands/composition suites remain green; typecheck, frontend tests, web
  build, native build and formatting pass.
- Native interactive GUI observation and installed-app resource lookup were not observable in this
  session; they remain under the Phase 26 deferred native qualification, as with Phases 16–29.

---

## 3. Known limitations

1. Glow has no canonical field in the Phase 29 domain, so Phase 30 does not render it.
2. Only Roboto-Bold is bundled; other weights/faces require a font asset and are reported as
   approximated, never silently substituted.
3. Center/Inside stroke alignment is drawn as half-width outside dilation with the fill covering
   the inner half (Outside is exact).
4. Text animation and animated transforms are Phase 31.
5. Oversized multi-line overlay PNG transport can exceed the preview IPC p95 budget on first render.
6. Native GUI/installed-app qualification remains deferred.

---

## 4. Evidence files

- `evidence-summary.json`
- `benchmark-production.json`
- `export-metrics.json`
- `export-smoke.mp4`
- `sample-rich-overlay.png`
- `alpha-white.png`, `alpha-black.png`, `alpha-color.png`
- `parity-preview.png`, `parity-export.png`


---

## 5. Independent review remediation (2026-10-25)

Code Reviewer returned NOT LGTM on the first Phase 30 snapshot. All blocking findings were
reproduced, fixed, and covered by regression tests before re-review:

1. **Descent/bleed clipping (HIGH).** ScaleFont::descent() is negative; block/bottom bounds now
   add the descender magnitude. Mask extents (raw, dilated strokes, offset+blurred shadows,
   background padding) are measured before buffer allocation and the symmetric bleed is derived
   from them. Regression: p30_review_descent_and_bleed_do_not_clip_diacritics_or_shadows
   asserts Vietnamese descenders/underdots are retained and the outer buffer border is transparent.
2. **Mask-cache collision/loss (HIGH).** The geometry signature now includes stroke/shadow vector
   lengths with an avalanche mix; production preview/export share one renderer per
   font+canvas identity so warm masks survive across calls. Regression:
   p30_review_cache_signature_never_aliases_strokes_and_shadows.
3. **Silent per-span font substitution (HIGH).** A span requesting another face/asset now fails
   with UnsupportedPerSpanFont in render and export. Regression:
   p30_review_per_span_font_fails_explicitly.
4. **Preview cache poisoning (MEDIUM-HIGH).** The WebView now requests the sampled timestamp
   frame, never caches a null (inactive) response, ignores async results from superseded
   revisions, and applies sampled opacity as a CSS property so a fade frame cannot poison the
   revision cache.
5. **Non-blocking fixes:** FFmpeg rich-text overlay uses ormat=auto; export scratch dirs are
   unique per run (	c_auto_<pid>_<uuid>); std::io::Error::other replaces the deprecated
   constructor.

Evidence regenerated after the fixes: export-metrics.json (overlay_roi [600, 506, 1328, 599],
8,061 bytes), enchmark-production.json (W1 buffer p95 12.61 ms, W2 p95 19.83 ms, canonical
packet p95 7.49 ms).
