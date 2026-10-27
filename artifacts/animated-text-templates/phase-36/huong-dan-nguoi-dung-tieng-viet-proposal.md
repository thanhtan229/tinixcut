# Hướng Dẫn Sử Dụng TinixCut: Bộ Công Cụ Template Văn Bản Động (Đề Xuất v0.1.0)

Tài liệu này cung cấp hướng dẫn thực hành chi tiết bằng tiếng Việt về hệ thống Animated Text Templates, nhập CapCut và tự động hóa Agent trong phiên bản TinixCut v0.1.0.

---

## 1. Tạo và Áp Dụng Template Văn Bản Động

### 1.1. Thêm template có sẵn từ thư viện
1. Trên giao diện chính của TinixCut, mở thanh công cụ bên trái và chọn biểu tượng **Templates (Thư viện mẫu)**.
2. TinixCut cung cấp 12 mẫu template tích hợp sẵn (như *Headline Bold*, *Cyber Frame*, *Typewriter Box*, *Retro Neon*, v.v.).
3. Nhấp đúp vào mẫu hoặc kéo thả vào vị trí mong muốn trên timeline video/text track.
4. Clip template sẽ hiển thị trên timeline với độ dài mặc định (ví dụ: 3 giây hoặc 5 giây).

### 1.2. Chỉnh sửa tham số hiển thị (Parameters)
1. Chọn clip template trên timeline.
2. Bảng **Inspector (Thuộc tính)** ở góc phải sẽ hiển thị các tham số được gán quyền chỉnh sửa:
   - **Văn bản chính (Title Text):** Nhập nội dung văn bản tiếng Việt có dấu.
   - **Màu sắc chủ đạo (Accent Color):** Bảng chọn màu hoặc nhập mã HEX.
   - **Độ trong suốt nền (Background Opacity):** Thanh trượt từ 0% đến 100%.
   - **Kích thước viền/khung (Bar Width / Scale):** Điều chỉnh linh hoạt theo kích thước khung hình.
3. Thay đổi sẽ cập nhật trực tiếp trên khung xem trước (Preview Canvas).

---

## 2. Thiết Kế Kiểu Chữ Tiếng Việt Nâng Cao (Rich Typography)

TinixCut tích hợp bộ tạo đồ họa 2D thuần Rust hỗ trợ đầy đủ chuẩn chính tả tiếng Việt:

### 2.1. Dấu thanh tiếng Việt không bị lỗi khi chuyển động
- Hệ thống tự động chuẩn hóa văn bản về Unicode NFC và phân tách theo Extended Grapheme Cluster (UAX #29).
- Khi chữ cái xoay, rơi hoặc trượt từng ký tự (stagger animation), các phụ âm kép và nguyên âm ghép dấu (`ử`, `ệ`, `ế`, `à`, `õ`) luôn dính liền thành một khối thống nhất, không bao giờ bị rơi rụng hay lệch dấu thanh.

### 2.2. Dải màu chuyển tiếp (Gradient Fill)
- Bạn có thể cấu hình màu chữ là màu đơn sắc (Solid) hoặc chuyển sắc đa điểm (Linear/Radial Gradient).
- Góc gradient (ví dụ: 118 độ) được cố định theo biên độ chữ tĩnh (static bounds), giúp màu sắc chuyển động mượt mà không bị giật nhấp nháy theo từng frame.

### 2.3. Viền chữ kép (Double Stroke) và Đổ bóng đa lớp (Multi-Shadow)
- Hỗ trợ tối đa 4 lớp viền ngoài được xếp từ dày đến mỏng, giúp chữ nổi bật hoàn toàn trên nền video chuyển động phức tạp.
- Hỗ trợ tối đa 4 lớp bóng mờ tách biệt với bán kính làm mờ Gaussian (blur radius) và độ lệch trục X, Y tùy chọn.

---

## 3. Quản Lý Tài Nguyên, Font Chữ và Gói Template (`.tinixtpl`)

### 3.1. Font chữ mặc định và font dự án
- TinixCut đóng gói sẵn font mã nguồn mở **Roboto-Bold** (giấy phép Apache-2.0). Ứng dụng chạy hoàn toàn offline mà không phụ thuộc vào font hệ thống Windows.
- Khi sử dụng font ngoài, hãy thêm file font (`.ttf` hoặc `.otf`) vào mục Assets của dự án. TinixCut tuyệt đối **không tự ý thay thế font âm thầm** mà sẽ hiển thị thông báo rõ ràng nếu thiếu font.

### 3.2. Xuất và Nhập Gói Template (`.tinixtpl`)
- **Xuất gói:** Chọn một template đã thiết kế → Chọn **Export Pack** → Chọn thư mục lưu file `.tinixtpl`. Toàn bộ định nghĩa, hình ảnh trang trí và font liên quan sẽ được đóng gói an toàn vào một file nén chuẩn hóa.
- **Nhập gói:** Nhấp nút **Import Pack** → Chọn file `.tinixtpl`. Hệ thống sẽ kiểm tra chữ ký SHA-256, quét mã độc ngăn chặn tấn công đường dẫn (Path Traversal) trước khi lưu vào thư viện mẫu cục bộ.

---

## 4. Nhập Mẫu Từ CapCut (CapCut Draft Import)

### 4.1. Định dạng hỗ trợ
- Hỗ trợ nhập trực tiếp thư mục dự án CapCut (chứa `draft_content.json`) hoặc file ZIP đóng gói toàn bộ thư mục dự án.
- **Hỗ trợ đầy đủ (Scope Option 1):** Mẫu chữ hiệu ứng chuyển động ký tự trượt kết hợp nảy/xoay (*StaggeredCharacterIn*), viền kép, bóng đổ kép và dải màu gradient.

### 4.2. Báo cáo chuyển đổi (Conversion Report)
Trước khi đưa vào dự án, TinixCut sẽ hiển thị bảng báo cáo kiểm định:
- **FULL:** Chuyển đổi hoàn hảo 100%, giữ nguyên toàn bộ văn bản và chuyển động.
- **PARTIAL:** Văn bản vẫn chỉnh sửa được, nhưng một số hiệu ứng phức tạp (ví dụ: chữ uốn lượn theo đường cong) được đưa về dạng thẳng ngang tiêu chuẩn.
- **BAKED REFERENCE:** Các hiệu ứng 3D khối đặc hoặc hiệu ứng shader độc quyền của CapCut sẽ được giữ nguyên dưới dạng một lớp video tĩnh tham chiếu để bạn lót nền.

### 4.3. Cách xử lý khi thiếu tài nguyên (Missing Resources)
- Nếu bản nháp CapCut sử dụng hình ảnh hoặc font nằm ở thư mục tạm ngoài máy, TinixCut sẽ đánh dấu trạng thái thiếu tài nguyên.
- Bạn có thể nhấp **Relink** để chọn lại file tương ứng trên máy tính của mình.

---

## 5. Tự Động Hóa Với Trợ Lý AI (Agent Workflow)

TinixCut cung cấp giao diện chuẩn hóa cho các lập trình viên và trợ lý AI thông qua CLI và giao thức MCP (Model Context Protocol).

### 5.1. Tạo và kiểm tra bản nháp template qua CLI
1. **Xem tài nguyên và giới hạn:**
   ```powershell
   tinixcut template capabilities
   ```
2. **Kiểm tra tính hợp lệ của bản nháp JSON:**
   ```powershell
   tinixcut template validate my_draft.json
   ```
3. **Kết xuất ảnh xem trước (Preview):**
   ```powershell
   tinixcut template preview my_draft.json preview.png --time 1.0
   ```
4. **Áp dụng trực tiếp vào dự án video:**
   ```powershell
   tinixcut template apply my_project.tcproj <item_id> my_draft.json
   ```

### 5.2. Kết nối qua MCP Server
Khởi động máy chủ MCP stdio phục vụ các AI Client (Claude Desktop, Cursor, Custom Agent):
```powershell
tinixcut mcp
```
Máy chủ hỗ trợ đầy đủ 12 công cụ bản quyền phục vụ thanh tra dự án, áp dụng lệnh giao dịch phân phiên và quản lý template an toàn.

---

## 6. Các Bước Kiểm Lỗi & Khắc Phục Sự Cố

| Hiện tượng | Nguyên nhân khả dĩ | Hướng xử lý |
| :--- | :--- | :--- |
| **Báo lỗi Missing Font khi mở template** | Dự án sử dụng font ngoài chưa được đính kèm vào mục Assets | Mở bảng Inspector, tải file font `.ttf`/`.otf` vào mục Assets của dự án hoặc chọn chuyển về font mặc định Roboto-Bold. |
| **Khung xem trước không cập nhật chữ mới** | Bộ nhớ đệm cache frame của WebView chưa xóa theo phiên bản | Di chuyển thanh trượt thời gian (Playhead) lùi lại 1 tick để kích hoạt làm mới dirty ROI. |
| **File video xuất ra nền đen thay vì trong suốt** | Định dạng MP4 (H.264) tiêu chuẩn không hỗ trợ kênh Alpha | Khi cần video trong suốt để chèn vào phần mềm khác, xuất chuỗi ảnh PNG (PNG sequence) hoặc định dạng ProRes 4444 (dành cho v2). |
| **Lỗi xung đột thời lượng khi cắt ngắn clip** | Chính sách va chạm thời lượng đang đặt là `Reject` | Đổi thuộc tính `Duration Conflict Policy` trên clip sang `ProportionalScale` (co giãn tỷ lệ) hoặc `ClampOut` (cắt đuôi chuyển động kết). |

---

## 7. Các Giới Hạn Kỹ Thuật Đã Ghi Nhận

- **Xuất video phần cứng (Hardware Encoder):** Bản v0.1.0 mặc định sử dụng bộ mã hóa tối ưu đa luồng CPU `libx264` (đạt tốc độ kết xuất ấn tượng ~5x thời gian thực). Tính năng tăng tốc phần cứng NVENC/AMF/QSV đang trong lộ trình kiểm định phần cứng mở rộng.
- **Môi trường máy ảo Clean-VM:** Người dùng cài đặt qua file `TinixCut_0.1.0_x64-setup.exe` cần đảm bảo hệ điều hành Windows 10/11 có sẵn Microsoft Edge WebView2 Runtime tiêu chuẩn.
- **Giới hạn số lớp trên 1 template:** Tối đa 16 layers, 8 text slots, 512 ký tự/slot nhằm đảm bảo hiệu năng dựng hình 60 fps ổn định.
