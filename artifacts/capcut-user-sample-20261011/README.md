# Thử nhập text template CapCut — 11/10/2026

Nguồn: project `TINIX_CAPCUT_IMPORT_TEST`, template `Variety show sense JP - いざ本番！`, video tham chiếu `C:\Users\Admin\AppData\Local\CapCut\Videos\1011.mp4`.

## Kết quả thử nghiệm

| Project mở trong TinixCut | Sửa nội dung chữ | Giữ kiểu chữ và animation CapCut |
| --- | --- | --- |
| `editable-text-partial.tcproj` | Có | Không: chỉ trích xuất nội dung, thời gian, kích thước canvas và fps; dùng text mặc định của Tinix |
| `baked-video-reference.tcproj` | Không | Có trong các pixel của MP4 đã xuất; đây là video, không phải template chuyển đổi |

Cả hai project được tạo qua CLI/Command Service của Tinix. Dry-run, transactional apply, mở lại và xuất MP4 đều thành công. Nội dung `Tinix — Thử nghiệm tiếng Việt` được giữ nguyên Unicode. Hai bản xuất có 150 frame, 5 giây, 1920×1080, 30 fps. Đã xem ảnh đối chiếu và xác nhận chuyển động từng ký tự trong bản video.

Video CapCut gốc dùng HEVC/yuv420p, không có alpha và chứa nền đen. Không thể dùng trực tiếp như lớp chữ trong suốt trên một video khác. Project video tham chiếu trỏ đến đường dẫn MP4 gốc; giữ file này ở đúng vị trí hoặc relink khi di chuyển.

## Các phần chưa chuyển đổi được

- Font M PLUS 1p ExtraBold và kích thước/layout gốc.
- Gradient trắng–vàng, một viền nâu, hai lớp bóng.
- Animation vào khoảng 1,033333 giây theo từng ký tự, dùng script Lua và runtime Amaz/SDFText của CapCut.

Không chạy script CapCut, không sao chép font vào ứng dụng, không đăng ký reusable template và không thêm UI importer. Không có bằng chứng cho khả năng import đầy đủ mọi pack CapCut từ thử nghiệm một mẫu này.

## Hướng phát triển đề xuất

Xây template chỉnh sửa được theo mô hình Tinix: style chữ phong phú, layout theo ký tự/từ/dòng, animation vào/lặp/ra và cách render thống nhất giữa preview/export. Dùng mẫu này làm phép thử đầu tiên: thay nội dung ngắn/dài/có dấu nhưng vẫn giữ style và nhịp animation.

Sau khi phép thử đó đạt, thêm adapter đọc project CapCut cùng tài nguyên được tham chiếu, chỉ chuyển những style/animation đã có ánh xạ rõ ràng; báo cụ thể phần thiếu tài nguyên hoặc không hỗ trợ. Không nhúng runtime CapCut vào project semantics.

Agent tạo hoặc dựng lại template bằng cùng schema và Command Service của Tinix, có preview trước khi lưu. Mục tiêu đầu tiên là tương đương về hành vi trong phạm vi đã kiểm chứng; không hứa chuyển nguyên mọi hiệu ứng native.

Chi tiết tài nguyên, hash và kết quả kiểm tra nằm trong `sample-analysis.json`. Hai file `*-tinix-export.mp4` là kết quả xuất thật từ Tinix. SSIM toàn khung hình của bản video là 0,999160, bị chi phối bởi nền đen lớn; không dùng con số này để suy ra khả năng chuyển đổi template chỉnh sửa được.

Không sửa mã nguồn hoặc cấu hình ứng dụng. Build và restart server không áp dụng cho thử nghiệm dữ liệu/tài liệu này.
