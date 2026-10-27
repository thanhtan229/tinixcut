# Checklist kiểm thử native — P34-AC-09 và Phase 31 (Gate T1)

**Trạng thái:** CHƯA THỰC HIỆN. Tạo ngày 2026-10-27.
**Mục đích:** Kiểm tra trên ứng dụng TinixCut đang chạy trên máy Windows. Hai phần này là phần còn lại chưa được xác nhận sau Phase 26.

## Quy tắc

- Chỉ đánh **ĐẠT** khi bạn tự quan sát được kết quả trên màn hình.
- Nếu không làm được hoặc không quan sát được, đánh **CHẶN**.
- Nếu kết quả không đúng mong đợi, đánh **KHÔNG ĐẠT** và ghi lại mô tả lỗi.
- Luôn làm việc trên **project thử** (tạo mới hoặc bản sao). Không mở hoặc lưu đè project gốc.
- Không sửa file mẫu CapCut gốc.

Ghi kết quả vào cột **Kết quả** (ĐẠT / KHÔNG ĐẠT / CHẶN) và cột **Bằng chứng** (tên ảnh chụp, đường dẫn file MP4, ghi chú).

Lưu bằng chứng vào thư mục:
- Phần A: `artifacts/animated-text-templates/phase-34/native/`
- Phần B: `artifacts/animated-text-templates/phase-31/native/`

---

## Phần A — Nhập template CapCut (P34-AC-09)

Tiêu chí: quy trình nhập native hoạt động, có báo cáo so sánh khung hình, các trường hợp lỗi được xử lý đúng.

| Bước | Việc cần làm | Kết quả mong đợi | Kết quả | Bằng chứng |
|---|---|---|---|---|
| A1 | Mở TinixCut từ bản build hiện tại. | Ứng dụng hiển thị trạng thái "Core Healthy". | | |
| A2 | Tạo project thử mới. | Project trống mở được. | | |
| A3 | Mở hộp thoại nhập CapCut, chọn file mẫu đã ghi trong tài liệu. | Hộp thoại hiển thị kết quả kiểm tra và báo cáo tài nguyên. | | |
| A4 | Đối chiếu kiểu chuyển đổi được báo cáo. | Khớp với `phase-34/source-hashes-and-profile.json` (chuyển đổi đầy đủ, kiểu `StaggeredCharacterIn`). | | |
| A5 | Áp dụng template vào project thử. | Có clip chữ trên timeline, không có thông báo lỗi. | | |
| A6 | Sửa nội dung chữ, rồi bấm Undo, sau đó Redo. | Nội dung quay lại khi Undo và áp dụng lại khi Redo. | | |
| A7 | Lưu project, đóng, mở lại. | Template và nội dung đã sửa vẫn còn nguyên. | | |
| A8 | Xuất một đoạn ngắn ra file MP4. | Xuất thành công, file MP4 phát được. | | |
| A9 | Thử chọn file không hợp lệ hoặc không hỗ trợ. | Có thông báo lỗi rõ ràng; project không thay đổi. | | |
| A10 | So sánh khung hình xuất ra với ảnh mẫu tại các mốc thời gian trong `phase-34/timing_frame_report.json`. | Ghi lại sự khác biệt trong báo cáo. Chỉ ghi "tương đương" nếu đạt ngưỡng ROI. | | |

---

## Phần B — Mẫu tái dựng Phase 31 (Gate T1)

Tiêu chí: mẫu tái dựng hiển thị đúng chuyển động, sửa được nội dung, xuất được file. Phải xác nhận được việc sửa nội dung, xem trước và xuất.

| Bước | Việc cần làm | Kết quả mong đợi | Kết quả | Bằng chứng |
|---|---|---|---|---|
| B1 | Mở bản sao của project mẫu `phase-31/recreated_sample_project.json`. | Clip chữ hiển thị đúng animation trong khung xem trước. | | |
| B2 | Phát, rồi tua đến đầu, giữa và cuối timeline. | Chuyển động hiển thị đầy đủ. Không bị đứng hình hay báo "Unable to play media" với clip chữ. | | |
| B3 | Đổi nội dung: một chuỗi ngắn, một chuỗi dài, một chuỗi có dấu tiếng Việt. | Mỗi lần đổi đều hiển thị, không treo ứng dụng, chuyển động vẫn giữ nguyên. | | |
| B4 | Xuất mẫu ra file MP4. | Có file MP4, các khung hình có chuyển động. | | |
| B5 | So sánh khung hình xuất ra với `phase-31/frame_t000.png`, `frame_t_mid.png`, `frame_t_end.png`. | Ghi lại khác biệt trong `differences_vs_reference.md`. | | |
| B6 | Thực hiện Undo/Redo, rồi lưu và mở lại project. | Thay đổi nội dung được giữ nguyên. | | |

---

## Tổng kết

- Phần A: ____ / 10 mục ĐẠT
- Phần B: ____ / 6 mục ĐẠT
- Mục KHÔNG ĐẠT hoặc CHẶN: ______________________________
- Ngày thực hiện: ____________
- Người thực hiện: ____________

Gửi lại cho trợ lý kết quả từng bước (ĐẠT / KHÔNG ĐẠT / CHẶN) và đường dẫn bằng chứng. Trợ lý sẽ cập nhật tài liệu trạng thái sau khi có đủ kết quả.
