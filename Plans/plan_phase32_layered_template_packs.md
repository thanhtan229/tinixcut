# Phase 32 — Layered Templates, Parameters và Portable Packs

**Status:** PLANNED — chưa triển khai.
**Dependency:** P31/T1 PASS.
**Gate:** Template nhiều lớp dùng lại được, giữ state/resources khi pack bị xóa.

## 1. Mục tiêu

Nâng từ single text thành template có nhiều text slot, ảnh/sticker và shape; cho phép lưu template và import/export reusable packs. Đọc domain/command/assets/persistence/cache, template registry hiện có, shared renderer và UX error/resource contract.

## 2. Slice thực hiện

### 32.1 Bounded layer model

- Một owning timeline item chứa resolved instance với stable layer/slot IDs: Text, Image và bounded Shape; layer order, layout/anchor/pivot/transform/style/motion.
- Flat finite layers theo limits P27; không nested arbitrary scene graph, constraint solver hay second timeline/reducer. Add schema migration nếu durable state cần, sau architecture reconciliation.
- Resolve image source asset/hash/dimensions qua media boundary; static PNG/WebP và shapes trong phạm vi đầu. Animated sticker dùng existing supported media semantics nếu đạt gate; SVG chỉ được nhận khi có parser/subset được duyệt, không execute SVG script.
- Item-level move/trim/split/delete/undo áp dụng instance đầy đủ; primary text slot tương thích existing text/caption commands. Không lưu cùng primary text/style ở hai nơi authoritative.

### 32.2 Definition và typed parameters

- Versioned definition gồm logical identity, typed slots/parameters/bindings, defaults, responsive policy và resource manifest. Template version immutable; update tạo version mới, existing instances không tự đổi.
- Khi apply, store đủ resolved layer/style/motion/resource refs trong project; registry không là điều kiện để render.
- Bindings chỉ tới properties có kiểu được whitelist; no arbitrary expression/code/property path traversal.
- Parameter edit resolve thành một atomic command transaction. Per-field override/unbind/reset có policy rõ; unrelated user edits được giữ khi apply update.
- Reset cần exact definition version hoặc persisted declarative snapshot nếu contract cho phép; thiếu recipe báo rõ, không resolve sang latest version âm thầm.

### 32.3 Local library và pack format

- Mở rộng registry/catalog hiện có cho bundled + user templates, không thêm DB/service nếu file-based atomic storage đủ.
- User save/edit/delete definition dùng typed application service; project edits vẫn qua Command Service. Library write cũng atomic, có catalog revision/conflict handling, không sửa thẳng JSON từ UI/agent.
- Định dạng pack/version/extension được lock trước implement, dự kiến `.tinixtpl`: manifest + definitions + authorized source assets + optional derived thumbnails. Không chứa executable/runtime/cache semantics.
- Explicit resource policy: bundle font/image khi có quyền; external required assets báo missing/relink. Identity/hash tránh load nhầm font/resource; rename không tự đổi identity.
- Import có schema/version/capability/resource validation, staged installation và atomic catalog commit. Reject unsafe archive paths/symlink, duplicate conflicting IDs, decompression/resource budget overflow, corrupted hashes; limits được nêu cụ thể trong contract.
- Export/import round-trip không phụ thuộc đường dẫn cache máy gửi. Delete/uninstall pack không xóa assets đang được project dùng hoặc invalidate resolved project state.

### 32.4 Curated native pack và extended options

- Giao tối thiểu 12 template gốc Tinix phủ headline/retro/neon/quote/lower-third/sticker card và character/word/line/in-loop-out; tái sử dụng six legacy presets, không tự đổi version/render của chúng.
- Responsive behavior tại bốn canvas ratio: relative anchors, wrap/auto-fit, safe margins, decoration follows text bounds.
- Extended styles/motion đã ghi trong matrix chỉ được thêm khi cùng primitive/evaluator đạt parity; effect chỉ là thumbnail không được count.
- Bộ golden tests representative theo primitive và integration sample cho từng template, tránh nhân mọi style×fps×ratio thành suite vô hạn.

## 3. Acceptance

- **P32-AC-01:** Multi-slot/layer model, z-order/responsive sizing, group lifecycle và renderer parity PASS, không second source of truth.
- **P32-AC-02:** Parameters/overrides/reset/update immutable-version atomic, undo/redo/reopen PASS; không tự chọn latest.
- **P32-AC-03:** Project render được khi remove catalog/pack; source resource missing có relink rõ và không substitute sai.
- **P32-AC-04:** Pack import/export round-trip giữ definitions/resources/versions/hash, offline và không cache paths.
- **P32-AC-05:** Corrupt/unsafe/oversized/conflicting packs bị reject, cancel/failure không partial catalog install hoặc xóa source assets.
- **P32-AC-06:** Ít nhất 12 native templates có editable content, supported animations và fixture evidence, không chỉ preview video.
- **P32-AC-07:** W3 layer/resource workload nằm trong budget; bounded cache/cancellation PASS.
- **P32-AC-08:** Commands/persistence/automation/captions/parity regressions và workflow checks/restart PASS.

## 4. Handoff

Evidence dưới phase-32 gồm pack specification, round-trip fixtures, resource lifecycle, validation/cancel cases và curated catalog render matrix. Update architecture cho durable instance/catalog ownership trước consumer mới. P33 dùng cùng query/service/command contracts; không thêm UI-only template state.
