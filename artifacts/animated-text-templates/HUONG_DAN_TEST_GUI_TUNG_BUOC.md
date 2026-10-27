# Hướng dẫn test GUI từng bước (G1–G4)

Thời gian ước tính: 15 phút. Không cần biết lập trình. Chỉ cần làm theo từng bước và ghi kết quả.

---

## Bước 0 — Mở ứng dụng

1. Mở thư mục `D:\___TOOLS\__TINIX\TinixCut`.
2. Double-click file **`run_app.bat`**.
3. Đợi khoảng 1–3 phút. Cửa sổ **TinixCut** sẽ hiện ra.
   - Nếu cửa sổ đen báo lỗi, chụp màn hình lỗi và gửi cho trợ lý. Không làm tiếp.

Lưu ý: giữ cửa sổ đen (terminal) mở suốt quá trình test. Đóng nó sẽ tắt ứng dụng.

---

## Phần 1 — Nhập CapCut (G1, G2)

Ghi chú: thao tác này chỉ **xem** kết quả kiểm tra. Không cần áp dụng vào project.

### G1 — Nhập draft CapCut thật của bạn

1. Ở **cột bên phải** của ứng dụng, bấm tab **Templates** (có chữ "Templates").
2. Tìm nút có tooltip **"Inspect and convert a CapCut draft"** (thường nằm cạnh các nút "Import .tinixtpl" và "Export"). Bấm vào nút đó.
3. Hộp thoại **"Import CapCut template"** mở ra.
4. Bấm nút **"Choose draft folder…"**.
5. Trong cửa sổ chọn thư mục, đi tới:
   ```
   C:\Users\Admin\Downloads\__CAPCUT\CapCut Drafts\TINIX_CAPCUT_IMPORT_TEST
   ```
   Chọn đúng thư mục **TINIX_CAPCUT_IMPORT_TEST** rồi bấm **Select Folder** (hoặc Open).
6. Đợi vài giây. Hộp thoại sẽ hiện:
   - Phần **Source SHA-256** (một dãy chữ và số).
   - Mục **Conversion details** (chi tiết chuyển đổi).
   - Phần **Missing resources** nếu có tài nguyên thiếu.

**Kết quả mong đợi (ghi lại):**
- [ ] Hộp thoại hiện kết quả kiểm tra, không báo lỗi đỏ.
- [ ] Có mục "Conversion details".
- [ ] Kiểu chuyển đổi là **chuyển đổi đầy đủ** (full conversion), kiểu **StaggeredCharacterIn** nếu được hiển thị.

Chụp ảnh màn hình hộp thoại và lưu vào `artifacts\animated-text-templates\phase-34\native\G1_hop_thoai.png`. Nếu chưa có thư mục `native`, tạo mới.

Sau đó bấm **×** ở góc trên bên phải hộp thoại để đóng.

### G2 — Thử một thư mục không hợp lệ

1. Tạo thư mục mới: `D:\TinixTest\BadDraft`.
2. Trong thư mục đó, tạo một file văn bản tên **draft_content.json** với nội dung: `ko phai draft`.
   - Cách làm: mở Notepad, gõ `ko phai draft`, lưu với tên `draft_content.json` (chọn "All files" ở mục Save as type).
3. Mở lại hộp thoại **"Import CapCut template"** (làm lại bước 1–2 của G1).
4. Bấm **"Choose draft folder…"**, chọn thư mục `D:\TinixTest\BadDraft`.

**Kết quả mong đợi (ghi lại):**
- [ ] Hiện thông báo lỗi rõ ràng (chữ đỏ hoặc hộp cảnh báo), ví dụ không đọc được draft.
- [ ] Ứng dụng không bị treo, không đóng.
- [ ] Không có gì được thêm vào project đang mở.

Chụp ảnh thông báo lỗi, lưu thành `artifacts\animated-text-templates\phase-34\native\G2_loi.png`.

---

## Phần 2 — Mẫu Phase 31 (G3, G4)

### Chuẩn bị file test (làm một lần)

1. Tạo thư mục `D:\TinixTest`.
2. Mở thư mục `D:\___TOOLS\__TINIX\TinixCut\artifacts\animated-text-templates\phase-31`.
3. Copy file **recreated_sample_project.json** vào `D:\TinixTest`.
4. Đổi tên bản copy thành **p31_test.tcproj**.
   - Nếu Windows không hiện đuôi file: vào View → bật "File name extensions" trong File Explorer.
   - Không đụng file gốc trong thư mục phase-31.

### Mở project

1. Trong ứng dụng TinixCut, ở thanh trên cùng, bấm **"Open Existing Project"** (biểu tượng thư mục).
2. Chọn `D:\TinixTest\p31_test.tcproj` rồi bấm **Open**.
3. Trên khung xem trước (giữa màn hình), bạn sẽ thấy chữ **"Tinix — Thử nghiệm tiếng Việt"** trên timeline.

Nếu không mở được file: báo lỗi nguyên văn cho trợ lý, không sửa file gốc.

### G3 — Chữ có chuyển động không, và có phát được không

1. Bấm vào **clip chữ** trên timeline để chọn nó.
2. Bấm **Space** (phím cách) để phát. Chờ khoảng 2 giây rồi bấm Space để dừng.
3. Quan sát khung xem trước:
   - Chữ xuất hiện dần, không đứng yên ngay từ đầu.
   - Không có dòng chữ đỏ hoặc thông báo **"Unable to play media"**.
4. Kéo đầu thanh playhead (đường dọc trên timeline) về **đầu**, rồi **giữa**, rồi **cuối**. Ở mỗi vị trí, chữ hiển thị đúng, không bị đứng hình.

**Kết quả mong đợi (ghi lại):**
- [ ] Chữ có chuyển động khi phát.
- [ ] Không có "Unable to play media" với clip chữ.
- [ ] Tua đầu, giữa, cuối đều hiển thị chữ.

### G4 — Undo và Redo

1. Bấm vào **clip chữ** trên timeline để chọn nó. Bảng bên phải (**Inspector**) sẽ hiện mục **TEXT CAPTION**.
2. Trong ô nhập có chữ gợi ý "Nhập chữ phụ đề (caption)...", xóa nội dung cũ và gõ: `Sua noi dung test`.
3. Nhấn phím **Enter**. Dòng trạng thái bên cạnh **TEXT CAPTION** sẽ chuyển thành "✓ Active" và khung xem trước hiện chữ mới.
4. Trên **thanh trên cùng** của ứng dụng, bấm nút **Undo** (mũi tên quay lại, tooltip "Undo committed edit").
   - Kết quả: chữ quay lại nội dung cũ.
5. Bấm nút **Redo** (mũi tên tiến, tooltip "Redo committed edit").
   - Kết quả: chữ lại thành `Sua noi dung test`.

Lưu ý: hướng dẫn này chỉ dùng các nút trên thanh trên. Không dùng phím tắt vì chưa được xác minh trong ứng dụng.

---

## Ghi kết quả

Điền vào bảng dưới đây (trong file `CHECKLIST_GUI_RUT_GON.md`, hoặc gửi trực tiếp cho trợ lý):

| Mục | ĐẠT / KHÔNG ĐẠT / CHẶN | Ghi chú hoặc tên file ảnh |
|---|---|---|
| G1 — nhập draft thật | | G1_hop_thoai.png |
| G2 — thư mục không hợp lệ | | G2_loi.png |
| G3 — chuyển động và phát | | |
| G4 — Undo/Redo | | |

**Nếu một bước KHÔNG ĐẠT:** ghi lại đúng điều bạn thấy, chụp ảnh màn hình, và gửi cho trợ lý. Đừng tự sửa gì.

**Nếu không tìm thấy nút hoặc tên không khớp:** chụp ảnh màn hình toàn bộ cửa sổ và gửi cho trợ lý để xác định đúng vị trí.
