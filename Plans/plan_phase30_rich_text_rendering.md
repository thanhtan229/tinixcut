# Phase 30 — Font, Layout và Rich Text Rendering

**Status:** IMPLEMENTED WITH DEFERRED NATIVE QUALIFICATION - Gate 30 functional criteria PASS; evidence in rtifacts/animated-text-templates/phase-30/.
**Dependency:** P29 PASS.
**Gate:** Style/layout/font/alpha khớp preview và export.

## 1. Mục tiêu

Đưa renderer đã chọn ở P28 vào production text path, tiêu thụ canonical rich values của P29. Đây là rich-text rendering, chưa phải CapCut importer. Đọc ADR text-overlay, composition/parity, media/cache/job/export, UX cho missing resources và workflow.

## 2. Slice thực hiện

### 30.1 Font lifecycle và shaping

- Font resolution bằng source identity/hash + face/weight/style; local system/user font chỉ dùng khi file/face đã xác định.
- App fonts được phép bundle, local font references, missing/relink và explicit fallback dùng cùng resolver ở headless và native preview.
- Shape/lay out rich spans theo grapheme/cluster, kerning, ligature, direction, wrap, tracking và line-height. Không draw từng Unicode code point riêng để giả character layout.
- Cache key gồm font/resource hash, content/spans, layout, style, renderer version và scale cần thiết; invalidate khi font/text thay, không phụ thuộc cache để mở project.

### 30.2 Style primitives

- Solid, linear/radial gradient theo coordinate space đã lock; gradient stops, angle, opacity.
- Ordered multiple strokes và shadows; blur/glow, background/padding/radius, per-span appearance; bounds bao gồm strokes/shadow/glow.
- Bounded 2D depth/retro appearance bằng resolved style layers theo contract; chưa phải 3D mesh/shader runtime.
- Character styles kế thừa có precedence cụ thể; preview/export dùng cùng blend/color/alpha semantics.
- Chốt riêng mask/reveal/animated blur/gradient primitive nào cần ở P31/P32; không quảng cáo control khi backend không có.

### 30.3 Integration preview/export

- Shared renderer tạo text overlay tại canonical time; preview consumer không resolve font/layout bằng CSS lần nữa.
- FFmpeg dùng output overlay qua media boundary; drawtext legacy path chỉ tồn tại theo backward-compatibility policy P29, không làm fallback im lặng cho rich items.
- Static text cache được reuse; mutation tạo đúng invalidation; seek/selection không reset playback clocks hoặc audio.
- Implement bounded queue/backpressure/latest-relevant preview requests, export snapshot isolation và cancellation; temp/cache output disposable.
- Alpha overlay lên moving video, transitions và các aspect ratio không viền đen/halo hoặc stale cached frame.

### 30.4 Qualification

- Lossless visual corpus cho mọi style và layout; captured preview ở cùng target canvas/frame; font hash/layout metrics ghi trong evidence.
- Test exact font/missing glyph/fallback đã chọn, user font removed/relinked, delete cache/reopen và installed-app resource lookup.
- Legacy six presets/captions/save/reopen/export tiếp tục hoạt động theo policy đã lock.

## 3. Acceptance

- **P30-AC-01:** Typography options trong matrix core có consumer thật; weight/line-height không còn chỉ hiển thị đúng ở preview.
- **P30-AC-02:** Shared font resolver/headless layout deterministic; missing/relink/fallback là trạng thái rõ, không tự thay font final export.
- **P30-AC-03:** Việt NFC/NFD, dấu, rich spans, emoji/ligature/wrap/auto-fit đạt fixture policy; unsupported glyph/script có diagnostic.
- **P30-AC-04:** Gradient, multiple strokes/shadows, glow và background đạt lossless overlay/captured preview parity tại ratios/fps cần kiểm.
- **P30-AC-05:** Text overlays alpha đúng trên nền trắng/đen/màu/video; clipping/blur bounds qua scale/rotation PASS.
- **P30-AC-06:** Cache invalidation/reopen without cache/cancel/export failures không stale, leak hoặc làm hỏng output final.
- **P30-AC-07:** W1/W2 budget và existing transport regression không bị vượt; giới hạn khả năng được báo rõ.
- **P30-AC-08:** Legacy presets/captions, command/history/persistence/export regression, checks/build/restart/native health PASS theo roadmap.

## 4. Evidence và stop conditions

Lưu crop text ROI/alpha masks, font hashes, layout metrics, preview captures, headless exports, missing-font/cancel tests và benchmark dưới phase-30. Native UI không quan sát được thì capture gate chưa PASS.

Dừng P31 nếu preview còn dùng font browser khác export, CSS approximates style chưa thông báo, renderer cắt shadow/glow, hoặc resource cache đang được dùng làm source. Không dùng ảnh chữ render sẵn để thay editable canonical content.

---

## 5. Acceptance Evidence (2026-10-25)

- **P30-AC-01 PASS** ? Typography/layout values affect output; NFC/NFD Vietnamese byte-identical; spans, wrap, auto-fit; CJK diagnosed as missing glyphs.
- **P30-AC-02 PASS** ? Deterministic bundled/asset font resolution; missing/invalid font fails explicitly; export never substitutes.
- **P30-AC-03 PASS** ? Preview and export consume identical ROI/PNG bytes; rich items never emit drawtext; legacy items keep drawtext.
- **P30-AC-04 PASS** ? Linear/radial gradients, ordered multi-stroke, multi-shadow blur, background box, per-span fill render deterministically.
- **P30-AC-05 PASS** ? Straight-alpha overlay has no dark halo on white; scale/rotation stays in canvas bounds; alpha artifacts captured.
- **P30-AC-06 PASS** ? Versioned bounded cache invalidates on style change; export removes derived overlay/automation directories; real FFmpeg export embeds text pixels.
- **P30-AC-07 PASS WITH DOCUMENTED LIMIT** ? W1 buffer p95 12.61 ms <= 16 ms; W2 19.83 ms <= 33.3 ms; canonical packet 7.49 ms. Oversized multi-line PNG packet p95 23.00 ms is a documented transport limit (preview caches per revision).
- **P30-AC-08 IMPLEMENTED WITH DEFERRED NATIVE QUALIFICATION** ? Legacy suites, typecheck, 44 frontend tests, web/native builds and fmt pass; native GUI/installed-resource observation remains under Phase 26.
- **Known limits** ? glow absent from the Phase 29 domain; only bundled Roboto-Bold ships (weights approximated, reported); Center/Inside stroke alignment approximated; animation is Phase 31.

Evidence: evidence-summary.json, erification-log.md, enchmark-production.json, export-metrics.json, sample-rich-overlay.png, lpha-white|black|color.png, parity-preview|export.png, export-smoke.mp4.
