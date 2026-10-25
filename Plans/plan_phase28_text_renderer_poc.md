# Phase 28 — Shared Text Renderer POC và Architecture Gate

**Status:** COMPLETE (Gated & Verified — 2026-10-25).
**Dependency:** P27 PASS.
**Gate:** Gate 28 PASS — Candidate B (Pure Rust Native Shaping & Rasterization Stack) được chọn và lock qua ADR-007.

## 1. Mục tiêu

Chứng minh rich text + character transform + alpha có thể render đúng và đủ nhanh trong Tinix. POC chỉ thay text-overlay path; không thay decoder, video/audio transport hoặc toàn bộ compositor. Đọc architecture/parity/media boundary, P27 contract, `RESEARCH.md`, workflow và Tauri IPC hiện có.

## 2. Slice thực hiện

### 28.1 POC native raster/shaping

- Thử ứng viên Skia/Rust với feature set tối thiểu đủ shaping/layout, gradient, stroke/shadow và RGBA; pin version cho experiment, ghi build/toolchain/license/artifact size.
- Vẽ cùng một neutral fixture từ P27 ở frame chính xác; expose glyph/cluster metrics cần thiết để animate mà không tách dấu/ligature sai.
- Font resolution đầu vào phải là font asset/hash đã chọn; không dựa vào một font-family name được hai platform resolve khác nhau.
- Không lưu renderer object vào project hoặc thêm schema chính thức ở POC.

### 28.2 Hai consumer dùng cùng overlay

- Preview nhận overlay tại canonical time, layer geometry và alpha; đo transport thật qua Tauri/WebView, không chỉ benchmark native function.
- Export stream overlay/frame được bounded qua media abstraction vào FFmpeg với time/frame mapping chính xác; không tạo thư mục toàn video PNG vô hạn hoặc spawn FFmpeg mỗi frame.
- Chốt premultiplied/straight alpha, color space, scaling và overlay bounds; test trên nền trắng/đen/màu/video. Không có halo, bị cắt shadow hoặc nền đen giả.
- Cancellation, EOF, failure và stale request phải giải phóng worker/buffer/temp output.

### 28.3 Benchmark và Windows delivery

- Đo W1–W4 của P27: warm/cold cache, p50/p95, preview delivery/dropped frames, export speed, peak/cache memory và seek/edit latency.
- Kiểm tra build fresh với documented prerequisites; app cài đặt dùng runtime resources của mình, không dựa vào CapCut hoặc SDK path của máy dev.
- Đo startup/installer size. GPU chỉ được thử nếu CPU path không đạt và có evidence; CPU fallback phải có phạm vi/capability rõ.
- Nếu ứng viên đầu không đạt, ghi nguyên nhân rồi thử một stack shaping/raster nhẹ hơn có primary-source evidence. Không kéo đồng thời nhiều engine vào shipping app.

### 28.4 ADR và gate lock

- Ghi so sánh candidate, lựa chọn, dependency/version/features, shared renderer seam, alpha transport/export seam, cache/cancel strategy và supported workload.
- Update `RESEARCH.md` với measured evidence; update `ARCHITECTURE.md` bằng ADR text-overlay phù hợp numbering thật trong repo. Chỉ sau đó P29 mới triển khai durable contract.
- Giữ neutral template semantics độc lập backend; update workflow nếu thêm prerequisites build/package có tính lâu dài.

## 3. Acceptance

- **P28-AC-01:** Sample typography và style primitives vẽ được từ fixture, với shaping/cluster mapping có thể dùng cho animation.
- **P28-AC-02:** Preview và headless export dùng cùng font/layout/evaluation/renderer; cùng input lossless overlay phải cho kết quả deterministic.
- **P28-AC-03:** Captured preview vs lossless export ở cùng resolution/time đạt tolerance P27; encoded MP4 được kiểm tra riêng, không dùng codec artifact để nới renderer tolerance.
- **P28-AC-04:** Alpha tests không nền đen/halo/clipping; geometry/bounds đúng qua scale và rotation.
- **P28-AC-05:** Native + actual delivery performance đáp ứng budget đã lock. Nếu workload phải hạ, report rõ và sửa scope trước gate; không tự gắn PASS cho workload cũ.
- **P28-AC-06:** Bounded memory/backpressure/cancel/worker failure không leak, không để output final giả thành công.
- **P28-AC-07:** Windows build/package smoke đủ kiểm chứng dependency; unresolved clean-VM evidence được giữ riêng.
- **P28-AC-08:** ADR, composition boundary và workflow được reconcile; chỉ một shipping backend được chọn.

## 4. Evidence và điều kiện dừng

Lưu candidate report, dependency/toolchain versions, reference host, images/alpha masks, performance CSV/report, actual Tauri preview capture, FFmpeg output và cancel results dưới phase-28. Nếu không lấy được native capture, đó là BLOCKED đối với integration parity gate; native-only benchmark không thay được.

Dừng P29 nếu chỉ DOM preview đạt còn export khác font/layout, cần screenshot toàn WebView để xuất, không expose được cluster layout, hoặc packaging/cancel không khả thi. Không đẩy một POC thất bại thành một fallback im lặng.

Checks/restart áp dụng cho mọi POC slice sửa code theo roadmap; chưa có schema hoặc production importer mới ở phase này.

## 5. Kết quả nghiệm thu thực tế

- **P28-AC-01 [PASS]:** Typography mẫu và style primitives (118° linear gradient, 2 outer strokes, 2 blurred drop shadows) render chuẩn xác từ fixture font Apache-2.0 `Roboto-Bold.ttf` (SHA-256 `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`, `0` missing glyphs). Chuẩn hóa NFC (`unicode-normalization`) kết hợp UAX #29 extended grapheme clusters (`unicode-segmentation`) và rasterize mặt nạ thống nhất (`rasterize_relative_cluster_mask`) giữ nguyên vẹn dấu tiếng Việt (`ử`, `ệ`, `ế`) qua 21-char vertical slide và 3-char bounce + Z-rotation (`24` ký tự không khoảng trắng / 29 graphemes).
- **P28-AC-02 [PASS]:** Tính tất định tuyệt đối giữa Preview và Export. Cùng input sinh ra bộ đệm RGBA hoàn toàn trùng khớp từng bit ($\Delta\text{PSNR} = \infty$, 0 mismatch bytes trên 8,294,400 bytes).
- **P28-AC-03 [PASS]:** Consumer 1 Tauri IPC command `render_text_template_poc_preview` cung cấp `PreviewOverlayIpcResponse` với Dirty ROI `[667, 471, 1087, 570]` và ROI PNG (`166,497 bytes`, 436 KB JSON). Đối chiếu giải mã Preview ROI PNG với Export RGBA đạt **$\text{ROI SSIM} = 1.000000$** ($\ge 0.995$) và **$\text{ROI MAE} = 0.000000$** ($\le 0.005$). Thời gian truyền tải IPC đầu cuối đạt p95 = 13.83 ms ($\le 16.0\text{ ms}$). Video MP4 H.264 xuất thực tế đạt **$\text{MP4 ROI SSIM} = 0.9977$** ($\ge 0.985$).
- **P28-AC-04 [PASS]:** Alpha channel integrity được chứng minh toán học và trực quan qua 4 nền chuẩn (`poc_alpha_white.png`, `poc_alpha_black.png`, `poc_alpha_color.png`, `poc_alpha_video.png`) với straight-alpha Porter-Duff Over. Kiểm tra viền chu vi xác nhận 100% không bị cắt cụt stroke/shadow. Bảo toàn hình học khi scale (`0.59x`, `1.208x`) và xoay Z (`-21°`, `+6°`).
- **P28-AC-05 [PASS]:** Đáp ứng toàn bộ ngân sách tham chiếu tại `benchmark-metrics.csv`: W1 Cold = 11.31 ms / Warm p95 = 7.10 ms ($\le 16.0\text{ ms}$); W2 Cold = 13.64 ms / Warm p95 = 12.21 ms ($\le 33.3\text{ ms}$); W3 16:9 Cold = 20.72 ms / Warm p95 = 20.32 ms ($\le 50.0\text{ ms}$); W3 9:16 Cold = 26.86 ms / Warm p95 = 19.78 ms ($\le 50.0\text{ ms}$); W1 Preview IPC Delivery p95 = 13.83 ms ($\le 16.0\text{ ms}$); Seek Latency p95 = 8.25 ms ($\le 16.0\text{ ms}$); Edit Latency p95 = 2.96 ms ($\le 25.0\text{ ms}$); W4 Throughput = 5.15× RTF ($\ge 1.0\times$ RTF). `BoundedGlyphCache` sử dụng 402,683 bytes ($0.38\text{ MiB} \le 256\text{ MiB}$).
- **P28-AC-06 [PASS]:** Headless export stream 60 frames trực tiếp qua stdin pipe của FFmpeg, luồng `stderr` được xả song song trên background thread triệt tiêu hoàn toàn nguy cơ deadlock pipe OS. Tiến trình con hủy ngay lập tức và dọn dẹp file dở dang sạch sẽ khi nhận token cancel.
- **P28-AC-07 [PASS]:** Windows clean delivery với zero external C++/LLVM/Python toolchains. Đo kích thước binary nhị phân thực tế: Debug tăng +1.27 MB (25.77 MB vs 24.50 MB baseline), Release tăng +1.18 MB (8.56 MB vs 7.38 MB baseline), hoàn toàn nằm trong ngân sách đóng gói (so với +42 MB của Skia). Tiếp tục bảo lưu mục qualification clean-VM Phase 26.
- **P28-AC-08 [PASS]:** Khóa lựa chọn Ứng viên B (Pure Rust Native Typography & Rasterizer Engine) trong `ADR_007_SHARED_TEXT_RENDERER_AND_OVERLAY_PIPELINE.md` và `ARCHITECTURE.md`. Trạng thái toàn bộ chương trình sẵn sàng cho Phase 29.
