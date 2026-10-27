# Animated Text Templates — Roadmap Phase 27–36

**Ngày lập:** 2026-10-11
**Trạng thái:** Phases 27-36 implemented; Gate T5 passed for automated/static/package scope, with native GUI, clean-VM, and hardware qualification deferred.
**Mục tiêu:** Text template phong phú, chỉnh sửa được, có animation; chuyển đổi những template CapCut được hỗ trợ; agent tạo template theo cùng mô hình Tinix.
**Phạm vi:** TinixCut trên Windows, kế thừa schema/Command Service/composition hiện có.

## 1. Quyết định về hướng thực hiện

Xây năng lực template gốc của Tinix trước, chứng minh bằng mẫu thực tế `TINIX_CAPCUT_IMPORT_TEST`, rồi mới làm importer CapCut và agent. Importer và agent là các client của cùng hệ thống template; không tạo renderer, reducer hoặc định dạng project riêng.

Không dùng MP4 đã xuất làm lời giải cho template chỉnh sửa được. MP4 chỉ là tài liệu đối chiếu hoặc media đã render, được ghi rõ là không sửa được chữ. Khả năng đọc một draft JSON không đồng nghĩa với chuyển được hiệu ứng native của CapCut.

Nguồn sự thật:

- `ARCHITECTURE.md`: kiến trúc hiện hành; thay đổi bền vững chỉ cập nhật sau gate P28 và trước triển khai phụ thuộc.
- Tài liệu này: phạm vi, thứ tự, dependency, các gate chung và milestone của chương trình.
- Các `plan_phase27_...` đến `plan_phase36_...`: công việc và acceptance cụ thể của từng phase, không lặp toàn bộ roadmap.
- `RESEARCH.md`: bằng chứng CapCut, nghiên cứu renderer, kết quả POC và hạn chế.
- `PROJECT_STATUS.md`: phase/slice đang thực hiện, gate đã đạt, blocker và bước tiếp theo.
- `UX_UI.md`, `DEVELOPMENT_WORKFLOW.md`: chuẩn UI và cách kiểm tra/build/restart runtime.

Phase 19 đã hoàn thành nền tảng sáu preset. Phase 26 cùng các qualification môi trường còn thiếu được giữ nguyên, không tự chuyển thành PASS. Chương trình này bổ sung Phase 27–36, không làm lại Phase 19.

## 2. Điểm xuất phát đã kiểm chứng

- Repo hiện dùng schema v9, `ItemKind::Text`, `TextStyle`, template ID/version, generic keyframes, Command Service, CLI/MCP và normalized composition.
- Sáu preset hiện tại chủ yếu là style; tên preset không chứng minh có character animation. Registry hiện không định nghĩa animation riêng.
- Preview chữ hiện dùng DOM; export có `drawtext` và font fallback Arial. Weight/line-height và font thực tế chưa khớp đầy đủ. Không kéo dài hai đường text-render độc lập khi thêm rich text.
- Mẫu CapCut đã đọc được chữ, canvas, timing và resource references. Style thật có gradient trắng–vàng, viền nâu, hai bóng; animation vào 1,033333 giây dùng Lua/Amaz theo ký tự, không nằm trong generic segment keyframes.
- Thử nghiệm trước chỉ tạo được editable text đơn giản và video đã render. Không có full importer hoặc reusable imported template.
- Evidence: `artifacts/capcut-user-sample-20261011/README.md` và `sample-analysis.json`; font/resource CapCut chỉ tham chiếu trên máy, chưa trở thành tài nguyên phân phối của Tinix.

## 3. Phạm vi công cụ cần giao

“Đầy đủ option” được chuyển thành ma trận tính năng có kiểm thử, không thành lời hứa hỗ trợ mọi hiệu ứng nội bộ CapCut.

| Nhóm | Phạm vi giao trong chương trình | Phase chính |
| --- | --- | --- |
| Typography | Font/weight/style, size, tracking, kerning theo shaping, line height, alignment, wrap/auto-fit, rich spans, tiếng Việt có dấu | 29–30 |
| Appearance | Solid/linear/radial gradient, nhiều viền, nhiều bóng, glow, background/padding/radius, các kiểu depth/retro bằng lớp 2D | 30, 32 |
| Motion | In/loop/out; toàn lớp/ký tự/từ/dòng; stagger, hướng/thứ tự, easing, position/scale/rotation/opacity; reveal/wipe và bounded blur/gradient motion nếu đạt gate | 31–32 |
| Layout nhiều lớp | Nhiều text slot, hình/sticker cục bộ, shape nền/decoration, layer order, anchor/pivot, responsive sizing | 32 |
| Reuse | Lưu selection thành template, version, parameters, reset/overrides, import/export pack, thumbnail/animated preview, favorites/search | 32–33 |
| CapCut | Inspect draft/pack, resource resolution, compatibility report, adapter cho style/animation đã xác nhận, conversion preview, apply atomic | 34 |
| Agent | Prompt/reference → draft template → validate/dry-run → preview → apply/save; thực hiện qua CLI/MCP và UI dùng chung | 35 |
| Delivery | Performance, cancel, recovery, offline resources, installer và native qualification | 36 |

Các hiệu ứng 3D thực, arbitrary shader/plugin, motion tracking, template cloud marketplace và việc thực thi Lua/JS CapCut không thuộc phạm vi này. Khi gặp chúng, importer báo không hỗ trợ; chỉ mở phase mới nếu có yêu cầu và bằng chứng. Kiểu depth 2D không được quảng cáo là 3D native.

Giới hạn ban đầu để tránh workload vô hạn: tối đa 16 lớp/instance, 8 text slot, 512 grapheme/text slot, 4 viền, 4 bóng, 8 gradient stop và một cấp layer phẳng. Phase 27 chốt các giới hạn cùng policy nhập text vượt giới hạn; Phase 28 kiểm chứng ngân sách. Đây là validation/resource limits, không được âm thầm cắt nội dung người dùng.

## 4. Kiến trúc dự kiến và gate quyết định

```text
UI authoring / CapCut adapter / agent CLI-MCP
                   ↓
validated template draft + dry-run
                   ↓
Command Service → canonical resolved project state
                   ↓
normalized composition + canonical animation evaluation
                   ↓
shared shaping/layout + text-overlay renderer
           ↙                     ↘
      preview                FFmpeg export
```

Ưu tiên một text compositor dùng chung ở native/core, tạo overlay RGBA để cả preview và export tiêu thụ. Đây là hướng thử POC, chưa phải quyết định chọn thư viện. Chỉ bổ sung backend cho text/decoration; không thay toàn bộ video/audio compositor.

P28 phải đo renderer, shaping, alpha transport qua Tauri/WebView, chi phí export streaming, Windows build và packaging. Skia qua Rust là ứng viên ưu tiên để thử; phương án Rust shaping/raster nhỏ hơn chỉ triển khai POC khi có lý do từ baseline. Chi tiết nguồn và hạn chế đặt trong `RESEARCH.md`. Không khóa Skia, GPU, CanvasKit hoặc engine mới trước khi có bằng chứng.

Canonical model lưu style/layout/motion/resource identity đã resolve và template identity; không lưu Lua, JavaScript, shader source, font handle, glyph atlas, PNG cache path hoặc FFmpeg graph. Layer hierarchy chỉ là cấu trúc hữu hạn thuộc một template instance trong project model, không là scene graph/editor riêng.

Instance vẫn render được khi xóa pack khỏi thư viện. Font/hình là source asset có identity/hash và trạng thái missing/relink; glyph layout/thumbnail/overlay chỉ là derived cache. Thiếu font không được tự đổi font trong final export: người dùng chọn fallback bằng command rõ ràng.

## 5. Thứ tự thực hiện

| Phase | Đầu ra chính | Điều kiện để đi tiếp |
| --- | --- | --- |
| [27 — Contract & baseline](plan_phase27_template_contract.md) | **COMPLETE (Gate 27 PASS)**: Ma trận option, 14 fixtures, contract semantics và ngân sách đã chốt | Baseline có evidence, contract không mâu thuẫn architecture |
| [28 — Renderer POC](plan_phase28_text_renderer_poc.md) | **COMPLETE (Gate 28 PASS)**: Shared text-render POC, parity/performance/packaging decision | Chọn được một backend đáp ứng gate; ADR được reconcile |
| [29 — Domain & commands](plan_phase29_rich_text_domain.md) | Rich-text/animation contract, migration, command/history/automation | Không mất dữ liệu cũ, mutation atomic và idempotent |
| [30 — Rich-text rendering](plan_phase30_rich_text_rendering.md) | Style/font/layout/alpha đầy đủ trong preview và export | Rich-style parity suite PASS |
| [31 — Animation & sample](plan_phase31_text_animation.md) | Motion evaluator và mẫu người dùng chỉnh sửa được | Native sample gate PASS với chữ ngắn/dài/có dấu |
| [32 — Layered templates & packs](plan_phase32_layered_template_packs.md) | Nhiều lớp, parameters, reusable definitions, pack I/O | Pack round-trip, overrides và resource lifecycle PASS |
| [33 — Authoring & library UI](plan_phase33_template_authoring_ui.md) | Browser template và editor dễ dùng | Native authoring flow và undo/redo PASS |
| [34 — CapCut importer](plan_phase34_capcut_template_import.md) | Resource-aware adapter, compatibility report, conversion | Mẫu thật nhập thành editable animated template; không báo thành công giả |
| [35 — Agent generation](plan_phase35_agent_template_generation.md) | Agent tạo/sửa/lưu template qua schema và Command Service | Một workflow agent thật end-to-end cùng failure/cancel evidence |
| [36 — Final qualification](plan_phase36_template_qualification.md) | Release-ready integration, performance, package và user guide | Acceptance toàn chương trình PASS; deferred item được nêu chính xác |

Đi tuần tự. Không làm importer trước renderer, không làm agent trước schema/library. Có thể tìm thêm mẫu ở P27/P28 nhưng không code phase sau hoặc sửa cùng module song song khi dependency chưa đạt.

## 6. Milestone sản phẩm

- **T1 — P31:** Chính mẫu CapCut của người dùng được dựng lại bằng Tinix, sửa được chữ và giữ hiệu ứng trong phạm vi đã xác nhận. Đây là template Tinix tái dựng, chưa gọi là CapCut import.
- **T2 — P33:** Người dùng tự tạo, chỉnh, lưu và dùng lại template nhiều lớp; có pack và browser có animated preview.
- **T3 — P34:** Một tập template CapCut có adapter và evidence được chuyển tự động; mỗi template có support report.
- **T4 — P35:** Agent tạo template mới hoặc dựng lại từ reference qua cùng đường mutation.
- **T5 — P36:** Bộ công cụ đủ điều kiện dùng ổn định và đóng gói, với danh sách hỗ trợ rõ ràng.

Không chờ hoàn thành tất cả phase mới xem được kết quả. T1 là gate bắt buộc, tránh làm thư viện đẹp trên nền render chưa đúng.

## 7. Kiểm tra dùng chung

### 7.1 Fixture và visual evidence

- Fixture corpus có text Việt NFC/NFD, emoji/grapheme ghép, multiline, short/long, wrap, ligature; thêm Arabic/RTL và CJK để xác định phạm vi, không âm thầm gắn nhãn hỗ trợ.
- Ratios: 16:9, 9:16, 1:1, 4:5; fps: 24, 25, 30, 60 và 30000/1001.
- Kiểm tra t=0, từng mốc stagger, cuối in, loop boundary, đầu/cuối out, frame cuối clip; empty/short/zero cases có policy cụ thể.
- Shared renderer: ưu tiên so sánh overlay lossless cùng font/hash, layout metrics và alpha; quy tắc pixels phải chốt ở P28, không nâng tolerance để giấu lỗi.
- CapCut comparison dùng crop/bounds của chữ trên các frame mốc, ghi khác biệt có chủ đích; SSIM toàn nền đen không phải bằng chứng tương đương template.
- Chữ có thể sửa, thời gian canonical, save/reopen, trim/split/move, undo/redo và project không cần CapCut để render là điều kiện độc lập với visual similarity.

### 7.2 Gate coding slice

Mỗi slice là một thay đổi coherent, có test hành vi và evidence phù hợp. Trước khi chốt phase phải chạy checks theo `DEVELOPMENT_WORKFLOW.md`: targeted tests, TypeScript typecheck, frontend build, Rust format/build/Clippy và regression/parity suite có liên quan; integration phases chạy toàn suite với timeout có báo cáo.

Known POC5 qualification harness có thể không kết thúc, all-target Clippy có warning cũ: ghi baseline cụ thể, chạy có timeout, không coi timeout là PASS hoặc bỏ test âm thầm. Không sửa unrelated tests chỉ để có số xanh. Mỗi phase file nêu regression trọng tâm; không viết test chỉ soi implementation.

Sau mỗi task thay code/UI/config/runtime, hoàn tất checks/build rồi restart đúng runtime project và xác nhận mới. User đã cho phép ngắt task local đang chạy; không hỏi lại chỉ vì runtime bận. `pnpm tauri dev` là workflow hiện có; native core không có backend server độc lập. Xác minh ownership bằng command line/cwd/PID, khởi động background ẩn trên Windows, verify frontend response, native process mới và typed `app_health_check`. HTTP 200 riêng lẻ không chứng minh native core đã cập nhật. Thiếu native surface thì ghi rõ phần health/GUI chưa quan sát, không fake PASS.

Tài liệu/plan-only không chạy build hoặc restart. Không commit/push tự động.

## 8. Quy tắc bàn giao cho coding agent

1. Đọc `AGENTS.md`, roadmap, đúng phase file và checkpoint; thêm companion docs theo router, không quét lại mọi tài liệu.
2. Kiểm tra git diff và phase trước; repo đang có nhiều thay đổi chưa commit, phải giữ nguyên công việc có sẵn.
3. Chọn slice đầu tiên chưa hoàn thành; chỉ thực hiện trong dependency đã PASS.
4. Trước coding, lock input/output/acceptance và baseline; khi hypothesis sai, lưu evidence rồi sửa plan/ADR liên quan trước khi đổi kiến trúc.
5. Cuối slice lưu evidence dưới `artifacts/animated-text-templates/phase-NN/`, gồm fixture/resource hash, revision, command/test, hình/video, duration/performance và limitation.
6. Cuối phase đánh dấu từng AC là PASS/FAIL/BLOCKED với evidence. Để trạng thái IMPLEMENTED WITH DEFERRED QUALIFICATION nếu GUI/môi trường chưa kiểm được; không đi qua gate phụ thuộc chưa đạt.
7. Cập nhật checkpoint ngắn trong `PROJECT_STATUS.md`; phase plan chỉ ghi acceptance evidence, không thành nhật ký dài.

## 9. Rủi ro và đường xử lý

| Rủi ro | Gate và cách xử lý |
| --- | --- |
| Native renderer làm build/installer nặng hoặc preview IPC chậm | P28 đo cả startup, transport, export và installed app; thử phương án nhẹ hơn trước khi đổi kiến trúc |
| Font/Unicode/layout khác nhau giữa preview/export | Font identity + shared shaping + lossless overlay comparisons; P30 chặn release nếu chưa đạt |
| Animation “ký tự” làm rời dấu tiếng Việt hoặc phá ligature | Grapheme/cluster mapping, shape trước animate, đơn vị/chế độ selection rõ; P31 |
| Thay chữ làm wrap và stagger đổi bất ngờ | Responsive/layout policy và duration conflict rõ; tested short/long/multiline |
| Pack chứa cache reference hoặc mất resource | Source resource identity, portable pack rules, missing/relink và render khi pack bị xóa; P32 |
| CapCut pack là format khác, mã hóa hoặc Lua không nhận diện | Inspect trước conversion; support report; không đoán hoặc chạy script; P34 |
| Agent tạo script, dữ liệu quá lớn, mutation stale | Typed schema/limits, capability discovery, staging/dry-run, revision conflict và cancel; P35 |
| Tính năng mới che regression playback/captions/transition | Retain P26 evidence và chạy regression đầy đủ; P36 không xóa các gate cũ |

## 10. Trạng thái bàn giao hiện tại

Phase 27 (Template Contract, Fixtures và Baseline), Phase 28 (Shared Text Renderer POC và Architecture Gate) và Phase 29 (Rich Text Domain, Schema v10, Commands) đã hoàn thành đạt chuẩn Gate 27, Gate 28 & Gate 29 PASS với đầy đủ bằng chứng kiểm định tại `artifacts/animated-text-templates/phase-27/`, `phase-28/` và `phase-29/`. Ứng viên B (Pure Rust Native Typography & Rasterization Stack) được chính thức chọn và lock qua ADR-007. Các Phase 30–36 tiếp tục được triển khai tuần tự theo kế hoạch. Tiếp tục bảo lưu các hạng mục qualification môi trường Phase 26 trong baseline. Lưu ý: benchmark Phase 28 chỉ đạt budget dưới profile test tối ưu đã ghi trong `phase-29/verification-log.md`.
