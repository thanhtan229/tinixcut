# Phase 27 — Template Contract, Fixtures và Baseline

**Status:** COMPLETE — Đã hoàn thành nghiệm thu specification, fixture corpus, compatibility matrix và baseline.
**Dependency:** Baseline hiện hành/schema v9 và evidence CapCut; chưa yêu cầu đóng toàn bộ qualification môi trường P26.
**Gate:** Gate 27 PASS — Đã có specification đo được trước khi chọn renderer.

## 1. Mục tiêu và tài liệu cần đọc

Chốt option, semantics, giới hạn và fixture cho chương trình, bảo đảm agent sau không tự diễn giải “giống CapCut”. Đọc `AGENTS.md`, roadmap, `ARCHITECTURE.md` phần domain/command/composition/parity, Phase 19, checkpoint P26 và nghiên cứu CapCut trong `RESEARCH.md`. Dùng `DEVELOPMENT_WORKFLOW.md` khi chạy baseline.

## 2. Phạm vi công việc theo slice

### 27.1 Baseline và inventory

- Kiểm tra source hiện tại, schema và git diff; không suy ra implementation từ tên preset hoặc plan cũ.
- Ghi các command/automation/text/renderer seams có thể mở rộng và các font fallback còn thiếu parity.
- Chạy targeted baseline text, command, persistence, export, playback; ghi failures/warnings/harness timeout tồn tại trước task.
- Đọc kết quả P26 đã có. Lỗi nền tảng trực tiếp ảnh hưởng text/parity phải giải quyết trước P28; clean-VM/hardware qualification độc lập vẫn được theo dõi đến P36.

### 27.2 Fixture corpus

- Index draft/resource/video của mẫu người dùng bằng hash và logical fixture ID. Đọc snapshot ổn định, không ghi vào CapCut draft/cache.
- Trích key facts cho một sample: effective rich style, font identity, layer geometry, character animation family, native duration và reference frame timestamps.
- Tạo/specify fixtures gốc Tinix cho solid/gradient, stroke/shadow, glow, background, rich spans, multilayer và in/loop/out. Không cần mua pack để qua phase này.
- Corpus chữ: sample gốc, `Tinix`, đoạn Việt dài nhiều dòng, NFC/NFD tương đương, emoji có ZWJ, empty và whitespace, ligature Latin, CJK và RTL. Ghi rõ supported/diagnostic-only khi chưa đủ glyph.
- Các tài nguyên font/ảnh không được phép phân phối chỉ là reference local, không commit hay đóng gói. Golden tests dùng font có thể phân phối và hash cố định; phép so với CapCut dùng resource local nếu còn hiện hữu.

### 27.3 Contract semantics

- Chốt units: time canonical `RationalTime`, màu/alpha, project pixels/reference canvas, anchor/pivot, stroke/shadow units, layer order và transform multiplication order.
- Chốt “character” = grapheme hiển thị; shaping cluster/glyph là derived data. Range rich spans phải có một quy ước indexing thống nhất Rust/TypeScript, không trộn byte offset/UTF-16/grapheme index.
- Chốt behavior empty text, missing glyph/font, overflow, wrap, auto-fit min/max, line spacing, direction/bidi, style bounds và bounds của shadow/glow.
- Chốt in/loop/out, stagger order, animation time domain, loop phase, trim/split, stretch duration và xung đột khi tổng in/out lớn hơn clip. Không auto đổi nhịp âm thầm.
- Chốt phân biệt template definition, resolved instance, user override, typed parameter binding và resource source/cache.
- Chốt matrix core/extended/unsupported cùng validation limits từ roadmap; plugin/native-script không trở thành escape hatch.

### 27.4 Ngân sách và acceptance lock

- Ghi cấu hình reference Windows host, baseline latency/memory và installer/startup hiện có.
- Workload W1: một title 120 grapheme tại 1080p30, gradient + hai viền + hai bóng + stagger. W2: bốn title tương tự đồng thời; W3: một instance 16 lớp với text/hình/shape; W4: export 60 giây. Thêm 9:16 và 60fps để xác định giới hạn.
- Đề xuất ban đầu: W1 text evaluation+raster+delivery p95 ≤16ms, W2 ≤33,3ms; bounded overlay cache mặc định ≤256MiB. P28 đo rồi chốt supported workload, không coi số đề xuất là benchmark đã đạt.
- Chốt pixel/layout/timing tolerances dùng kiểm tra overlay, preview/export và đối chiếu CapCut. So text ROI/alpha, không so toàn canvas đen để tạo điểm cao giả.
- Dựng checklist người dùng kiểm tra tại các milestone; manual qualification vẫn cần evidence, không thay bằng build PASS.

## 3. Đầu ra

Specification/fixture index/compatibility matrix/evidence baseline nằm dưới `artifacts/animated-text-templates/phase-27/`. Contract lâu dài cần được đặt ở source-of-truth thích hợp sau P28; không tạo một architecture song song trong fixture metadata.

## 4. Acceptance và Kết quả Thực tế

- **P27-AC-01 [PASS]:** Source inventory và baseline ghi nhận đầy đủ tại `artifacts/animated-text-templates/phase-27/baseline-inventory.json`. Git revision `e6462544b6956054ee50af40727d44e92121487b`. Typecheck PASS (0 errors), Vitest 44/44 PASS, production build PASS, cargo fmt check PASS, cargo test lib 141 PASS (1 POC5 matrix skipped theo policy), integration suites 81 PASS (tổng 222 tests Rust PASS), cargo clippy sạch với 3 warnings cũ, cargo build PASS, đo đạc kích thước binary snapshot (debug 24,498,688 B, release 7,381,504 B, installer 59,085,922 B).
- **P27-AC-02 [PASS]:** Mẫu người dùng `TINIX_CAPCUT_IMPORT_TEST` được index chi tiết tại `capcut-sample-index.json` gồm draft/video/style/script/font/template-wrapper SHA256 hashes, exact 29 graphemes / 24 non-space chars, layer geometry và scale factors (0.31 và 1.903216, combined 0.589997), thông số gradient 118°, brown stroke `#AD5100` width 0.03808219, 2 shadows (`#AD5100` và `#FFA758`), in-stagger 1,033s với công thức virtual timeline và phân nhóm chuyển động 21 ký tự trượt + 3 ký tự nảy 2 chặng Bezier và xoay Z. Ghi rõ giới hạn font không phân phối và Lua runtime độc quyền không thực thi native.
- **P27-AC-03 [PASS]:** Ma trận 19 options (Core, Extended, Unsupported) với owner phase (P29–P35), test scenarios, định dạng `.tinixtpl` và validation limits (max 16 layers, 8 text slots, 512 graphemes/slot, 4 strokes, 4 shadows, 8 gradient stops) được chốt tại `compatibility-matrix.json`.
- **P27-AC-04 [PASS]:** Semantics chốt tại `contract-semantics.md`: Canonical RationalTime, thứ tự nhân ma trận 3 cấp (Item -> Layer -> Unit) và thứ tự 2D affine Transform, Painter's order (background -> shadows/glows -> outer strokes -> glyph fill -> inner strokes) và đơn vị quy chuẩn, character là Extended Grapheme Cluster (Unicode UAX #29), rich text span indexing dùng grapheme offset với chính sách rebind tất định khi sửa text, duration collision resolution với mặc định Reject và tùy chọn ProportionalScale/ClampOut tường minh, quy tắc Move/Trim/Split/Stretch, whitelist parameter bindings và giới hạn an toàn gói `.tinixtpl`.
- **P27-AC-05 [PASS]:** 14 fixtures (F01–F14) chốt tại `fixture-corpus.json` kiểm tra CapCut sample với các biến thể chữ ('Tinix', 'Hi'), tiếng Việt NFC/NFD, emoji ZWJ, multiline, edge cases, Latin ligatures, ranh giới CJK/RTL, multi-stroke/shadow stress, gradient, duration collision policies, 4 tỷ lệ khung hình, rich spans + background/glow, multilayer density stress (16 layers), và full in/loop/out lifecycle với 8 mốc sampling checkpoints. Quy tắc ROI + alpha tolerance được xác định.
- **P27-AC-06 [PASS]:** Reference Windows host (Windows 11 Pro 24H2, i5-13400F, 64GB, RTX 3090, 990 PRO NVMe), baseline metrics (dung lượng binary, Phase 17 transport/export baselines), 4 workloads W1–W4 và đề xuất ngân sách (W1 p95 <= 16ms, W2 p95 <= 33.3ms, overlay cache <= 256MiB) chốt tại `workload-budgets.md`. Bảng checklist kiểm định thủ công 5 cột mốc được chốt tại `milestone-verification-checklist.md`.
- **P27-AC-07 [PASS]:** Gaps P26 (GUI QA, Clean-VM, Hardware Encoder) được giữ nguyên, không fake PASS. Không có app code hoặc file CapCut nào bị chỉnh sửa trái phép.

## 5. Bàn giao sang Phase 28

Tất cả 7 AC đã PASS với evidence đầy đủ trong `artifacts/animated-text-templates/phase-27/`. Gate 27 mở đường cho Phase 28 (Shared Text Renderer POC) tiến hành thử nghiệm Skia/Rust và đo đạc trên reference host.
