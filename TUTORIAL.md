# Hướng Dẫn Thực Hành TinixCut (Hands-on Tutorial)

> **Mục tiêu khóa thực hành:** Tự tay tạo ra một video ngắn hoàn chỉnh (Video Showcase) với đầy đủ các yếu tố: hình ảnh, video clip, phụ đề động, hiệu ứng âm thanh và xuất ra file MP4 chuẩn chất lượng cao.
>
> **Thời lượng ước tính:** 15 – 20 phút.  
> **Độ khó:** Cơ bản đến Nâng cao (có kèm bài tập thực hành dòng lệnh cho AI / Dev).

---

## Chuẩn bị trước khi thực hành

1. Đảm bảo bạn đang ở thư mục dự án `TinixCut`.
2. Kiểm tra các công cụ hệ thống bằng cách mở terminal PowerShell và gõ:
   ```powershell
   pnpm -v
   cargo --version
   ffmpeg -version
   ```
3. Khởi động ứng dụng bằng cách nhấp đúp vào file **`run_app.bat`** (hoặc chạy `pnpm tauri dev`). Cửa sổ giao diện TinixCut sẽ xuất hiện.

---

## Bài Thực Hành 1: Khởi Tạo Dự Án & Thiết Lập Ban Đầu

### Mục tiêu:
Tạo một dự án video mới và làm quen với thanh trạng thái.

### Các bước thực hiện:
1. Quan sát góc trên bên trái màn hình: Bạn sẽ thấy nhãn `● Core Healthy (v0.1.0)` báo hiệu hệ thống Rust backend đang hoạt động hoàn hảo.
2. Nhấn nút **`+ New`** trên thanh TopBar.
3. Thanh trạng thái ở chân trang (Status Bar) sẽ đổi sang: `Status: New project created`.
4. Quan sát thông số bên phải Status Bar:
   - `Tracks: 0` (chưa có track nào).
   - `Playhead: 0f` (con trỏ thời gian ở khung hình số 0).
   - `Revision: 0` (phiên bản dự án ban đầu).

---

## Bài Thực Hành 2: Tạo Cấu Trúc Timeline Đa Track

### Mục tiêu:
Tạo các track video độc lập để xếp lớp hình ảnh và video.

### Các bước thực hiện:
1. Nhấn nút **`+ Track`** trên TopBar lần thứ nhất:
   - Một track video có tên `Track 1` xuất hiện trên timeline.
   - Status Bar cập nhật: `Add Track committed (rev 1)`.
2. Nhấn nút **`+ Track`** trên TopBar lần thứ hai:
   - Một track video tiếp theo có tên `Track 2` xuất hiện bên dưới `Track 1`.
   - Status Bar cập nhật: `Add Track committed (rev 2)`.
3. Nhấn nút **`↩ Undo`** trên TopBar:
   - `Track 2` biến mất, trạng thái dự án trở về `rev 1`.
4. Nhấn nút **`↪ Redo`**:
   - `Track 2` được phục hồi nguyên vẹn, trạng thái trở lại `rev 2`.

> **💡 Ghi nhớ:** Mọi thao tác biên tập trong TinixCut đều là một giao dịch nguyên tử (`Atomic Transaction`). Bạn luôn có thể Undo/Redo an toàn mà không sợ mất dữ liệu.

---

## Bài Thực Hành 3: Nhập Media & Thêm Clip Vào Timeline

### Lựa chọn A: Dùng file video / hình ảnh thật trên máy tính của bạn
1. Tại bảng **Media Library** bên trái, nhấn nút **`+ Import`**.
2. Hộp thoại Windows Explorer sẽ mở ra. Chọn một video ngắn (`.mp4` hoặc `.mov`) hoặc một file ảnh (`.png`, `.jpg`).
3. File media sẽ xuất hiện trong danh sách Media Library kèm loại media và dung lượng file.
4. **Đưa video vào Timeline để chỉnh sửa:**
   - Trên thẻ video vừa nạp ở bảng Media Library, nhấn nút **`+ Timeline`** màu xanh.
   - Một clip video mang đúng tên file của bạn sẽ xuất hiện trên track ở Timeline!
   - Nhấp chọn clip đó (clip sẽ có viền sáng xanh), bảng **Inspector** bên phải sẽ hiển thị `Media: 🎬 [Tên video]`.

### Lựa chọn B: Dùng chức năng tạo Clip mẫu có sẵn
1. Nhấn nút **`+ Clip`** trên TopBar:
   - Một clip mẫu dài 60 frames (tương đương 2 giây ở 30 fps) có tên `Clip 1` sẽ được tạo trên `Track 1`.
2. Nhấn nút **`+ Clip`** một lần nữa:
   - `Clip 2` sẽ được tự động tính toán vị trí để đặt tiếp nối ngay sau `Clip 1` (bắt đầu từ frame 60 đến frame 120), tránh hoàn toàn việc đè lấn khung hình.

---

## Bài Thực Hành 4: Kỹ Thuật Biên Tập Trên Timeline (Move, Trim & Split)

### 4.1. Kéo thả di chuyển Clip (Move)
1. Dùng chuột nhấp chọn `Clip 1` (clip sẽ sáng viền xanh).
2. Nhấp giữ chuột vào giữa thân clip và kéo sang phải đến mốc frame 30 rồi nhả chuột:
   - Clip sẽ chuyển sang vị trí mới bắt đầu từ frame 30.
   - Màn hình xem trước và đồng hồ timecode sẽ đồng bộ tương ứng.

### 4.2. Cắt bớt phần đầu / đuôi Clip (Trim In & Trim Out)
1. Rê chuột vào **mép phải** của `Clip 1`: Con trỏ chuột sẽ đổi thành biểu tượng mũi tên hai chiều `↔`.
2. Nhấp giữ chuột và kéo về bên trái khoảng 20 frames:
   - Thời lượng của clip sẽ rút ngắn lại.
   - Thao tác này giúp bạn loại bỏ các đoạn thừa ở cuối video.

### 4.3. Cắt đôi Clip tại vị trí con trỏ (Split)
1. Dùng chuột nhấp vào thanh thước đo thời gian (Ruler) hoặc kéo con trỏ màu đỏ **Playhead** đặt vào chính giữa `Clip 1` (ví dụ ở frame 45).
2. Đảm bảo `Clip 1` đang được chọn (viền xanh).
3. Nhấn nút **`✂ Split at Playhead`** trên thanh công cụ timeline:
   - Ngay lập tức, `Clip 1` được tách thành 2 clip riêng biệt. Bạn có thể xóa một trong hai nửa hoặc kéo nửa sau sang vị trí khác!

### 4.4. Nhân đôi Clip (Duplicate)
1. Nhấp chọn nửa clip đầu tiên.
2. Nhấn nút **`📑 Duplicate`**:
   - Một bản sao giống hệt của clip sẽ xuất hiện ngay phía sau.

---

## Bài Thực Hành 5: Tinh Chỉnh Biến Đổi Hình Học (Transform & Opacity)

### Mục tiêu:
Điều chỉnh kích thước, vị trí và độ mờ của clip để tạo hiệu ứng hình trong hình (Picture-in-Picture).

### Các bước thực hiện:
1. Nhấp chọn một clip trên timeline.
2. Nhìn sang bảng **Inspector** bên phải màn hình:
3. **Chỉnh vị trí (Position):**
   - Đặt `Position X` = `100` (clip dịch sang phải 100 pixel).
   - Đặt `Position Y` = `-50` (clip dịch lên trên 50 pixel).
   - Màn hình Preview Canvas sẽ cập nhật ngay vị trí mới của clip.
4. **Chỉnh tỉ lệ (Scale):**
   - Đổi `Scale` từ `1.0` thành `0.75` (thu nhỏ clip lại 75% kích thước gốc).
5. **Chỉnh góc xoay (Rotation):**
   - Đặt `Rotation` = `15` (clip nghiêng một góc 15 độ).
6. **Chỉnh độ mờ (Opacity):**
   - Kéo thanh trượt Opacity về mức `0.8` (80% rõ nét, 20% trong suốt nhìn xuyên lớp dưới).

---

## Bài Thực Hành 6: Thêm Chữ Phụ Đề (Text Caption) & Làm Mượt Âm Thanh (Audio Fades)

### 6.1. Thêm Phụ đề chữ lên Video
1. Đảm bảo clip đang được chọn.
2. Trong bảng **Inspector**, tìm ô **TEXT CAPTION**.
3. Nhập dòng chữ:
   ```text
   Xin chào tinixcut!
   ```
4. Nhấn **Enter** (hoặc nhấp nút **`✓ Apply`**):
   - Ngay lập tức trên **Preview Canvas**, dòng chữ phụ đề nổi bật màu trắng với nền đen tương phản sẽ xuất hiện ở giữa/dưới khung hình video!
   - Trên thân clip ở **Timeline**, một nhãn huy hiệu `💬 Xin chào tinixcut!` xuất hiện để bạn dễ dàng quản lý.
   - Khi xuất video MP4, FFmpeg sẽ tự động kết xuất dòng chữ này sắc nét lên video thành phẩm.

### 6.2. Thiết lập Âm thanh to dần và nhỏ dần (Audio Fades)
1. Vẫn trong bảng **Inspector**, cuộn xuống mục **AUDIO FADES (FRAMES)**.
2. Tại ô **Fade In**, nhập: `30` (âm thanh sẽ to dần trong vòng 1 giây đầu tiên).
3. Tại ô **Fade Out**, nhập: `30` (âm thanh sẽ nhỏ dần về im lặng trong 1 giây cuối cùng trước khi hết clip).

---

## Bài Thực Hành 7: Lưu Dự Án An Toàn & Xuất Video MP4

### 7.1. Lưu dự án (`.tinix`)
1. Nhấn nút **`💾 Save`** trên TopBar.
2. Vì đây là dự án mới, hộp thoại lưu file của Windows sẽ hiện ra:
   - Chọn thư mục bạn muốn lưu (ví dụ: `Desktop` hoặc thư mục làm việc của bạn).
   - Đặt tên file: `demo_showcase.tinix`.
   - Nhấn **Save**.
3. Thanh trạng thái sẽ báo: `Project saved atomically to ...demo_showcase.tinix`.

### 7.2. Thử nghiệm Mở lại Dự án (Open Project)
1. Nhấn nút **`+ New`** để đưa màn hình về trạng thái trống ban đầu.
2. Nhấn nút **`📂 Open`** trên TopBar:
   - Chọn file `demo_showcase.tinix` vừa lưu.
   - Toàn bộ track, clip, vị trí playhead, text caption và hiệu ứng transform được phục hồi chính xác 100%!

### 7.3. Xuất video thành phẩm (`.mp4`)
1. Nhấn nút **`🎬 Export`** màu xanh nổi bật trên thanh TopBar.
2. Hộp thoại lưu file MP4 sẽ hiện ra:
   - Đặt tên: `demo_showcase.mp4`.
   - Nhấn **Save**.
3. Hệ thống sẽ trích xuất snapshot bất biến và gọi FFmpeg xử lý ở tiến trình nền.
4. Sau vài giây, thanh trạng thái hiển thị:
   ```text
   Export complete! D:\Exports\demo_showcase.mp4 (4.0s in 520ms)
   ```
5. Mở thư mục chứa file và mở `demo_showcase.mp4` bằng Windows Media Player hoặc VLC để thưởng thức thành quả video đầu tiên của bạn!

---

## Bài Thực Hành 8 (Nâng Cao): Tự Động Hóa Dòng Lệnh Không Cần Giao Diện (Headless CLI)

TinixCut hỗ trợ các kỹ sư phần mềm và AI Agent can thiệp trực tiếp qua dòng lệnh.

### Bước 1: Mở terminal tại thư mục dự án
Mở PowerShell tại thư mục `TinixCut`.

### Bước 2: Kiểm tra cấu trúc file dự án bằng lệnh `inspect`
Chạy lệnh:
```powershell
.\src-tauri\target\debug\tinixcut.exe inspect "demo_showcase.tinix"
```
Hệ thống sẽ trả về cấu trúc JSON chi tiết:
```json
{
  "project_id": "prj_...",
  "name": "demo_showcase",
  "revision": 8,
  "duration_frames": 120,
  "duration_seconds": 4.0,
  "frame_rate": {
    "numerator": 30,
    "denominator": 1
  },
  "resolution": [1920, 1080],
  "tracks_count": 2,
  "clips_count": 2
}
```

### Bước 3: Xuất video trực tiếp qua dòng lệnh bằng lệnh `render`
Chạy lệnh:
```powershell
.\src-tauri\target\debug\tinixcut.exe render "demo_showcase.tinix" "cli_output.mp4"
```
Video sẽ được FFmpeg tự động kết xuất trực tiếp ra file `cli_output.mp4` mà không cần bật cửa sổ đồ họa!

---

## Bảng Tự Đánh Giá Kết Quả Thực Hành (Self-Check Checklist)

Hãy đánh dấu kiểm tra các mục bạn đã hoàn thành:

- [ ] Đã khởi động thành công ứng dụng qua file `run_app.bat`.
- [ ] Đã tạo được ít nhất 2 track video trên timeline.
- [ ] Đã thêm clip và thực hiện thành công các thao tác: **Move**, **Trim**, **Split** và **Duplicate**.
- [ ] Đã thử nghiệm thành công tính năng **Undo** và **Redo**.
- [ ] Đã thay đổi được vị trí (Position), độ phóng to (Scale) và độ mờ (Opacity) trong bảng Inspector.
- [ ] Đã gán được **Text Caption** và cấu hình **Audio Fades** cho clip.
- [ ] Đã lưu dự án thành công thành file `.tinix` và mở lại thành công qua nút **`📂 Open`**.
- [ ] Đã xuất thành công video thành phẩm `.mp4` và xem lại được trên trình phát video của máy tính.

**Chúc mừng bạn đã hoàn thành xuất sắc khóa thực hành làm chủ TinixCut!**
