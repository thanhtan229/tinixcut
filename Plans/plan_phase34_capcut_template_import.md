# Phase 34 — CapCut Template Inspection và Conversion

**Status:** PLANNED — chưa triển khai.
**Dependency:** P33/T2 PASS; native sample recipe và corpus P31 sẵn sàng.
**Gate T3:** Importer chuyển được template thật thành editable animated instance theo supported profile.

## 1. Mục tiêu

Đọc draft/pack và chính tài nguyên được tham chiếu, rồi chuyển style/layout/motion nhận diện được sang Tinix. Đây là adapter, không CapCut runtime và không loader chung cho mọi file seller cung cấp. Đọc nghiên cứu sample, domain/commands, asset/resource/persistence/job boundaries và UX partial-success policy.

## 2. Slice thực hiện

### 34.1 Inspect trước conversion

- Input đầu tiên: CapCut desktop draft folder/JSON đọc được, có resources và optional reference MP4. Native `.tinixtpl` đi đường P32, không trộn converter.
- ZIP pack/draft bundle chỉ mở sau format inspection. Resource-only bundle/preset cần profile riêng; link online, project mã hóa/binary/unknown hoặc installer không được nhận là template hợp lệ.
- Source profile gồm app/draft/resource version khi có, family ID, hashes, layers, effective rich style, animation metadata và missing resource list.
- Resolver dùng selected roots + referenced paths/logical asset map; không quét cả máy hoặc tự download/account scrape. Hỗ trợ relocated bundle, case/path normalization, duplicate ID và source hash mismatch rõ ràng.
- Inspect chỉ đọc; snapshot/hash nguồn, input size/path/archive budgets, progress/cancel và precise diagnostics.

### 34.2 Mapping styles/layout/resources

- Effective style lấy từ content/styles/template resource; không chỉ outer Text fields vốn có thể border=0/shadow=false nhưng native effect vẫn có.
- Convert recognized typography/fill/strokes/shadows/background, native reference canvas/parent+child transform, multi-slot/layer order và duration sang contract Tinix.
- Resolve font/sticker dependencies thành source assets được quản lý; source license metadata giữ khi có, không suy ra quyền redistribute từ việc cache tồn tại.
- No cache path/native resource ID làm renderer instruction; provenance source ID/hash/profile là diagnostics, không requirement render.

### 34.3 Animation adapters

- Adapter theo recognized family/version/fingerprint, dùng declarative data hoặc mapping đã review với fixture. Không execute Lua/JS, không generic interpreter, không regex đoán rồi đánh dấu full.
- Family đầu: sample resource `7498202895196736769` và wrapper `7522343677012774197`; derive style/timing/selection từ source thật. Không hardcode text gốc hoặc simply return recipe P31 bất kể source.
- Validate biến thể text/duration/layout/resource version; resource changed không known thì báo unsupported. Numeric literals chỉ là input của parser/family adapter hữu hạn đã kiểm chứng.
- Thêm tối thiểu hai profile khác dựa trên draft/pack mẫu đọc được (chẳng hạn whole-text transform và multiline/decoration); nếu chưa có source thì không tự tuyên bố hỗ trợ chúng. Không bắt buộc mua pack để coding tiếp.

### 34.4 Report, preview và commit

- Report từng template với hai chiều support và resource availability: `FULL_CONVERSION`, `PARTIAL_CONVERSION`, `UNSUPPORTED_SOURCE`, `MISSING_RESOURCES`; video-only option là `BAKED_MEDIA_ONLY`.
- “Full” nghĩa mọi semantic trong supported profile đã ánh xạ và đạt fixture/visual gate, không cam kết pixel-identical cho mọi CapCut version. Missing font/effect hoặc approximated animation không được gọi full.
- Partial nêu chính xác content/style/layer/motion bị bỏ/thay và cho xem conversion preview. Apply partial là lựa chọn có thông tin; không silent fallback trong final export.
- MP4 là reference/baked media; không dùng làm ảnh nền giả để đạt score hoặc tự gắn vào instance rồi gọi editable conversion.
- Conversion staging không sửa project; apply/save-to-library dùng một transaction/service atomic, source hash/revision được recheck. Reimport có identity/conflict/version policy; cancelled/stale result không commit.
- UI: chọn source → inspect/report → preview/edit mapping khi hỗ trợ → apply hoặc save template. No CapCut UI mouse automation.

### 34.5 Qualification

- Import sample từ source vào project sạch, đổi text ngắn/dài/có dấu, preview/export/reopen, render sau khi đóng CapCut và không dùng MP4 nguồn.
- Source resources bị relocation/missing/corrupt, unknown/encrypted draft, changed Lua fingerprint, archived unsafe paths, cancel và revision conflict có tests.
- Compatibility matrix chỉ ghi các profile/source versions có evidence. Paid pack chưa có file thật không được ghi supported từ trang quảng cáo.

## 3. Acceptance

- **P34-AC-01:** Inspect read-only, source roots/budgets/snapshot/cancel và resource diagnostics PASS.
- **P34-AC-02:** Effective style/transform/resource mapping giữ sample semantics; không dùng outer defaults để bỏ effect.
- **P34-AC-03:** Sample animated template được chuyển tự động từ draft/resource sang native instance; thay text/duration có behavior đúng, không hardcoded fixture substitution.
- **P34-AC-04:** Source/version/hash/family recognition rõ; unknown native script không execute hoặc fake full conversion.
- **P34-AC-05:** Compatibility/report statuses đúng, partial loss list/preview rõ, MP4 không bị quảng cáo là editable template.
- **P34-AC-06:** Atomic apply/save/version handling, undo/redo/reopen/idempotency/stale/cancel giữ project và source an toàn.
- **P34-AC-07:** Imported supported sample render được offline không cần CapCut runtime/cache hoặc reference MP4; source assets còn cần phải được packaged/relinked theo policy.
- **P34-AC-08:** Các profile bổ sung chỉ qua gate khi có source+conversion evidence. Thiếu corpus ghi PARTIALLY QUALIFIED, không tự nhận general-pack support.
- **P34-AC-09:** Native importer workflow, source comparison ROI/frame report, failure tests, regression/build/restart/runtime evidence PASS.

## 4. Dừng và bàn giao

T3 tối thiểu là sample profile converter thật, không chỉ plain-text extraction đã có. Nếu format mới cần semantics chưa supported, quay về evidence/matrix rồi lên scoped extension; không bypass renderer bằng Lua/FFmpeg string hoặc generic untyped payload.

Lưu source hash/profile, resource availability, conversion report/commands, changed-text exports và reference differences dưới phase-34. Profile bổ sung thiếu mẫu là coverage gap, không buộc dừng agent/native-template work khi sample gate và converter infrastructure đã đạt. Giữ gap trong qualification P36.
