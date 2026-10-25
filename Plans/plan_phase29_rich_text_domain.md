# Phase 29 — Rich Text Domain, Migration và Commands

**Status:** PLANNED — chưa triển khai.
**Dependency:** P28 PASS và ADR đã reconcile.
**Gate:** Canonical state/Command Service biểu diễn đủ contract, không mất dữ liệu cũ.

## 1. Mục tiêu

Mở rộng model có kiểu cho text/style/layout/motion theo P27, độc lập renderer đã chọn. Đọc domain/time/id, command/history, persistence/migration, composition và automation contracts; tìm callers trước khi đổi shared structs.

## 2. Slice thực hiện

### 29.1 Typed values và invariant

- Rich spans, font reference/face/weight/style, layout, fill/gradient stops, ordered strokes/shadows, background và các primitive được duyệt.
- Animation definition theo unit, in/loop/out, rational offsets/durations, easing, stagger/order, pivot và seed khi cần. Không lưu render frame buffers/glyph handles.
- Validate limits, màu/alpha, finite values, range indexing, ID/resource bindings; reject malformed và inconsistent payload trước commit.
- Reuse item transform và generic easing nơi phù hợp; document thứ tự item transform → layer transform → per-unit animation, không duplicate semantic fields.
- P29 định nghĩa single-text slice trước. Layered instance/pack extension của P32 dùng đúng primitives này; không thêm toàn bộ layer system chưa có consumer vào phase này.

### 29.2 Compatibility và persistence

- Kiểm tra schema hiện tại trước bump. Nếu vẫn v9, migration kế tiếp dự kiến v9→v10; không dành cố định schema numbers cho các phase chưa triển khai.
- Legacy TextStyle/template refs và projects v1–v9 giữ content, timeline, commands/ledger và non-text data. New fields có lossless/default mapping, không tự thêm animation cho preset có tên “pop”.
- Lock legacy rendering policy: không đổi layout cũ âm thầm; nếu cần chuyển từ accepted font fallback sang shared renderer thì expose conversion/migration rõ, có golden legacy fixtures và decision trong ADR.
- File atomic save, recovery và schema rejection giữ nguyên. Fonts/source resources có logical identity; cache paths không canonical.

### 29.3 Command Service

- Extend style/text-set và thêm operations tối thiểu cho rich spans/layout/animation/font binding; tên cuối cùng theo conventions hiện có.
- Atomic apply/reset và typed parameter edits resolve qua cùng engine; mọi UI/CLI/MCP edit dùng same commands.
- `ClipTextSet`/caption track batch với rich content có policy rebind range/style rõ; không để stale span index khi đổi text.
- Preserve revision conflict, transaction rollback, durable idempotency, operation affected scope và undo/redo round trip.
- Validate backend capabilities trước accepted unsupported edits/export theo policy; không serialize native script/string như operation.

### 29.4 Composition và automation contracts

- Carry neutral resolved values vào normalized composition, với schema/capability version rõ trong frontend types.
- CLI/MCP capability discovery/inspect/dry-run hiểu features mới; giữ existing transport, không thêm server/provider runtime.
- Expose semantic errors đủ chẩn đoán missing resource, invalid span, duration conflict, unsupported renderer feature; không fake render success.

## 3. Acceptance

- **P29-AC-01:** Typed validation có success/failure cases; giới hạn hợp lệ, Unicode range không sai byte/UTF-16/grapheme.
- **P29-AC-02:** Legacy migrations và golden data round-trip không mất non-text data/keyframes/ledger; legacy typography policy được kiểm tra.
- **P29-AC-03:** Atomic save/recovery và unsupported/newer schema failure giữ nguyên tính an toàn.
- **P29-AC-04:** Rich text/layout/motion/font commands round-trip undo/redo; transaction invalid rollback toàn bộ.
- **P29-AC-05:** Revision conflict và replay qua save/reopen không duplicate mutation.
- **P29-AC-06:** Content change và caption batch không tạo span lỗi hoặc thay template ngoài scope.
- **P29-AC-07:** Composition/frontend/CLI/MCP nhìn cùng resolved data; renderer-specific state không lọt vào file.
- **P29-AC-08:** Domain/command/persistence/automation regressions, checks/build và runtime verification PASS theo roadmap.

## 4. Handoff

Lưu model/migration/command evidence và legacy fixture diffs dưới phase-29. Update durable architecture chỉ khi triển khai bổ sung decision khác ADR; update checkpoint sau gate. Không bắt đầu picker/importer để che model chưa đạt. P30 nhận canonical rich-text fixture và automation commands có thể áp dụng thật.
