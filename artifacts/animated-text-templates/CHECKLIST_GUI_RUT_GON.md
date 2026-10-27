# Checklist GUI rút gọn — chỉ các bước không tự động hóa được

Tạo ngày 2026-10-27. Mọi bước khác đã có bằng chứng tự động (xem `phase-34/headless-verification-2026-10-27.md` và `phase-31/automated/`).

Quy tắc: làm trên project thử. Chỉ đánh ĐẠT khi tự quan sát được trên màn hình. Ghi kết quả vào cột Kết quả (ĐẠT / KHÔNG ĐẠT / CHẶN).

## Phần 1 — Nhập CapCut (đóng P34-AC-09)

| Bước | Việc cần làm | Kết quả mong đợi | Kết quả |
|---|---|---|---|
| G1 | Mở hộp thoại nhập CapCut, chọn thư mục draft `TINIX_CAPCUT_IMPORT_TEST`. | Hộp thoại hiện kết quả kiểm tra, báo cáo tài nguyên, kiểu chuyển đổi "chuyển đổi đầy đủ". | |
| G2 | Chọn một file không hợp lệ (ví dụ file .txt đổi tên thành draft_content.json). | Hiện thông báo lỗi rõ ràng. Project không thay đổi. | |

## Phần 2 — Mẫu Phase 31 (Gate T1)

| Bước | Việc cần làm | Kết quả mong đợi | Kết quả |
|---|---|---|---|
| G3 | Mở bản sao `phase-31/recreated_sample_project.json` trong ứng dụng. Phát và tua đầu, giữa, cuối. | Chữ có chuyển động trong khung xem trước. Không có "Unable to play media" với clip chữ. | |
| G4 | Trên mẫu đã mở, bấm Undo rồi Redo sau khi sửa nội dung chữ. | Nội dung quay lại khi Undo, áp dụng lại khi Redo. | |

Ghi chú: sai khác về glow và vị trí đường nền đã được chấp nhận hoặc đưa vào roadmap (GAP-07, GAP-08). Không cần kiểm tra lại.
