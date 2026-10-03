# Hướng Dẫn Sử Dụng TinixCut (User Guide)

> **TinixCut** — Trình biên tập video desktop hiện đại với kiến trúc tự động hóa (Automation-First), thiết kế theo tiêu chuẩn kỹ thuật Tinix.
> 
> Hỗ trợ Windows 10/11, giao diện tối ưu (Dark Compact Workspace), an toàn dữ liệu tuyệt đối (Atomic Save & Promotion), xử lý media hiệu năng cao qua FFmpeg và sẵn sàng cho AI/Scripting automation.

---

## 1. Giới thiệu & Điểm nổi bật

TinixCut được phát triển theo mô hình **Modular Monolith** kết hợp giữa giao diện người dùng **React + TypeScript (Tauri 2)** và nhân xử lý **Rust Native**:

- **Nguồn chân lý duy nhất (Single Source of Truth):** Toàn bộ trạng thái dự án được lưu trong mô hình dữ liệu chuẩn (`Project Domain Model`), định dạng thời gian `RationalTime` (timescale 60,000) bảo đảm độ chính xác khung hình tuyệt đối, không trôi lệch timecode.
- **An toàn dữ liệu tuyệt đối:** Cơ chế **Atomic Save** và **Snapshot Render** luôn ghi vào file tạm trước khi hoán đổi nguyên tử. Mất điện hoặc sự cố máy tính không bao giờ làm hỏng file dự án gốc hay file video đã xuất.
- **Cơ chế Command Service đơn nhất:** Mọi thao tác biên tập (từ nút bấm chuột, phím tắt, cho đến API tự động hóa của AI/CLI) đều đi qua cùng một cổng `CommandService`, có kiểm tra xung đột `baseRevision`, chống trùng lặp `operationId` và hỗ trợ rollback giao dịch nếu gặp lỗi.
- **Khả năng tự động hóa (Headless Automation):** Cho phép kiểm tra (`inspect`), dự báo thay đổi (`diff`), áp dụng lệnh (`apply`) và render video (`render`) hoàn toàn qua dòng lệnh không cần mở giao diện đồ họa.

---

## 2. Yêu cầu hệ thống & Cài đặt

### 2.1. Yêu cầu môi trường
- **Hệ điều hành:** Windows 10 hoặc Windows 11 (64-bit).
- **Phần mềm cần có:**
  - **Node.js** (v18, v20 hoặc v24) và **pnpm** (khuyên dùng `pnpm -v`).
  - **Rust toolchain** (1.80+ với MSVC target).
  - **FFmpeg & FFprobe** (khuyên dùng bản 6.0+ hoặc 9.0) nằm trong biến môi trường hệ thống `PATH`.
  - **Microsoft Edge WebView2 Runtime** (mặc định đã có sẵn trên Windows 10/11).

### 2.2. Khởi chạy ứng dụng
Trong thư mục dự án `TinixCut`, bạn có sẵn các file script tiện ích:

| File Script | Mục đích | Cách sử dụng |
| :--- | :--- | :--- |
| **`run_app.bat`** | **Mở giao diện ứng dụng Desktop (Khuyên dùng)** | Nhấp đúp chuột vào file hoặc gõ `.\run_app.bat` trong terminal. |
| **`run_tests.bat`** | Chạy toàn bộ bộ kiểm thử tự động (Typecheck, Vitest, Cargo test, Clippy) | Gõ `.\run_tests.bat` hoặc `.\run_tests.bat quick`. |

*(Hoặc dùng lệnh gốc: `pnpm tauri dev` để chạy dev, `pnpm tauri build` để đóng gói bản cài đặt).*

---

## 3. Tổng quan giao diện ứng dụng (Layout UI)

Giao diện TinixCut được thiết kế theo phong cách không gian làm việc chuyên nghiệp (Compact Dark Creative Workspace), gồm 5 khu vực chính:

```text
+--------------------------------------------------------------------------+
|  TopBar: [Brand] [+ New] [📂 Open] [+ Track] [+ Clip] [💾 Save] [🎬 Export] [↩ Undo] [↪ Redo] |
+------------------+------------------------------------+------------------+
|                  |                                    |                  |
|  Media Library   |          Preview Canvas            |    Inspector     |
|                  |     (Màn hình phát video &         |                  |
|  - Danh sách     |      Timecode HH:MM:SS:FF)         |  - Transform     |
|    Video / Audio |                                    |  - Opacity       |
|  - Nút + Import  +------------------------------------+  - Text Caption  |
|                  |          Timeline Editor           |  - Audio Fades   |
|                  |                                    |                  |
|                  |  - Multi-track (Video/Audio)       |                  |
|                  |  - Playhead & Trimming / Snapping  |                  |
+------------------+------------------------------------+------------------+
|  Status Bar: Trạng thái lệnh | Số tracks | Vị trí Playhead (frame) | Revision   |
+--------------------------------------------------------------------------+
```

### 3.1. Thanh công cụ phía trên (TopBar)
- **Brand & Core Status:** Hiển thị tên phần mềm, phiên bản lõi Rust và trạng thái kết nối (`● Core Healthy`).
- **Tên dự án & Revision:** Hiển thị tên file và số revision hiện tại của dự án.
- **`+ New`:** Tạo một dự án mới hoàn toàn trống.
- **`📂 Open`:** Mở hộp thoại duyệt file để nạp dự án cũ (`.tinix` hoặc `.tcproj`).
- **`+ Track`:** Thêm một track video mới vào timeline.
- **`+ Clip`:** Tạo nhanh một clip mẫu vào track đầu tiên để dựng thử.
- **`💾 Save`:** Lưu dự án nguyên tử xuống đĩa. Nếu là dự án mới, hộp thoại lưu file sẽ mở ra để chọn vị trí lưu.
- **`🎬 Export`:** Mở hộp thoại xuất video thành file `.mp4` chuẩn H.264/AAC qua FFmpeg.
- **`↩ Undo` / `↪ Redo`:** Hoàn tác hoặc làm lại các thao tác chỉnh sửa trước đó.

### 3.2. Bảng quản lý tài nguyên (Media Library - Bên trái)
- Hiển thị danh sách các tài nguyên media đã nạp vào dự án: Video, Audio, Hình ảnh.
- Nút **`+ Import`**: Kích hoạt hộp thoại chọn file trên máy tính (`.mp4`, `.mov`, `.avi`, `.mkv`, `.mp3`, `.wav`, `.png`, `.jpg`, `.webp`...).
- Mỗi tài nguyên hiển thị rõ loại media, dung lượng file và tên gốc.

### 3.3. Màn hình xem trước (Preview Canvas - Ở giữa bên trên)
- Hiển thị bố cục khung hình chuẩn Rec.709 $1920 \times 1080$ (tỉ lệ 16:9).
- Trực quan hóa vị trí clip, tỉ lệ phóng to/thu nhỏ (Scale), góc xoay (Rotation) và độ mờ (Opacity).
- Góc dưới bên phải hiển thị đồng hồ thời gian chuẩn SMPTE Timecode dạng `HH:MM:SS:FF` (Giờ:Phút:Giây:Khung hình) theo vị trí playhead.

### 3.4. Dòng thời gian biên tập (Timeline Editor - Ở giữa bên dưới)
- **Multi-track:** Hỗ trợ nhiều track video và audio xếp lớp theo thứ tự Z-index từ dưới lên trên.
- **Thanh đo thời gian (Ruler):** Đánh dấu các mốc frame (0f, 30f, 60f, 90f...).
- **Con trỏ Playhead:** Kéo thả vạch màu đỏ để duyệt thời gian trên toàn bộ timeline.
- **Kéo thả Clip (Move):** Nhấp giữ và kéo thân clip để dịch chuyển vị trí phát trên timeline.
- **Cắt ranh giới Clip (Trim In / Trim Out):** Rê chuột vào mép trái hoặc mép phải của clip, con trỏ đổi thành mũi tên 2 chiều, kéo để cắt bớt hoặc mở rộng phần đầu/cuối clip.
- **Các nút công cụ nhanh trên timeline:**
  - **`✂ Split at Playhead`:** Cắt đôi clip được chọn ngay tại vị trí con trỏ đỏ playhead.
  - **`🗑 Delete`:** Xóa clip đang chọn khỏi track.
  - **`📑 Duplicate`:** Nhân đôi clip được chọn đặt liền kề phía sau.
  - **`Zoom Slider`:** Điều chỉnh mức phóng to/thu nhỏ độ rộng timeline (từ 1px/frame đến 8px/frame).

### 3.5. Bảng thuộc tính (Inspector - Bên phải)
Khi nhấp chọn một clip trên timeline, bảng Inspector sẽ hiển thị các thuộc tính:
- **Clip Properties:** ID định danh duy nhất, loại clip (Video/Audio/Text), vị trí phát (Start frame, Duration).
- **Text Caption:** Nhập nội dung chữ hiển thị phụ đề trực tiếp lên video.
- **Transform (Biến đổi hình học):**
  - `Position X / Y`: Tọa độ vị trí tâm clip so với khung hình (đơn vị pixel).
  - `Scale`: Hệ số tỉ lệ phóng to/thu nhỏ (1.0 = kích thước gốc 100%).
  - `Rotation`: Góc xoay của clip (đơn vị độ, từ 0 đến 360).
- **Opacity (Độ mờ):** Thanh trượt điều chỉnh từ 0% (trong suốt hoàn toàn) đến 100% (rõ nét).
- **Audio Fades (Làm mờ âm lượng đầu/cuối):** Nhập số khung hình (frames) để âm thanh to dần khi bắt đầu clip (**Fade In**) hoặc nhỏ dần khi kết thúc clip (**Fade Out**).

---

## 4. Hướng dẫn các thao tác làm việc cơ bản

### 4.1. Tạo mới và Lưu dự án
1. Nhấn nút **`+ New`** trên thanh TopBar để tạo không gian làm việc mới.
2. Thêm track hoặc clip theo nhu cầu.
3. Nhấn **`💾 Save`**:
   - Nếu dự án chưa từng được lưu: Hộp thoại Windows Explorer sẽ hiện ra để bạn đặt tên và chọn thư mục lưu (file có đuôi `.tinix`).
   - Nếu dự án đã được lưu: Hệ thống sẽ tự động ghi đè nguyên tử an toàn vào đúng file đó và cập nhật số revision.

### 4.2. Mở lại dự án đã có
1. Nhấn nút **`📂 Open`** trên TopBar.
2. Chọn file `.tinix` hoặc `.tcproj` cần mở.
3. Ứng dụng sẽ tải lại toàn bộ danh sách track, các clip với đầy đủ vị trí, transform và thuộc tính.

### 4.3. Nhập tài nguyên (Import Media)
1. Trong panel **Media Library** bên trái, nhấn nút **`+ Import`**.
2. Chọn một hoặc nhiều file video/audio/hình ảnh từ máy tính của bạn.
3. FFmpeg sẽ ngầm quét thông số kỹ thuật (độ phân giải, thời lượng, bitrate) và nạp vào danh sách tài nguyên với mã định danh duy nhất (`ast_...`).

### 4.4. Đưa video vào Timeline để chỉnh sửa (Add to Timeline)
Sau khi video xuất hiện trong panel **Media Library**:
1. Nhìn vào thẻ thông tin của video đó trong Media Library.
2. Nhấn nút **`+ Timeline`** màu xanh ở góc phải của thẻ video:
   - Hệ thống sẽ tự động đưa video vào track video trên Timeline (nếu chưa có track nào, ứng dụng sẽ tự động tạo `Video Track 1`).
   - Nếu đã có clip trước đó trên track, video mới sẽ được xếp nối tiếp ngay phía sau để tránh đè lấn khung hình.
3. Nhấp chuột vào clip vừa tạo trên timeline (clip sẽ sáng viền xanh) để xem thông tin liên kết tài nguyên `🎬 [Tên video]` và bắt đầu chỉnh sửa trong bảng **Inspector** cũng như trên **Timeline**.
*(Ngoài ra, bạn cũng có thể bấm nút **`+ Clip`** trên TopBar nếu muốn tạo nhanh một clip mẫu).*

### 4.5. Thao tác dựng trên Timeline
- **Cắt đôi clip (Split):**
  1. Nhấp chuột chọn clip muốn cắt (clip sẽ có viền sáng viền xanh).
  2. Kéo con trỏ đỏ playhead đến vị trí cần cắt bên trong clip.
  3. Nhấn nút **`✂ Split at Playhead`** trên thanh công cụ timeline. Clip sẽ được tách thành 2 clip độc lập.
- **Rút ngắn clip (Trim):**
  - Đặt chuột vào mép trái clip và kéo sang phải để bỏ đoạn đầu.
  - Đặt chuột vào mép phải clip và kéo sang trái để bỏ đoạn đuôi.
- **Xóa clip:** Chọn clip và nhấn nút **`🗑 Delete`**.

### 4.6. Thêm chữ phụ đề (Caption)
1. Chọn clip trên timeline.
2. Trong bảng **Inspector** bên phải, tìm mục **TEXT CAPTION**.
3. Nhập nội dung văn bản bạn muốn hiển thị và nhấn **Enter** (hoặc nút **`✓ Apply`**). Màn hình xem trước (Preview Canvas) sẽ lập tức hiển thị dòng phụ đề nổi bật với khung nền chuẩn tương phản, đồng thời trên thân clip ở Timeline cũng xuất hiện nhãn phụ đề `💬`. Khi xuất video, FFmpeg sẽ kết xuất chính xác chữ này lên video.

### 4.7. Chỉnh âm thanh to/nhỏ dần (Audio Fades)
1. Chọn clip video hoặc audio trên timeline.
2. Tại bảng **Inspector**, cuộn xuống mục **AUDIO FADES (FRAMES)**:
   - Nhập ví dụ `30` vào ô **Fade In** (âm thanh sẽ to dần trong 1 giây đầu ở 30fps).
   - Nhập `30` vào ô **Fade Out** (âm thanh sẽ nhỏ dần về im lặng trong 1 giây cuối).

### 4.8. Xuất video thành phẩm (Export MP4)
1. Nhấn nút **`🎬 Export`** màu xanh nổi bật trên thanh TopBar.
2. Hộp thoại lưu file sẽ mở ra, chọn nơi bạn muốn lưu file video thành phẩm (mặc định tên `video.mp4`).
3. Nhấn **Save**:
   - Hệ thống tự động đóng băng trạng thái timeline thành snapshot bất biến.
   - FFmpeg bắt đầu kết xuất video ở tiến trình nền với codec H.264/AAC tương thích mọi thiết bị.
   - Nếu máy tính có GPU NVIDIA/Intel/AMD, phần cứng sẽ được sử dụng; nếu không có, hệ thống tự động chuyển sang phần mềm `libx264` mà không làm hỏng tiến trình render.
4. Khi hoàn thành, thanh trạng thái góc dưới sẽ hiển thị thông báo thành công kèm thời lượng video và thời gian render tính bằng mili-giây.

---

## 5. Hướng dẫn sử dụng cổng Dòng lệnh Headless CLI (Automation)

TinixCut cung cấp giao diện dòng lệnh mạnh mẽ dành cho các lập trình viên, script tự động hóa CI/CD, hoặc các AI Agent.

File thực thi nằm tại: `src-tauri/target/debug/tinixcut.exe` (hoặc `target/release/tinixcut.exe`).

### 5.1. Xem danh sách câu lệnh
```cmd
tinixcut.exe help
```
Kết quả hiển thị:
```text
TinixCut Automation CLI
Usage: tinixcut [inspect|diff|apply|render] <args...>

Commands:
  inspect <project_path>                     Inspect a project and output JSON summary
  diff <project_path> <envelope_json_path>   Run a dry-run diff of commands against a project
  apply <project_path> <envelope_json_path>  Apply commands transactionally to a project
  render <project_path> <output_path>        Render project to MP4 headlessly
  help, --help                               Show this help message
```

### 5.2. Lệnh `inspect` — Trích xuất cấu trúc dự án dạng JSON
Xem toàn bộ thông tin về track, clip, asset, framerate, độ phân giải mà không cần mở app:
```cmd
tinixcut.exe inspect "D:\Projects\my_video.tinix"
```

### 5.3. Lệnh `diff` — Dự báo thay đổi trước khi ghi (Dry-Run)
Kiểm tra xem một gói lệnh command envelope có hợp lệ hay có gây xung đột không trước khi ghi vào file:
```cmd
tinixcut.exe diff "D:\Projects\my_video.tinix" "D:\Commands\edit_envelope.json"
```

### 5.4. Lệnh `apply` — Áp dụng chỉnh sửa tự động vào file dự án
Thực thi các lệnh biên tập và tự động lưu dự án an toàn:
```cmd
tinixcut.exe apply "D:\Projects\my_video.tinix" "D:\Commands\edit_envelope.json"
```

### 5.5. Lệnh `render` — Xuất video tự động không cần mở giao diện
```cmd
tinixcut.exe render "D:\Projects\my_video.tinix" "D:\Exports\final_video.mp4"
```

---

## 6. Khắc phục sự cố thường gặp (Troubleshooting)

| Tình huống | Nguyên nhân | Cách xử lý |
| :--- | :--- | :--- |
| **Báo lỗi `FFmpeg / FFprobe not found`** | Máy tính chưa cài FFmpeg hoặc chưa thêm vào biến môi trường `PATH`. | Tải FFmpeg từ [ffmpeg.org](https://ffmpeg.org), giải nén và thêm thư mục `bin` vào biến môi trường `PATH` của Windows. Kiểm tra lại bằng `ffmpeg -version` trong terminal. |
| **Báo lỗi `pnpm is not recognized`** | Chưa cài đặt pnpm package manager. | Mở PowerShell và chạy: `npm install -g pnpm`. |
| **Báo lỗi `cargo is not recognized`** | Chưa cài đặt Rust toolchain. | Tải và cài đặt rustup từ [rustup.rs](https://rustup.rs). |
| **Không thấy giao diện khi chạy `run_tests.bat`** | File `run_tests.bat` là công cụ chạy test tự động trong dòng lệnh. | Hãy nhấp đúp vào file **`run_app.bat`** để mở cửa sổ giao diện ứng dụng. |
| **File video xuất ra bị màn hình đen** | Các clip trên timeline chưa được liên kết với file video thật (clip mẫu). | Nhấn **`+ Import`** trong Media Library để chọn file video thật từ máy tính trước khi xếp lên timeline. |
