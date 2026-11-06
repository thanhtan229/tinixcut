# TinixCut — Kế hoạch hoàn thiện sản phẩm, Phase 48–57

**Ngày lập:** 2026-11-04
**Trạng thái:** ĐANG NGHIỆM THU CUỐI (cập nhật 2026-11-05) — G54 có lỗi seek/preview chưa đạt; G55 còn yêu cầu review độc lập và brief/project thực; G56 đã PASS với native stale-save conflict, Save a Copy và reopen; G57 installed UI/upgrade còn mở. Gom blocker môi trường cuối cùng, không tuyên bố phase PASS khi thiếu evidence.
**Điểm xuất phát:** Phase 37–47 đã được ghi nhận hoàn tất trong phạm vi nghiệm thu và các giới hạn của từng phase. Điều này chưa tương đương với một editor đã được kiểm chứng trên mọi media/máy.
**Tài liệu liên quan:** [PLAN.md](../PLAN.md), [ARCHITECTURE.md](../ARCHITECTURE.md), [PROJECT_STATUS.md](../PROJECT_STATUS.md), [roadmap 37–47](plan_editor_mcp_ai_roadmap.md), [Phase 47](phase47_webview_profile_and_recovery_consistency.md).

## 1. Định nghĩa sản phẩm cần đạt

TinixCut là app dựng video desktop Windows cho video ngắn và video nội dung thông thường. Người dùng có thể hoàn thành quy trình import → xem/chọn nguồn → dựng nhiều track → xử lý lời thoại/nhạc/chữ → xem trước → lưu/phục hồi → xuất MP4. MCP và AI thực hiện cùng quy trình trên project chỉnh sửa được, có thể xem xét, sửa tay và Undo/Redo.

Phạm vi phát hành đầu tiên của kế hoạch này là SDR, MP4 H.264/AAC, 1080p ở 30 fps; canvas 16:9, 9:16, 1:1 và 4:5. Media đầu vào có thể khác frame rate/resolution, nhưng phải qua chính sách chuẩn hóa và báo lỗi rõ. Khả năng 4K đầu vào, 60 fps, media dài và nhiều lớp chỉ được công bố sau khi workload tương ứng qua gate; không suy từ fixture ngắn.

Không đưa HDR chuyên nghiệp, multicam, cộng tác nhiều người, remote MCP/OAuth, cloud render, tracking/rotoscoping, marketplace hay toàn bộ CapCut parity vào 10 phase này. Khi một hạng mục cần đổi kiến trúc hoặc schema, cập nhật ADR/architecture và thiết kế migration trước khi code.

## 2. Nguyên tắc kế thừa và nghiệm thu

- Mở rộng nền có sẵn: Command Service, Project Domain Model, history, atomic persistence, Job Manager, composition, proxy/cache, waveform, caption/template và planner. Không xây lại các module chỉ vì UI chưa tiện dùng.
- Mọi mutation do UI, shortcut, CLI, MCP hoặc AI đều đi qua cùng Command Service. Dữ liệu analysis/cache không trở thành semantics của project.
- Dùng RationalTime và frame/sample grid chuẩn. Preview/export đọc cùng composition; mọi hiệu ứng mới phải khóa thứ tự áp dụng và sai số so sánh trước khi triển khai.
- Bảo vệ revision, idempotency ledger, transaction và stale-writer khi edit, Undo/Redo, restore, save/reopen và retry. Snapshot nội dung lịch sử không được làm mất replay protection đã cam kết.
- Mỗi phase bắt đầu bằng đối chiếu source/test hiện tại và chọn gap nhỏ nhất. Hạng mục đã có bằng chứng phù hợp được kế thừa; chỉ chạy lại khi code/candidate hoặc rủi ro thay đổi.
- Test dùng bản project/media riêng và manifest nguồn/quyền sử dụng; giữ nguyên sample gốc, recovery copy và các thay đổi chưa rõ ownership. Không gom working tree hiện tại vào một commit rộng.
- Trạng thái gate: `PLANNED`, `IN PROGRESS`, `PASS`, `PASS WITH LIMITS`, `BLOCKED`. Gate thiếu evidence bắt buộc không được coi là PASS.
- Khi triển khai: test mục tiêu trước; build/typecheck theo phần bị ảnh hưởng; restart đúng runtime của project sau thay đổi cuối và xác minh code mới theo AGENTS/DEVELOPMENT_WORKFLOW. Ghi candidate, fixture, lệnh/kết quả và giới hạn trong evidence của phase.
- A/V vật lý đã hiệu chuẩn, clean Windows VM và encoder phần cứng của bản cài vẫn là kiểm tra tùy chọn phục vụ claim tương ứng. Không dùng chúng để chặn toàn bộ roadmap hoặc thay thế các kiểm tra software/persistence cần thiết.
- Người dùng đã duyệt tự triển khai tuần tự Phases 48–57 và yêu cầu gom blocker thật sự để hoàn thiện cuối cùng; các gate chưa có evidence bắt buộc vẫn giữ OPEN đến khi blocker được đóng.

## 3. Bản đồ 10 phase

| Phase | Kết quả người dùng nhận được | Phụ thuộc chính | Gate |
|---|---|---|---|
| 48 | Settings bền vững, mở/lưu/phục hồi project tin cậy | Baseline 47 | G48 |
| 49 | Xem, tìm, đánh dấu và quản lý media thuận tiện | 48 | G49 |
| 50 | Timeline đủ thao tác để dựng video hằng ngày | 48–49 | G50 |
| 51 | Lời thoại, audio trong video và nhạc được kiểm soát rõ | 50 | G51 |
| 52 | Chỉnh hình và animation trực quan, nhất quán khi xuất | 50; 51 cho volume automation | G52 |
| 53 | Sửa phụ đề/transcript và dựng theo văn bản | 49–51; 52 cho presentation | G53 |
| 54 | Preview và background jobs ổn định ở workload công bố | 49–53 | G54 |
| 55 | AI tạo bản dựng theo brief và thời lượng có thể kiểm tra | 50–54 | G55 |
| 56 | MCP điều khiển workflow đầy đủ, retry/resume an toàn | 48–55 | G56 |
| 57 | Export rõ ràng và bản cài sẵn sàng phát hành trong phạm vi đã thử | 48–56 | G57 |

Thứ tự mặc định: **48 → 49 → 50 → 51 → 52 → 53 → 54 → 55 → 56 → 57**. Có thể tách công việc độc lập sau khi hợp đồng module đã khóa; không bỏ qua gate phụ thuộc để đẩy một tính năng AI ra trước nền dựng cần thiết.

## 4. Phase 48 — Settings, session và độ tin cậy dữ liệu

**Mục tiêu:** Đóng các rủi ro nền trước khi thêm thao tác editor và tự động hóa.

**Công việc:**

1. Chụp baseline source/evidence và phân loại các thay đổi đang tích lũy theo ownership; xác định candidate có thể tái lập. Không yêu cầu commit nội dung chưa rõ nguồn gốc.
2. Tách settings của app khỏi WebView profile: đường dẫn Whisper/model, ngôn ngữ, recent projects và các tùy chọn UI được chọn rõ. Dùng native typed settings, schema version, atomic write và lỗi đọc/ghi dễ hiểu; không lưu token/secrets vào settings thông thường.
3. Nếu profile đang hoạt động còn đọc được localStorage, chỉ migrate các key cho phép sau validate. Profile cũ bị đen không được parse LevelDB, xóa hoặc tự reset; cung cấp cách nhập lại/import settings an toàn. Migration phải chạy lại được mà không ghi đè lựa chọn mới.
4. Kiểm tra tương tác giữa revision tăng sau Undo/Redo của Phase 47 với command ledger/client registry, stale AI draft, save/reopen và nhánh edit mới sau Undo. Sửa gap được chứng minh; không xóa ledger để làm test đạt.
5. Chuẩn hóa dirty/saved/recovery state và hành vi đóng/mở project khi có edit chưa lưu. Recovery không được dựa riêng vào revision bằng nhau để kết luận hai nội dung giống nhau.
6. Có chẩn đoán startup/profile và hướng dẫn khôi phục không xóa dữ liệu; ghi lỗi có thể tra cứu, tránh fallback âm thầm. Chỉ chọn cơ chế đổi profile tiếp theo khi có bằng chứng.

**Gate G48:** Settings còn sau restart và đổi WebView profile; dữ liệu settings lỗi có thể xử lý mà không làm mất project. Apply → Undo → Redo → Undo → edit mới → save/reopen/recovery đều giữ nội dung đúng và revision hợp lệ. Retry operation cũ không áp dụng lại, draft stale bị từ chối. Kiểm tra trên project chưa lưu và đã lưu; profile/recovery cũ còn nguyên. Ghi rõ phần migration không thể tự phục hồi.

**Rủi ro:** HIGH với persistence/replay protection; cần QA và review độc lập trên candidate chính xác trước đóng gate.

## 5. Phase 49 — Media library và source viewer

**Mục tiêu:** Người dùng chọn đúng đoạn nguồn trước khi đưa vào timeline.

**Công việc:**

1. Mở rộng MediaPanel hiện có với tìm kiếm, sắp xếp, lọc loại media, trạng thái online/offline và metadata cần cho dựng; bổ sung tag/bin nếu chưa có. Xác định metadata nào chỉ thuộc library và metadata nào cần persist.
2. Source viewer có play/pause, step frame, waveform/thumbnail dùng cache sẵn có, In/Out và đưa khoảng đã chọn lên timeline; không thay source range của clip đã có khi chỉ xem nguồn.
3. Batch import có tiến độ, hủy và kết quả từng file; xử lý file không hỗ trợ/trùng đường dẫn/thay đổi nội dung một cách rõ ràng. Không tự coi hai asset khác nhau là cùng media chỉ vì cùng tên.
4. Mở rộng relink thành workflow nhiều asset: đề xuất mapping theo thông tin nguồn, người dùng kiểm tra trước Apply; không tự thay thế file không chứng minh được danh tính/phạm vi tương thích.
5. Thêm collect/copy project media vào thư mục mới với preflight dung lượng, collision policy và manifest. Copy không di chuyển/xóa nguồn; chỉ cập nhật project qua transaction sau khi phần copy cần thiết thành công.

**Gate G49:** Import một thư mục có video/audio/image, file trùng tên và file lỗi; chọn In/Out đúng frame, thêm clip đúng source range; relink nhiều asset có review và rollback hợp lý. Bản project thu gom mở được sau khi nguồn cũ không truy cập được. Hủy/thiếu dung lượng không để project trỏ vào media chưa copy xong. Các file nguồn giữ nguyên hash.

**Rủi ro:** MEDIUM; collect/relink có integrity risk cần review phần persistence/path handling.

## 6. Phase 50 — Timeline phục vụ dựng hằng ngày

**Mục tiêu:** Hoàn thành phần dựng bằng chuột và bàn phím mà không phải chỉnh JSON hoặc nhờ AI cho thao tác cơ bản.

**Công việc:**

1. Rà soát split/trim/move/ripple/duplicate/multi-select/snapping/seek đã có; hoàn thiện UX, shortcut và feedback thay vì tạo thao tác trùng.
2. Thêm insert/overwrite, gap close và roll/slip edit theo lát cắt cần thiết. Khóa quy tắc overlap, source handles, track đích và ảnh hưởng transition trước khi thêm command.
3. Hoàn thiện khóa track, target track, track reorder/rename và thao tác nhiều clip. Linked selection/grouping chỉ là semantics được định nghĩa rõ; link audio/video cần tái sử dụng trong Phase 51.
4. Copy/paste trong project, marker và chỉnh thời điểm/duration bằng frame. Clipboard không được mang đường dẫn/tham chiếu thiếu sang project khác; paste liên project chỉ mở khi asset mapping đã được thiết kế.
5. Một gesture hoặc một thao tác nhóm là một transaction/history entry. Pointer state transient; edit bị từ chối phải trả clip về trạng thái đúng.

**Gate G50:** Native workflow dựng một video 60–120 giây từ ít nhất 10 clip bằng source In/Out, insert, trim, ripple, move, gap close và clipboard trong project. Undo/Redo nhóm khôi phục đúng nội dung, selection không trỏ ID đã xóa. Trường hợp track khóa, source thiếu handle, transition phụ thuộc và overlap sai đều có kết quả rõ. Save/reopen và export không đổi vị trí/source range.

**Rủi ro:** HIGH do timeline invariants và history; QA/review bắt buộc cho command mới.

**Checkpoint 2026-11-04:** Đã thêm group copy/paste, nondestructive close-gap, TrackLockSet, insert-at-playhead, overwrite-at-playhead, Alt-drag slip, Shift-drag roll, track reorder/rename controls và project markers. Marker là dữ liệu project có migration v11→v12 và thao tác command undo/redo. Roll giữ tổng thời lượng, cập nhật source range theo speed, bảo toàn offset keyframe clip-relative và từ chối source handles thiếu mà không mutate. Frontend suite 115/115, Rust Phase 50 10/10, migration test, typecheck, build và `git diff --check` đạt. Native G50 vẫn OPEN. Chi tiết trong [Phase 50 checkpoint](phase50_timeline_daily_editing.md).

## 7. Phase 51 — Audio trong video và mixer

**Mục tiêu:** Dựng được lời thoại và nhạc có kiểm soát, tránh video xuất ra khác phần nghe trước.

**Công việc:**

1. Kiểm kê và khóa contract audio nhúng trong Video item: stream được chọn, mute/gain, speed, trim và quan hệ với audio tách riêng. Không suy từ hỗ trợ standalone audio rằng video audio đã tương đương.
2. Bổ sung detach/link audio qua Command Service nếu thiếu; detach không được phát đồng thời hai bản âm thanh của cùng clip.
3. Mixer clip/track với gain dB, mute/solo và meter peak; giữ tương thích volume/fade/crossfade/keyframe hiện có. Chọn rõ canonical semantics của master gain và clipping.
4. Thêm loudness analysis/normalize theo target được chọn và ducking nhạc theo lời thoại dưới dạng edit plan có review. Bắt đầu bằng gain envelope xác định được, chưa thêm chuỗi DSP/denoise tùy ý.
5. Khóa audio-clock/seek/end-seek và resume sau offline/relink cho phạm vi engine hiện tại. Đổi native audio backend chỉ khi POC có bằng chứng cần thiết và ADR riêng.

**Gate G51:** Fixture có voice trong video, nhạc stereo, mute/solo, fade/crossfade, volume automation và clip speed. Preview có playback state đúng; export giải mã đầy đủ, kiểm tra sample count/channel layout, peak/loudness và envelope theo dung sai khóa trước. Detach/link không nhân đôi âm. Không công bố độ trễ âm/hình vật lý chỉ từ timestamp phần mềm.

**Rủi ro:** HIGH do preview/export parity và liên kết item; QA/review bắt buộc.

**Checkpoint 2026-11-04:** Schema v13 adds the default-on embedded audio switch; v14 adds default-off track solo; v15 adds unity-default track gain and dB faders. Media Library reports non-destructive mean/peak dBFS. Schema v16 adds reciprocal video/audio links, initialized to null by v15→v16 migration. Detach creates an aligned Audio item, turns off embedded audio and links both items atomically. Group move, trim, speed changes and delete keep pair semantics; split is rejected until explicit unlink. Inspector exposes unlink and `ClipLinkSet` is available through automation. Regression tests cover migration, generated H.264/AAC detach, reciprocal links, one-sided group move, mirrored speed, split rejection, and unlink. Existing command/composition/audio-analysis tests, 118 frontend tests, TypeScript, production build and `cargo check` pass. Playback peak meter code is implemented with WebView feature detection and explicit unsupported state. A reviewed deterministic dBFS normalization and voice-ducking plan now uses Command Service dry-run and revision-checked apply; native meter behavior, preview audio-clock qualification and native UI qualification remain open. Automated voice/music export acceptance passes. See [Phase 51 checkpoint](phase51_audio_mixer.md).

## 8. Phase 52 — Chỉnh hình và keyframe trực quan

**Mục tiêu:** Người dùng tự hoàn thiện bố cục, chuyển động và màu cơ bản cho bản dựng.

**Công việc:**

1. Canvas handles cho position/scale/rotate/crop, align, reset và fit/fill; dùng các command/semantics có sẵn. Thao tác trực tiếp phải có giá trị số tương ứng và một history entry khi commit.
2. Keyframe lane/editor cho các channel đã hỗ trợ, chọn/di chuyển/copy keyframe và sửa easing bằng UI. Tái sử dụng evaluator hiện tại và giữ quy tắc hidden keyframe sau trim/split.
3. Mở rộng visual effects cơ bản có giới hạn: brightness, contrast, saturation và temperature theo contract SDR đã khóa. So sánh trước/sau; chỉ thêm effect thực sự thiếu trong enum/preset hiện tại.
4. Định nghĩa thứ tự crop → transform → effect → opacity/composite và cách tương tác với transition. Lưu tham số effect thành domain data; không persist FFmpeg graph.
5. Lát cắt 52A cho canvas/keyframe phải qua gate trước 52B cho màu/effect mới. LUT/HDR/mask/tracking nằm ngoài phase.

**Gate G52:** Native chỉnh canvas và animation trên ít nhất hai lớp video/image/text; Undo/Redo và save/reopen giữ giá trị. So sánh preview/export tại keyframe, giữa easing và cut theo fixture/tolerance đã chọn trước; effect bypass/reset khôi phục kết quả gốc. Các hiệu ứng chưa đạt parity không được bật cho bản phát hành.

**Rủi ro:** HIGH khi thêm composition semantics; QA/review cho renderer và migration.

## 9. Phase 53 — Phụ đề, transcript và dựng theo văn bản

**Mục tiêu:** Chỉnh video có lời nói nhanh bằng văn bản và hoàn thiện phụ đề tiếng Việt/Anh.

**Công việc:**

1. Editor dạng danh sách cho caption đã có: sửa chữ, thời gian, split/merge, tìm/thay thế, báo overlap/đoạn trống và áp style cả track. Giữ template/rich text hiện tại.
2. Hiển thị transcript gắn source time, tìm câu và nhảy tới media/timeline liên quan; phân biệt transcript nguồn với caption đã sửa trên timeline.
3. Auto-caption chạy local có progress/cancel, chọn model/ngôn ngữ và cache invalidation theo settings/source. Word timing chỉ được dùng khi provider thật sự trả mốc đủ tin cậy.
4. Dựng theo văn bản: chọn đoạn transcript để tạo range-based rough cut, hoặc đề xuất cắt khoảng lặng; preview rõ phần clip/audio/caption bị ảnh hưởng trước Apply.
5. Import/export SRT/WebVTT xử lý Unicode, xuống dòng, offset và frame-rate mapping; preset caption bảo đảm safe zone/readability cho các canvas mục tiêu.

**Gate G53:** Fixture tiếng Việt có dấu và tiếng Anh; sửa/split/merge/reorder caption rồi Undo/Redo/save/reopen. SRT/WebVTT round-trip giữ text và thời điểm trong dung sai một frame sau quy tắc quantize đã công bố. Dựng từ transcript giữ source ranges đúng; hủy analysis/preview không đổi project. Model nhận sai phải sửa được và không mất phần người dùng đã xác nhận.

**Rủi ro:** MEDIUM; text-based timeline edits có risk HIGH và dùng gate command/history.

## 10. Phase 54 — Preview, proxy và ngân sách tài nguyên

**Trạng thái hiện tại (2026-11-04):** Đã chốt host/toolchain và fixture H.264/AAC 30 giây/900 frame. FFmpeg full decode 5 lần: 399–431 ms. Đã sinh hai project thử tổng hợp, không sửa nguồn: 10 phút/100 clip/2 video layer/3 audio track và 30 phút/300 clip/2 video layer/3 audio track; CLI inspect đạt 5/5 ở cả hai, median 23.61 ms và 46.60 ms (đo parse/validate/inspect, không phải render hoặc seek UI). Thêm `Media cache` status/clear cho sáu thư mục derived; clear chỉ xóa các category được quản lý, giữ file khác trong root, và từ chối khi instance hiện tại có media/export job chạy. Bảy test cache, native `cargo check`, frontend suite 148/148, typecheck, production build và `git diff --check` đạt. G54 vẫn chưa đạt: seek/UI p95, dropped frames, playback CPU/GPU, proxy regeneration và 30-cycle process-tree RAM chưa đo. Lượt UIA hiện tại không mở được workload: common file picker không cung cấp filename edit control; SendKeys làm rơi ký tự khỏi đường dẫn, sau đó dialog đã được đóng. Không file workload nào bị sửa. Computer Use không có trong toolset; native capture/UIA chỉ được dùng khi hai nguồn quan sát đồng nhất. Bằng chứng workload: `artifacts/phase54/phase54_baseline_20261104.json` và `artifacts/phase54/phase54_workload_fixture_summary.json`.

**Mục tiêu:** Dựng được workload công bố mà không treo UI hoặc làm tăng bộ nhớ không giới hạn.

**Công việc:**

1. Đo baseline decode/seek/playback, RAM/process tree, CPU/GPU, temp disk và queue trên máy tham chiếu; ghi phiên bản phần cứng/driver/toolchain. Khóa budget trước tối ưu.
2. Mở rộng proxy/cache hiện có: policy auto/manual, progress/cancel, chọn chất lượng, cache status/size và clear derived data an toàn. Canonical project luôn trỏ media gốc; export dùng nguồn gốc theo policy rõ.
3. Giới hạn decoder/media element/analysis concurrency, queue/backpressure và cache eviction. Superseded seek không được xuất frame cũ hoặc tiếp tục tiêu tài nguyên sau cancel.
4. Tối ưu timeline dài/nhiều item bằng virtualization và thumbnail/waveform theo vùng nhìn thấy; không thay canonical time để đổi lấy tốc độ.
5. Kiểm tra workload: 30 giây tham chiếu; 10 phút/100 clip/2 video layer/3 audio track; 30 phút/300 clip; thêm 4K đầu vào qua proxy như một gate có điều kiện. Nếu không đạt, giảm phạm vi công bố bằng quyết định rõ và ghi số đo, không sửa ngưỡng sau test để tạo PASS.

**Gate G54:** Trên workload chính, seek p95 ≤ 500 ms khi proxy đã sẵn sàng; thao tác UI chính phản hồi p95 ≤ 100 ms theo phép đo đã định nghĩa; RAM dưới budget đã khóa và không tăng liên tục qua 30 vòng seek/play/cancel. Proxy xóa được và tạo lại; offline source phải hiển thị rõ, không âm thầm dùng proxy để export thiếu nguồn. Báo riêng workload nào chưa đạt.

**Rủi ro:** MEDIUM/HIGH theo backend bị đổi; profile bằng chứng trước khi tối ưu.

## 11. Phase 55 — AI tạo bản dựng theo nội dung và thời lượng

**Mục tiêu:** Từ brief tạo rough cut có source ranges, nhạc và caption; người dùng biết AI đã dựa vào dữ liệu nào.

**Công việc:**

1. Tái sử dụng scene/transcript analysis và planner 41–46; thêm brief có cấu trúc: thời lượng, canvas, chủ đề, media cho phép, đoạn phải giữ/bỏ và style/template.
2. Kết hợp scene caption và transcript theo provenance/source fingerprint; metadata, heuristic và suy đoán model phải được phân biệt. Không coi caption của model là sự thật đã xác nhận.
3. Khắc phục giới hạn Phase 46 chỉ giữ nguyên scene interval: planner chọn thứ tự và khoảng nguồn có thể trim trong giới hạn cho phép. Deterministic scheduler tính frame count/handles và giải constraint thời lượng sau output model.
4. Chọn chế độ duration `exact` hoặc `advisory`. Exact phải đạt số frame đích hoặc trả lỗi constraint không thể đáp ứng; không tự lặp cảnh, đổi speed hay cắt đoạn bắt buộc giữ nếu brief chưa cho phép.
5. Review có timeline draft, lý do chọn/bỏ, điểm cắt, caption/audio dự kiến và độ lệch thời lượng do core tính. Người dùng sửa selection/range trước Apply; supporting audio/ducking dùng contract Phase 51.
6. Cancellation, provider timeout, partial/invalid output và source/project thay đổi đều fail rõ; request chỉ gửi dữ liệu được cho phép. Giữ local-first; provider ngoài là lựa chọn riêng, không tự gửi media lên mạng.

**Gate G55:** Corpus tối thiểu 12 brief gồm tiếng Việt/Anh, thiếu media, exact/advisory, must-keep, silent video và model trả ID/range sai. Core bảo đảm invariant/range/duration; exact đạt số frame đích hoặc từ chối có lý do. Đánh giá biên tập bằng rubric đã khóa cho relevance, continuity và readability; tách điểm chất lượng AI khỏi pass kỹ thuật. Native review → sửa → Apply → Undo/Redo → save/reopen/export đạt trên ít nhất ba project riêng.

**Rủi ro:** HIGH tại planner/application boundary; không cho model thực thi shell hoặc reducer riêng.

## 12. Phase 56 — MCP hoàn chỉnh và workflow tự động có thể tiếp tục

**Mục tiêu:** AI client hoặc script dựng/xuất nhiều project có thể theo dõi, retry và sửa lỗi mà không mất dữ liệu.

**Công việc:**

1. Rà soát tool discovery/schema/examples theo capability thực tế sau 48–55. Bổ sung khoảng thiếu cho media, source ranges, captions, mixer, keyframes, analysis, draft và export; UI/MCP parity kiểm tra theo workflow thay vì chỉ đếm tool.
2. Khóa ownership giữa project đã lưu trên disk và project có edit chưa lưu trong UI. Nếu thêm điều khiển session UI đang mở, phải có ADR cho broker/session ID/lease/revision; tiếp tục gọi Command Service hiện hữu. Chưa có broker thì trả lỗi rõ, không tự ghi đè file và reload UI.
3. Workflow manifest gồm input fingerprints, brief/plan version, revision, structured operation identities và artifact paths. Resume kiểm tra lại preconditions; không lưu credentials hoặc nội dung media thừa vào log.
4. Job status/progress/cancel có correlation rõ; xác định handle nào chỉ sống trong process. Nếu muốn job sống qua MCP disconnect/restart, thiết kế persistence/reconciliation và ownership trước khi cung cấp tính năng đó; không hứa resume từ một jobId cũ không còn hợp lệ.
5. Batch workflow có giới hạn concurrency, output collision policy và error per-project; retry lost-response không nhân đôi edit hoặc ghi đè output đã tồn tại.
6. Cung cấp recipe/script tham chiếu: folder → inspect/analyze → plan/review policy → apply/save → render/validate. Automation tự chạy chỉ trong quyền và phạm vi người dùng đã cấu hình; publish/send ra ngoài vẫn cần quyền tương ứng.

**Gate G56:** Client thật qua stdio thực hiện ba workflow (dựng từ brief, sửa project có sẵn, batch ba project). Test mất response/retry, MCP EOF/restart, stale revision, unsaved UI conflict, cancel và output collision. Số clip/track và durable ledger không bị nhân đôi; phần fail có thể resume hoặc được báo rõ không hỗ trợ. Artifact cuối mở được trong native UI và qua ffprobe/full decode.

**Rủi ro:** HIGH do concurrency/ownership/replay; QA, code review và ADR cho ranh giới session mới.

## 13. Phase 57 — Export, bản cài và nghiệm thu sản phẩm

**Mục tiêu:** Người dùng cài app, dựng và nhận file đầu ra đáng tin cậy với thông báo lỗi đủ để xử lý.

**Công việc:**

1. Export preflight thống nhất: media offline, font thiếu, feature không hỗ trợ, canvas/rate, source handles, output tồn tại, disk space và codec capability; dùng snapshot project rõ ràng.
2. Hoàn thiện export presets và render queue local có giới hạn, tiến độ/cancel và báo lỗi từng job; tái sử dụng Job Manager và semantics Phase 56. Queue persistence chỉ thêm khi contract resume đã chốt; không tự đánh dấu file partial là completed.
3. Postflight ffprobe/full decode cho fixture nghiệm thu; manifest liên kết project revision, profile export, toolchain và output. Không áp full decode bắt buộc lên mọi export của người dùng nếu chưa đo chi phí và có lựa chọn phù hợp.
4. Build NSIS candidate có provenance/version; kiểm tra cài/upgrade/launch/uninstall trong thư mục thử được cô lập. Uninstall giữ project và có chính sách settings/cache/recovery rõ; không tự xóa dữ liệu cũ để làm candidate chạy được.
5. Hoàn thiện onboarding, shortcut/help, empty/error states, keyboard navigation/focus và diagnostics export có thể xem trước/redact. Hướng dẫn MCP/local AI nêu đúng model/media cần thiết và giới hạn đã đo.
6. Chốt supported-format/workload matrix và known limitations; xử lý tất cả lỗi data-loss/crash/blocker thuộc phạm vi. Mỗi candidate thay đổi sau review phải được kiểm tra lại phần liên quan.

**Gate G57:** Trên candidate bản cài chính xác, chạy ba kịch bản hoàn chỉnh: dựng thủ công, AI draft rồi sửa tay, và MCP batch. Từng kịch bản phải có save/reopen, recovery, history, preview và export; file cuối đúng dimensions/rate/duration/audio contract và full decode đạt. Có test hủy export, thiếu media, disk error mô phỏng và upgrade giữ project/settings. Candidate đạt QA/review/final review theo policy, không còn lỗi nghiêm trọng thuộc supported scope. Clean VM, installed hardware encoder và physical A/V chỉ bổ sung nếu muốn công bố các claim đó.

**Rủi ro:** HIGH tại release gate; đóng gate trên exact candidate, không lấy build cũ làm bằng chứng cho source mới.

## 14. Các mốc kiểm tra sản phẩm

| Mốc | Phase phải đạt | Demo bắt buộc |
|---|---|---|
| M1 — Dựng thủ công và dữ liệu an toàn | 48–51 | Dựng video có lời thoại/nhạc, đóng/mở, phục hồi và xuất |
| M2 — Hoàn thiện nội dung | 52–54 | Chỉnh animation/hình/caption và dựng workload dài trong budget |
| M3 — Tự động hóa có kiểm soát | 55–56 | Brief → project có thể chỉnh, AI/MCP retry/resume không nhân đôi edit |
| M4 — Bản phát hành | 57 | Bản cài thực hiện cả ba workflow, có evidence và giới hạn công bố |

Một phase có thể chia thành nhiều task nhỏ và commit theo ownership đã xác định. Chỉ mở phase phụ thuộc sau khi gate tiền đề đạt; nếu chặn bởi môi trường, ghi đúng phần blocked và tiếp tục phần độc lập có ích. Không tự tạo phase mới chỉ để kéo dài roadmap.

## 15. Quyết định mặc định và điều kiện cần hỏi lại

Mặc định giữ Windows local-first, ưu tiên Việt/Anh và workflow video nội dung/short-form; không mua model/dịch vụ, không upload media tự động. Không đặt lịch hoàn thành theo ngày khi chưa đo effort trên baseline; các phase 48, 50–52 và 56 có rủi ro lớn hơn về dữ liệu/semantics.

Chỉ cần quyết định sản phẩm mới khi có thay đổi đáng kể: dịch vụ AI có phí hoặc upload ngoài máy; nâng mục tiêu sang HDR/4K output/60 fps/long-form chuyên nghiệp; hỗ trợ remote/multi-user; hoặc cần migration phá tương thích. Lựa chọn triển khai thông thường được xử lý trong phạm vi gate đã chốt.

**Checkpoint hiện tại (2026-11-04):** G54 có native seek samples đã ghi nhận nhưng vẫn có timeout (25/30 đạt trong lượt readback-gated), và timeline preview offline dù Source Viewer đọc được cùng media; gate G54 còn mở. G55 planner provenance và bounded-input tests pass, nhưng rubric sample hiện tại 10/15 cần thêm brief/project và independent reviewer. G56 stdio workflow, revision guard, batch/cancel/collision và 3 MP4 outputs pass tự động; persistent copies/hash/ffprobe/full-decode ở `artifacts/phase56/verified_exports/`; native conflict/Save As/reload và mở artifact vẫn chưa xác minh. G57 installed MCP, preflight, cancellation, promotion-failure preservation, NSIS và same-version reinstall pass; exact-candidate UI, authentic cross-version installer và literal disk-full test còn mở. Tổng hợp gate cuối: [phase48_57_final_qualification_blockers.md](phase48_57_final_qualification_blockers.md). Phase 26 physical A/V, clean VM, installed hardware encoder là tùy chọn nếu không công bố claim tương ứng.

## Consolidated final blocker pass — 2026-11-04

Implementation across Phases 48–57 has progressed; this is a verification checkpoint, not a declaration that the product gates passed. The remaining work is grouped here so independent automated/MCP/release checks can continue while native UI limitations are handled together.

| Gate | Verified evidence retained | Remaining acceptance / blocker |
|---|---|---|
| G48 settings/session/data | Command, persistence and basic native history/recovery smoke are recorded in `Plans/phase48_settings_session_data_reliability.md`. | Native settings persistence across restart/profile and saved-project/recovery coexistence still need a controlled native run. Do not alter existing recovery copies. |
| G49 media/source viewer | Automated media library/collect tests, Phase 49 import/preview/timeline/export smoke, and native import → Source Viewer playback → Cancel are recorded in `Plans/phase49_media_library_source_viewer.md` and `artifacts/phase49/`. A screenshot-backed exact file-row selection imported the disposable video; Source Viewer rendered/played to end; ranges 0–15 s and 0–5.5 s committed to Video Track 1, with the partial range shown as 165 frames at 30 fps. Undo/Redo/final Undo removed/restored/removed clips; source hash stayed unchanged. | One video source preview and one partial-range quantization case are qualified. Folder batch import/cancel, frame-accurate source scrubbing, relink/collect, path control and save/reopen remain open. |
| G50 timeline | Native 12-clip/60-second save/reopen/export and close-gap/history pass. A separate keyboard trim-start + Undo pass and two-clip group nudge (both clips moved 1 frame together) are now verified by UIA. Evidence: `artifacts/phase50/phase50_native_12clip_workflow_20261105.json` and `phase50_native_keyboard_group_move_20261105.json`. | G50 remains OPEN: positive slip/roll needs a source with unused handles; pointer multi-select drag and transition interaction remain unqualified. The current full-length 15-second duplicate pair correctly rejected roll beyond media end. |
| G51 audio | Embedded-audio toggle/detach/history and automated voice/music export pass. Native UIA now confirmed Play/Pause and playhead advance during a 4-second WDM-KS Stereo Mix capture; signal peak rose 38.26 dB over a measured paused baseline. Evidence: `artifacts/phase51/phase51_confirmed_playback_loopback_20261105.json`. | OPEN: capture is system-wide, the internal meter remained −60 dBFS, audio-clock synchronization and linked-clip behavior are unqualified, and the later UI snapshot returned to frame 0. |
| G52 visual canvas | Native transforms/keyframes, save/reopen, export/full-decode and one color reset/history case pass. The animated source-aware Fit/overlay mismatch was fixed and the revision-28 export frame qualitatively matches its saved pre-fix native preview. | OPEN: updated-runtime native recheck could not open the fixture; UIA returned `Target element cannot receive focus` and exact filename `WM_SETTEXT` had no readback. Easing/cut, color bypass/reset render parity and broader layers remain. Evidence: `Plans/phase52_visual_canvas_keyframes.md`. |
| G53 captions/transcript | SRT import/editor display and transcript command/parser tests are recorded in `Plans/phase53_caption_transcript_editor.md`. Native caption edit/Apply/Undo/Redo, save and H.264/AAC export/full decode pass on a disposable fixture. | OPEN: the reproduced legacy square-background caption mismatch now has a preview raster-path candidate with passing automated coverage; updated-runtime native frame-60 confirmation is blocked because the reliable native Computer Use control is unavailable and current exact fixture input is unverified. Dirty-draft cancel and native timing-edit workflow also remain open. Evidence: `artifacts/phase53/phase53_caption_native_qualification.json`, `artifacts/phase53/disposable/recheck_20261105/native_recheck_evidence.json`, and `Plans/phase53_caption_transcript_editor.md`. |
| G54 preview/resources | CLI inspect passes 5/5 for 10m/100-clip and 30m/300-clip fixtures; decode reference 399–431ms. Historical seek runs are separate (29/30 and 25/30; readback-gated p95 2556.53ms). Fresh native UIA run on a separate-ID 100-clip copy visually renders two layers at frame 1 (17.9ms overlay) and a different scene at frame 360; frame 0 reports Empty Track Interval and a readback-confirmed frame-1800 seek returns to frame 360 after 3s. Source scan found maximum keyframe gap 5.033s; FFmpeg decodes the 60s target and Rust plan/sample p95 is 172/51µs, narrowing the unresolved area to WebView/media seek or presentation without identifying the cause. | G54 remains OPEN / NOT PASSED: seek reliability/500ms p95 is not met; frame drops, proxy/GPU, calibrated audio/meter, and full process-tree 30-cycle memory are unmeasured. Evidence: `artifacts/phase54/phase54_native_uia_retest_20261105.json`, `artifacts/phase54/phase54_source_gop_seek_diagnostic_20261105.json`; details in `Plans/phase54_preview_performance.md`. |
| G55 AI edit quality | 12-brief deterministic corpus, provenance boundary audit, privacy/allowlist/range/duration checks, local-model suggestion, native preview/apply/history, one editorial rubric review, and an exhaustive 16,329-case exact-duration scheduler test are recorded in `Plans/phase55_content_duration_edit_plans.md` and `artifacts/phase55/`. | Three distinct project reviews and an independent second reviewer are still required for the quality gate. Current semantic source material is one real project; synthetic fixtures cannot substitute for independent editorial examples. Native save/reopen/export for three projects remains open. |
| G56 MCP workflow | Three-project stdio batch, checkpoint resume, stale revision, collision preservation, export/cancel and artifact decode evidence are in `artifacts/phase56/`. Three latest MP4 exports were re-hashed, ffprobed and fully decoded; persistent copies are in `artifacts/phase56/verified_exports/`. Native stale-save conflict, preserved recovery, Save a Copy and reopening the copy are verified; G56 PASS. | — |
| G57 release | Latest exact-source NSIS candidate passes Core Healthy, native manual/AI Save/reopen/export, MCP AI/export/full-decode, recovery preservation, missing-media/relink, cancellation cleanup and locked-destination error presentation. | OPEN only for cross-version project/settings preservation against an authentic earlier installer; none is available. Literal ENOSPC is not required. Evidence: `artifacts/phase57/phase57_latest_candidate_ai_workflow_evidence.json` and `artifacts/phase57/phase57_latest_candidate_native_export_error_ui_20261105.json`. |

The current toolset does not expose Computer Use. Read-only Windows UI Automation and screenshots identify the isolated TinixCut window and support native controls. The common file picker has no filename ValuePattern and one SendKeys attempt corrupted the filename before Open; a later screenshot-backed exact file-row selection succeeded for the visible disposable fixture. Do not repeat unreliable typing. This does not qualify navigation or selection for other files; batch import, relink and collect remain open. Continue only with actions whose target is confirmed by UIA and a current screenshot.

Runtime snapshot at this checkpoint: workspace debug TinixCut PID 33680 and isolated candidate PID 31756 are both present; `http://localhost:1420` returned HTTP 200. Paseo was left running. No app code changed in this documentation-only checkpoint, so restart was not applicable. `git diff --check` passed. The working tree has broad pre-existing changes; no commit was created to avoid including unrelated phase work.
### Current-worktree regression verification — 2026-11-04

The accumulated candidate passed the MCP integration suite (**49 passed, 0 failed, 3 ignored**), frontend typecheck, **150/150** frontend tests, production build, `cargo fmt -- --check`, and `git diff --check`. Build emitted the existing bundle-size advisory for a >500 kB chunk. MCP tests ran with isolated `CARGO_TARGET_DIR=%TEMP%\tinixcut-phase50-57-final-target`, so no running TinixCut binary was replaced. This refreshes automated evidence for G56/G57 but does not close any native/installed UI or external-review gate listed above.
### Cross-process and export-error checks — 2026-11-04

- The current-source MCP integration suite is **49 passed, 0 failed, 3 ignored**; cross-process stale-writer integration is **2/2 passed**; hardware-error discrimination is **1/1 passed**, including no-space errors being excluded from encoder fallback. Frontend typecheck, **150/150** tests, build, Rust format and `git diff --check` pass.
- These checks confirm saved-project stale-writer rejection and error classification, but do not close G56 native unsaved-project ownership or G57 real write-failure/install UI behavior.
### Phase 55 implementation continuation — optional AI title suggestion (2026-11-04)

Closed the implementation gap for model-generated opening-title text: it is opt-in, privacy-bounded, length/control-character validated, and shown as an editable suggestion. Copying into the draft and selecting a template are explicit user actions; scene suggestion remains read-only. Evidence: [title suggestion](../artifacts/phase55/phase55_title_suggestion_evidence.json) and [Phase 55 details](phase55_content_duration_edit_plans.md). Automated checks pass; native title/UI quality remains in the consolidated final blocker pass.
Live local-model evidence now confirms the opt-in title path: Gemma returned “Hoi An Morning Serenity” without project mutation. It selected 10 seconds against an 8-second target while rationale incorrectly claimed 15 seconds; the exact duration is independently computed and shown by the review UI. This reinforces the requirement to treat model prose as untrusted. See `artifacts/phase55/phase55_title_suggestion_evidence.json`.

### Sequential gate disposition — 2026-11-05

This is a handoff checkpoint, not a declaration that Phases 48–57 are complete.

| Gate | Disposition now | Remaining concrete acceptance |
|---|---|---|
| G54 | **NOT PASSED / BLOCKED** | Frames 1 and 360 render on the 10m/100-clip copy, but frame 0 is empty; frame 1800 times out and returns to frame 360. Prior readback-gated seek p95 2556.53 ms exceeds 500 ms. Audio, frame drops, proxy/GPU and full process-tree memory remain unmeasured. |
| G55 | **OPEN / EXTERNAL HUMAN INPUT** | Deterministic 12-brief corpus, privacy/provenance and local-model checks pass; one native synthetic-project review/apply/history/save/reopen/export passes. Workspace sample audit found only one underlying real Hoi An media corpus: landscape/portrait projects share media hashes and the 15 s video variants are identical. Do not count them as independent samples. Two additional approved real project/brief samples, three-project rubric scoring and an independent qualified reviewer remain. Inventory: `artifacts/phase55/phase55_sample_availability_audit_20261105.json`.
| G56 | **PASS** | MCP workflow/persistence and native artifact viewing pass. Native stale-save conflict preserved the newer disk revision and unsaved recovery copy; the conditional **Save a Copy…** action wrote a separate disposable file, and TinixCut reopened it at revision 3. Evidence: `phase56_native_conflict_and_restart_evidence_20261105.json`. |
| G57 | **OPEN / EXTERNAL ARTIFACT BLOCKER** | Latest-source installer SHA `E0C686E088000B8D5587633011A4E3F96AF50901CD84D58B1512E1485B46801D` passes current-candidate native manual/AI/save-reopen/export, MCP AI/export/status/full-decode, recovery preservation, missing-media/relink, cancellation, and locked-destination error UI. Exact evidence: `artifacts/phase57/phase57_latest_candidate_ai_workflow_evidence.json` and `artifacts/phase57/phase57_latest_candidate_native_export_error_ui_20261105.json`. Only authentic prior-version upgrade/data-preservation remains open; no genuine earlier installer is available. Literal ENOSPC is not a gate.

Phase 26 physical A/V capture, clean VM, and installed hardware encoder stay optional claim-specific qualification per the Product Owner's prior decision. Continue G55, G56, and G57 independently; collect external reviewer/material and authentic prior-installer requests for one final blocker pass.

### Current-worktree automated regression recheck — 2026-11-05

- Frontend TypeScript and Vitest: **30 files / 168 tests passed**. This exercises the current timeline editing, audio, canvas, caption, preview, and edit-plan frontend modules; it does not establish native pointer gestures or WebView rendering.
- G50 timeline Command Service regressions: **10 passed** (close gap, insert/overwrite, slip/roll, track locking, markers and transaction/history behavior).
- G51 audio Command Service and analysis regressions: **8 passed**; native FFmpeg voice/music export acceptance: **1 passed**.
- G52 color/crop Command Service and shared-SDR-matrix regressions: **3 passed**.
- G53 SRT/VTT and caption/transcription regressions: **9 passed**; transcript source-time mapping: **1 passed**.
- Phase 30 shared text renderer acceptance: **11 passed, 1 ignored**, including the legacy-caption preview/MP4 pixel regression.
- Phase 32 layered-template/canvas-adjacent acceptance: **20 passed, 1 ignored**; this does not replace the Phase 52 color/keyframe native parity cases.
- Phase 55 structured and bilingual brief corpora: **2 passed**.
- MCP acceptance covering the current Phase 56/57 automation workflow: **49 passed, 0 failed, 3 ignored**; ignored cases require an explicitly started local LM Studio model.
- Rust format check, frontend production build and `git diff --check`: **PASS**. The build retains the existing 575 kB chunk advisory.
- The first isolated Cargo target ran out of space on C: (`os error 112`). Only that task-created target was removed after verifying its exact path; the retry used the workspace's D: target and all listed Rust checks passed. No app/runtime process was stopped.
- These automated checks confirm the current implementation paths only. G50–G54 native behavior, G55 independent human/sample evidence, and G57 authentic cross-version upgrade remain open as stated in the final blocker register.

### G56 native stale-save preservation — 2026-11-05

The native UI edited a separate-ID disposable project while a second stdio MCP process advanced the same disk file. Native Save displayed the expected-revision conflict, left the newer file hash unchanged, and kept the native unsaved revision in its recovery snapshot. The conditional **Save a Copy…** action opened the system dialog, wrote a new recovered project file, and TinixCut reopened that exact file at revision 3 with Core Healthy and a rendered preview. The conflict source and prior Phase 54 recovery evidence remained unchanged. **G56 PASS.** Exact hashes and screenshots are in `phase56_mcp_edit_plan_duration_parity.md` and `artifacts/phase56/`.
### G54 evidence refinement — 2026-11-05

A fresh native UIA run on a unique-ID copy now visually verifies two decoded timeline states: frame 1 (two layers; 17.9 ms overlay) and frame 360 (different scene at 12 seconds). Frame 0 showed an empty interval; a readback-confirmed seek to frame 1800 failed to leave frame 360 within 3 seconds. This supersedes the broad “timeline preview remains offline” statement for this copy, but does not close G54: the prior controlled 25/30 seek run has p95 2556.53 ms and exact decoded-frame/drop/resource coverage is incomplete. Canonical/source hashes stayed unchanged. Full sample detail: `artifacts/phase54/phase54_native_uia_retest_20261105.json`.
