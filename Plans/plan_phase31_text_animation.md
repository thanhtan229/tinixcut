# Phase 31 — Text Animation và Editable Sample Gate

**Status:** PLANNED — chưa triển khai.
**Dependency:** P30 PASS.
**Gate T1:** Mẫu người dùng được dựng lại thành template Tinix chỉnh sửa được.

## 1. Mục tiêu

Animation in/loop/out theo toàn text, grapheme/ký tự, từ và dòng, dùng canonical evaluator thay vì CSS clock hoặc Lua runtime. Dựng lại sample là evidence đầu tiên của native capability, không gọi là automated CapCut import. Đọc canonical time/easing/lifecycle, P27 contract, P29 model, shared renderer ADR và resource evidence của sample.

## 2. Slice thực hiện

### 31.1 Evaluator và motion primitives

- Evaluate `RationalTime` tại frame grid; không lấy wall clock/WebView timer làm canonical animation time.
- Unit selection: all, grapheme, word, line; stable order, reverse/center-out, stagger và seeded order nếu có random. Shape đầy đủ trước, map grapheme→cluster/glyph; combining mark và ligature không bị tách sai.
- Position, scale, rotation, opacity, pivot và easing Linear/EaseIn/Out/InOut/CubicBezier dùng các foundations hiện có.
- In/loop/out là các đoạn có timing/order/composition rõ; loại loop repeat/ping-pong được type hóa, boundary deterministic. Preset bounce/elastic nếu giao phải thành bounded typed curve/keys, không JS expression.
- Reveal/typewriter/wipe dùng mask/visibility semantics renderer đã duyệt; animated blur/gradient chỉ expose khi primitive đạt parity. Một feature chưa đạt phải giữ unsupported trong capability matrix.

### 31.2 Clip lifecycle và text replacement

- Apply/edit/reset motion qua Command Service, một gesture meaningful một history entry.
- Move không đổi local animation; trim/split giữ visible phase/origin theo contract P27, không tự restart character animation ở nửa clip.
- Duration edits có policy in/out overlap, short clip, loop remainder, stagger completion; reject conflict hoặc dùng explicit command option, không silent rescale.
- Text đổi độ dài/newline/wrap phải rebuild layout+unit mapping và recompute schedule deterministic. Animation giữ intent/order/duration policy; empty text không panic/divide by zero.
- Generic item keyframes vẫn compose với per-unit motion đúng thứ tự; existing clip speed/source mapping không bị chỉnh để giả animation.

### 31.3 Native sample reconstruction

- Dùng source font local đã xác định khi được dùng cho test; nếu thiếu thì diagnostic và fallback được chọn rõ. Không phân phối font CapCut chỉ vì test cần nó.
- Dựng gradient trắng–vàng, viền nâu, hai bóng, size/anchor/geometry và stagger entrance tương ứng sample resource.
- Recipe native dùng Tinix typed primitives; không chạy hoặc nhúng Lua/JS. Lưu definition provenance là `recreated_reference`, không `fully_imported`.
- Render text gốc, `Tinix`, text Việt dài/wrapped và NFC/NFD; chữ đổi mà style/motion vẫn là editable state.
- Đối chiếu reference MP4 ở frame mốc/ROI theo tolerance P27. Giữ bảng khác biệt về font/effect/timing; pixel khác do renderer phải được báo, không gọi “identical”.

### 31.4 Qualification

- Frame sampling ở đầu/giữa/cuối in, từng stagger, loop boundary, out và frame cuối; replay seek/scrub không stale.
- Full lifecycle save/reopen, undo/redo, duplicate, trim/split và export on a machine/session with no CapCut running.
- Native user smoke đủ xác nhận edit content + preview + export. Chưa có library hoàn chỉnh vẫn có thể dùng command-backed fixture/editor controls tối thiểu.

## 3. Acceptance

- **P31-AC-01:** All/grapheme/word/line motion deterministic, marks/ligatures theo mapping contract, không codepoint slicing sai.
- **P31-AC-02:** In/loop/out/stagger/easing và supported reveal primitives đạt preview/export parity tại tất cả frame boundaries.
- **P31-AC-03:** Trim/split/move/duplicate/duration edits và text replacement tuân lifecycle policy, giữ undo/redo/save/reopen.
- **P31-AC-04:** Short/empty/long/multiline text, fractional fps và seeded order không crash hoặc drift.
- **P31-AC-05:** Native recreated sample có style và chuyển động tương ứng, với reference ROI/timing report cùng sai khác rõ.
- **P31-AC-06:** Sample đổi được nội dung ngắn/dài/có dấu và render được không cần CapCut hoặc source MP4.
- **P31-AC-07:** Captured native edit/play/seek/export smoke và checks/build/restart/runtime evidence PASS.
- **P31-AC-08:** W1/W2 và playback/caption/keyframe/parity regressions vẫn đạt budget/gates.

## 4. Dừng và bàn giao

T1 chưa đạt thì không làm library/importer/agent production. Giữ status FAIL/BLOCKED chính xác nếu chỉ một baked video giống mẫu hoặc ảnh chữ được đổi mà animation mất. Lưu template fixture, applied commands, exact frames, user text variants, reference comparison và benchmark dưới phase-31.

P32 nhận bộ primitives đã kiểm chứng, không một danh sách animation chỉ có tên/thumbnail.
