# Phase 33 — Template Browser và Authoring UI

**Status:** PLANNED — chưa triển khai.
**Dependency:** P32 PASS.
**Gate T2:** Người dùng tự tạo, lưu và dùng lại template phong phú trong native app.

## 1. Mục tiêu

Đưa các option đã kiểm chứng vào workflow dễ dùng, giữ preview/timeline/Inspector hiện hành. Đọc `UX_UI.md` trước UI work, cùng P32 query/command/library contracts và development workflow. Không biến template editor thành app/scene editor thứ hai.

## 2. Slice thực hiện

### 33.1 Browser và thư viện

- Entry Text/Templates trong existing shell; tabs/category, search, favorites/recent, built-in/user/imported filters và named template versions.
- Cards có static thumbnail và animated preview khi hover/select. Chỉ animate items đang visible/active, có global budget/cancel; reduced-motion tắt motion trang trí UI, không đổi animation của project/export.
- Loading/empty/missing/unsupported states cụ thể. No “download” icon giả cho asset local, no premium badge khi không có billing thực.
- Insert template vào playhead/selected track qua commands; default content/duration/layout được preview, undo tạo một coherent entry.

### 33.2 Inspector đầy đủ với progressive disclosure

- Content/slots, font/typography/layout, fill/strokes/shadows/background, layers, animation in/loop/out và reusable parameters.
- Advanced controls grouped/collapsible; option chỉ hiện hoặc enabled theo capability thực. Tên/preset có thumbnail nhưng thiếu backend không được expose như feature hoàn thành.
- Layer selection/reorder/duplicate/show-hide/slot edit dùng stable IDs; text bounds/anchor guides preview-only.
- Motion unit/stagger/order/easing/timing có preview/scrub đúng project time. Duration conflict/missing font có message và thao tác khắc phục cụ thể.
- Pointer/slider state transient, commit một meaningful command khi kết thúc gesture; text edit batch theo policy có undo hợp lý, không mỗi frame IPC/history.

### 33.3 Authoring và quản lý version

- Flow: tạo Text/instance → chỉnh → Save as template → đặt tên/category/parameters → library preview → dùng lại.
- Save as new version khác replace existing instance; thay definition không tự cập nhật project khác. “Apply version/update” hiển thị scope và override policy trước mutation.
- Reset từng group/all theo recipe version có sẵn; missing recipe rõ ràng.
- Import/export native pack, resource relink, duplicate, local delete/uninstall dùng services của P32. Project reference không bị phá khi delete library entry.
- Draft authoring staging có trạng thái dirty/saved/error; không merge half-complete fields thành canonical project ngoài Command Service.

### 33.4 Native qualification

- Keyboard focus/navigation, labels, clear errors, small-window resizing; browser không đẩy mất preview/Inspector.
- Người dùng làm một template Việt gradient/viền/bóng/stagger, một template text+sticker, save pack/reimport và reuse ở 9:16.
- Confirm undo/redo/reopen/reset/version selection, playback while editing, cancel và missing-font recovery.

## 3. Acceptance

- **P33-AC-01:** Browser search/filter/favorite/preview/insert hoạt động với real catalog, không placeholder backend.
- **P33-AC-02:** Mọi core/extended option được đánh dấu supported trong matrix có UI chỉnh được hoặc binding rõ, không field frontend-only.
- **P33-AC-03:** Native create/edit/save/reuse/export-pack flow hoàn chỉnh, version/override/reset behavior đúng P32.
- **P33-AC-04:** Keyboard/accessibility/loading/empty/error/missing-resource states và responsive editor layout theo UX standard.
- **P33-AC-05:** Gesture/history batching, revision conflict và rollback không duplicate edits hoặc mất user changes.
- **P33-AC-06:** Animated card virtualization/budget/cancel và authoring performance không gây playback freeze/seek regression.
- **P33-AC-07:** User/native evidence chứng minh hai authoring scenarios, undo/redo/save/reopen/preview/export PASS.
- **P33-AC-08:** Relevant frontend tests, core regressions, build/checks/restart/native health PASS.

## 4. Bàn giao

Evidence screenshots/video walkthrough, command receipts, UI error cases và authoring fixtures dưới phase-33. Không tạo màn hình importer/agent “đã hoạt động” trước các phase tương ứng. P34/P35 sẽ gắn vào browser/staging flow hiện có.
