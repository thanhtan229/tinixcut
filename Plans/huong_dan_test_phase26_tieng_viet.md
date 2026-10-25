# Hướng dẫn kiểm thử thủ công Phase 26 (Tiếng Việt)

> **Mục đích:** kiểm định cuối cùng các chức năng đã triển khai trong TinixCut bằng một lượt test thủ công có thể lặp lại.
>
> **Thời điểm thực hiện:** sau khi mọi thay đổi mã nguồn đã hoàn tất và ứng dụng TinixCut đã được khởi động lại.
>
> **Cách ghi nhận:** với mỗi mục, ghi `PASS`, `FAIL` hoặc `BLOCKED`. Khi `FAIL`, ghi đúng thao tác, kết quả nhìn/nghe được và nếu có, ảnh chụp màn hình hoặc tệp đầu ra.

Tài liệu kiểm định gốc bằng tiếng Anh: [phase26_manual_qualification.md](phase26_manual_qualification.md).

## 1. Chuẩn bị một lần

1. Mở TinixCut, chọn **+ New** để tạo project mới.
2. Nhấn **+ Import** và chuẩn bị ít nhất các tệp sau:
   - Một video dài từ 15 giây trở lên, có cả hình và tiếng.
   - Một audio độc lập dài từ 10 giây trở lên.
   - Một ảnh.
   - Một tệp phụ đề ngắn định dạng `.srt` hoặc `.vtt` (nếu có).
3. Kéo video vào Video track, audio vào Audio track và ảnh vào Video track. Sắp xếp để project dài ít nhất 12 giây.
4. Nhấn **💾 Save** và lưu thành một project thử nghiệm trong thư mục riêng. Không dùng project quan trọng vì phần relink sẽ đổi tên hoặc di chuyển một tệp nguồn.
5. Nếu test Auto captions, chuẩn bị sẵn đường dẫn tuyệt đối đến `whisper-cli.exe` và model `.bin` tương thích trên máy. Hai tệp này không được TinixCut cài sẵn.
6. Bật loa hoặc tai nghe. Đặt âm lượng hệ thống ở mức có thể nghe rõ.

**Quy ước kết quả:**

- `PASS`: kết quả khớp phần “Kỳ vọng”.
- `FAIL`: khác phần “Kỳ vọng”, bị treo, màn hình đen, mất âm thanh hoặc hiện lỗi không rõ nguyên nhân.
- `BLOCKED`: chỉ dùng cho Auto captions khi chưa có cả `whisper-cli.exe` lẫn model `.bin` tương thích. Các mục khác không dùng `BLOCKED`.

---

## 2. Canvas, bố cục social và text template

### 2.1 Đổi tỉ lệ canvas

1. Trên thanh trên cùng, mở hộp chọn **Canvas**.
2. Lần lượt chọn **16:9**, **9:16**, **1:1** và **4:5**.
3. Mỗi lần chọn, quan sát khung Preview và nhãn kích thước ở Preview.
4. Với ít nhất một tỉ lệ dọc, xuất một đoạn ngắn theo mục 7 để xác nhận kích thước đầu ra.

**Kỳ vọng:** Preview đổi đúng khung hình cho từng tỉ lệ; tỉ lệ và kích thước export mặc định đi theo canvas hiện tại.

### 2.2 Fit, Fill và màu nền

1. Chọn video trên Timeline.
2. Ở **Inspector** bên phải, tìm phần bố cục/placement của clip.
3. Chọn **Fit**, quan sát toàn bộ ảnh video và phần nền thừa.
4. Chọn **Fill**, quan sát video phủ kín canvas; phần dư có thể bị crop theo tỉ lệ canvas.
5. Trên TopBar, đổi ô màu cạnh nhãn **Canvas** sang một màu dễ nhận biết, ví dụ đỏ đậm hoặc xanh lam.
6. Phát hoặc seek đến một thời điểm có thể nhìn rõ phần nền.

**Kỳ vọng:** Fit và Fill tạo bố cục đúng; màu nền được hiển thị tại vùng không có nội dung video và xuất hiện trong video export khi có vùng nền.

### 2.3 Text template, chỉnh sửa và Undo/Redo

1. Nhấn **+ Text** trên TopBar để thêm text item.
2. Chọn text item mới trên Timeline.
3. Trong **Inspector**, mở phần **TEXT TEMPLATES** và lần lượt áp dụng các template có sẵn.
4. Với mỗi template, sửa nội dung chữ và một thuộc tính style nhìn thấy được, ví dụ màu, cỡ chữ hoặc font.
5. Nhấn **↩ Undo** một lần rồi **↪ Redo** một lần sau mỗi thay đổi đáng kể.
6. Nhấn **💾 Save**, đóng/mở lại project hoặc dùng **📂 Open** để mở lại chính tệp `.tinix` vừa lưu.

**Kỳ vọng:** Text vẫn chỉnh sửa được sau khi áp dụng template; Undo/Redo khôi phục chính xác trạng thái trước/sau; canvas và thuộc tính template/text được giữ sau khi mở lại.

---

## 3. Import, chỉnh sửa và export phụ đề thủ công

### 3.1 Import SRT/VTT

1. Nhấn **💬 Import captions** trên TopBar.
2. Chọn tệp `.srt` hoặc `.vtt` ngắn đã chuẩn bị.
3. Kiểm tra Timeline có một text/caption track riêng và nhiều cue tương ứng.
4. Chọn một cue, sửa nội dung trong Inspector, sau đó phát đến thời điểm cue xuất hiện.

**Kỳ vọng:** Một track phụ đề chuyên dụng được tạo; mỗi cue có thể chọn, sửa và hiện đúng lúc trên Preview.

### 3.2 Áp dụng style/template cho track phụ đề

1. Chọn một cue thuộc caption track.
2. Trong Inspector, áp dụng một text template hoặc thay đổi style cho caption track.
3. Nếu có nhiều text track, chọn một track khác và xác nhận nó không bị thay đổi.

**Kỳ vọng:** Chỉ caption track đang chọn thay đổi. Các track text khác giữ nguyên.

### 3.3 Export và import lại

1. Nhấn **📝 Export captions** trên TopBar.
2. Lưu tệp phụ đề xuất ra vào thư mục test.
3. Tạo project bản sao hoặc xóa caption track vừa import trong project test.
4. Nhấn **💬 Import captions** rồi chọn đúng tệp vừa export.
5. So sánh ít nhất ba cue: nội dung và thời điểm bắt đầu/kết thúc.

**Kỳ vọng:** Nội dung và timing cue được giữ khi export rồi import lại.

---

## 4. Auto captions (chỉ khi có Whisper CLI và model)

### 4.1 Mở đúng điều kiện

1. Chọn một video hoặc audio clip có tiếng trên Timeline.
2. Nhấn **✨ Auto captions** trên TopBar.

**Kỳ vọng:** Hộp thoại **Auto captions** mở ra. Nếu chưa chọn clip hoặc chọn clip không có audio, ứng dụng phải từ chối rõ ràng và không tạo dữ liệu mới.

### 4.2 Tạo phụ đề tự động

1. Trong hộp thoại, dùng nút chọn tệp hoặc dán đường dẫn đến `whisper-cli.exe`.
2. Chọn model `.bin` tương thích.
3. Chọn ngôn ngữ **Auto**, **Vietnamese** hoặc **English** phù hợp với clip.
4. Nhấn **Generate captions**.
5. Chờ trạng thái tiến trình hoàn tất; không thao tác thay đổi clip đang xử lý.
6. Kiểm tra Timeline có track tên **Auto Captions**.
7. Phát từ đầu clip; kiểm tra câu chữ xuất hiện trong phần clip được chọn, không nằm trước điểm trim hoặc vượt quá phần clip hữu dụng.

**Kỳ vọng:** Tiến trình hiển thị rõ ràng, hoàn tất không lỗi và tạo một `Auto Captions` text track. Timing phụ đề tuân theo trim và tốc độ clip.

### 4.3 Undo/Redo kết quả tạo tự động

1. Sau khi tạo thành công, nhấn **↩ Undo** một lần.
2. Xác nhận toàn bộ Auto Captions track/cue biến mất cùng lúc.
3. Nhấn **↪ Redo** một lần.

**Kỳ vọng:** Toàn bộ kết quả được xóa và khôi phục trong một giao dịch, không còn cue rời hoặc track trống ngoài ý muốn.

### 4.4 Hủy job

1. Chọn clip dài hơn, mở **✨ Auto captions** và khởi chạy lại.
2. Khi trạng thái đang chạy hoặc có tiến trình, nhấn **Cancel**.
3. Chờ trạng thái kết thúc.

**Kỳ vọng:** Job báo đã hủy rõ ràng và không để lại Auto Captions track hoặc cue một phần.

### 4.5 Kiểm tra lỗi đường dẫn

1. Mở **✨ Auto captions**.
2. Nhập đường dẫn không tồn tại cho CLI hoặc model.
3. Nhấn **Generate captions**.

**Kỳ vọng:** Thông báo lỗi nêu rõ đường dẫn/công cụ/model không hợp lệ; project không thay đổi.

Nếu không có CLI/model, ghi `Auto captions: BLOCKED (CLI/model available: no)` và bỏ qua các bước 4.2–4.5. Đây không phải lỗi của ứng dụng.

---

## 5. Playback, seek và mute

1. Đặt playhead ở đầu project rồi nhấn **Play** trong Preview.
2. Để project chạy hơn 10 giây.
3. Nhấn **Pause**, chờ khoảng 2 giây, sau đó Play lại.
4. Bấm vào một vị trí giữa clip trên thước Timeline để seek. Lặp lại ở ít nhất ba vị trí, gồm gần đầu và gần cuối project.
5. Dùng điều khiển quay về đầu và đi đến cuối project (nếu hiển thị); nếu không, click trực tiếp ở đầu/cuối thước Timeline.
6. Trong tiêu đề track trên Timeline, nhấn **Mute** của track audio hoặc video có tiếng, sau đó nhấn **Unmute**.

**Kỳ vọng:** Hình và tiếng chạy đồng bộ; Pause/Play/seek phản hồi nhanh; Preview không thành màn hình đen; seek không bị kẹt ở vị trí cũ; Mute/Unmute đổi âm lượng ngay mà không làm dừng Preview.

---

## 6. Transition, crossfade và tốc độ

### 6.1 Video transition

1. Đặt hai video clip liền nhau trên cùng một Video track.
2. Chọn điểm cắt hoặc một clip để mở phần transition trong Inspector.
3. Lần lượt tạo **Cross Dissolve** và **Dip to Black**.
4. Phát qua điểm cắt.
5. Dùng nút remove/xóa transition, sau đó Undo và Redo.
6. Nếu Inspector cho phép, thử duration được hỗ trợ.

**Kỳ vọng:** Transition được tạo, hiện trên Timeline/Preview và playback mượt qua điểm cắt. Remove, Undo và Redo đúng trạng thái.

### 6.2 Audio crossfade và trường hợp thiếu handle

1. Đặt hai audio clip liền nhau trên cùng một Audio track.
2. Tạo audio crossfade và thay duration được hỗ trợ.
3. Nghe qua điểm nối, rồi Undo/Redo.
4. Tạo tình huống thiếu handle bằng cách trim sát đầu/cuối source; thử tạo crossfade lại.

**Kỳ vọng:** Crossfade hợp lệ làm âm lượng chuyển mượt, duration và Undo/Redo hoạt động. Trường hợp thiếu handle hiện lý do cụ thể thay vì thông báo chung chung.

### 6.3 Tốc độ clip

1. Chọn một video hoặc audio clip.
2. Trong Inspector, chọn lần lượt **0.5× (Half)**, **1× (Normal)**, **2× (Double)**.
3. Với mỗi tốc độ, phát một đoạn có mốc âm thanh/hình dễ nhận biết.
4. Export ít nhất một tốc độ khác 1× trong mục 7.

**Kỳ vọng:** Half kéo dài, Double rút ngắn và Normal trở về thời lượng bình thường. Playback và export phản ánh đúng tốc độ đã chọn.

---

## 7. Export tổng hợp

1. Chuẩn bị một project ngắn có đủ: video, audio, caption, text template, một thay đổi style/hiệu ứng đang hỗ trợ và một transition.
2. Chọn preset chất lượng export mong muốn trên TopBar.
3. Nhấn **🎬 Export**.
4. Chọn nơi lưu file `.mp4`; chờ job hoàn tất.
5. Mở file MP4 bằng trình phát ngoài TinixCut và xem/nghe toàn bộ đoạn.

**Kỳ vọng:** Export hoàn tất không lỗi. File có hình, tiếng, timing, caption/text, bố cục nền, transition và tốc độ đúng như Preview.

Khi `FAIL`, ghi thêm: tên preset, thời lượng project, thời điểm xuất hiện lỗi trong video và đường dẫn file output nếu có.

---

## 8. Save, mở lại và relink media

1. Nhấn **💾 Save**, đóng project, sau đó **📂 Open** đúng file `.tinix` vừa lưu.
2. Xác nhận các track, clip, canvas, text/template và thay đổi đang có vẫn tồn tại.
3. Đóng project. Trong File Explorer, đổi tên hoặc di chuyển **một** tệp media đã import sang thư mục khác.
4. Mở lại project trong TinixCut.
5. Quan sát trạng thái media bị mất.
6. Dùng thao tác relink trong Media Library/Inspector khi ứng dụng hiển thị trạng thái media offline, rồi trỏ đến đúng tệp đã đổi tên/di chuyển.
7. Phát clip đã relink.

**Kỳ vọng:** Media thiếu được báo rõ là offline/missing; clip không tự đổi thành tệp khác. Sau relink đúng tệp, clip hoạt động lại với vị trí/timing trên Timeline không đổi.

---

## 9. Mẫu báo cáo gửi lại

Sao chép khối sau, thay bằng kết quả thực tế. Với `FAIL`, nêu thao tác tối thiểu để tái hiện và kết quả nhìn/nghe được.

```text
Phase 26
- Canvas/templates: PASS/FAIL
- Caption import/style/export: PASS/FAIL
- Auto captions: PASS/FAIL/BLOCKED (CLI/model available: yes/no)
- Playback/seek/mute: PASS/FAIL
- Transitions/crossfade/speed: PASS/FAIL
- Export: PASS/FAIL
- Save/reopen/missing-media relink: PASS/FAIL
- Failures: exact action + visible result
```
