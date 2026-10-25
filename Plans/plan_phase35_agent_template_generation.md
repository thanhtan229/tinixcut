# Phase 35 — Agent Tạo và Chỉnh Template

**Status:** PLANNED — chưa triển khai.
**Dependency:** P34 sample gate PASS; P32 schema/library và P33 staging UI ổn định.
**Gate T4:** Một agent thật tạo template chỉnh sửa được end-to-end qua đường mutation chung.

## 1. Mục tiêu

Prompt hoặc reference → proposed typed template → validation/dry-run → preview → apply/save. Agent dùng capabilities/schema của Tinix, không viết renderer code, mô phỏng chuột hoặc sửa file `.tcproj` trực tiếp. Đọc automation/command/idempotency, P32 service contracts, P33 staging và job/cancel/privacy boundaries.

## 2. Slice thực hiện

### 35.1 Schema/capability và agent contract

- Expose supported options, limits, units, built-in reusable primitives, template definition schema và machine-readable errors qua existing CLI/MCP service seam.
- Agent output là typed declarative draft: slots, style/layout, motion, parameters, asset references, provenance và validation issues; reject script/FFmpeg/native shader/unknown fields.
- Reuse inspect/capabilities/dry-run/apply/save/export primitives. Nếu cần thêm read-only template validate/preview/catalog queries hoặc library write transport, reconcile automation ADR/tool scope trước; không tự giữ giới hạn “8 tools” trong docs rồi thêm tools bí mật.
- Provider-independent draft/session contract. External coding agent hiện có qua CLI/MCP là client đầu tiên; không khóa OpenAI/Claude/Gemini model hoặc thêm cloud service vào core.

### 35.2 Generate/refine workflow

- Text prompt → generate draft; reference image/video/CapCut report nếu người dùng chọn → mô tả/dựng lại appearance/motion bằng supported primitives.
- Report assets/font cần thêm, options unsupported và phần agent chỉ tái dựng gần giống; không hứa extraction chính xác từ một ảnh tĩnh.
- Validate limits/resources/schema, normalize units, dry-run dựa current revision, render preview từ cùng compositor. Bounded repair iterations với lỗi rõ; không loop model/render vô hạn.
- Refine draft (“ít bóng hơn”, “chậm hơn”, “đổi màu”) giữ existing user overrides và staged candidate history; chưa commit project mỗi token/response.
- Apply một atomic command transaction hoặc Save definition qua library service; user edit project song song tạo revision conflict và re-dry-run, không overwrite.

### 35.3 UI và resource boundary

- Create with agent entry trong template browser/editor; prompt, attach selected reference, candidate preview và action Apply/Save/Refine/Cancel.
- External-agent client workflow có handoff/return draft rõ để đạt trải nghiệm đầu tiên mà không nhúng một provider SDK bắt buộc vào app.
- Adapter in-app provider chỉ thêm nếu có cấu hình/provider thật được chọn; secrets ở local secure configuration, không project/pack/log. Không biến phase này thành subscription/account/billing project.
- Dùng reference người dùng chọn, không tự upload CapCut cache/media library. Provider network transmission nếu có được mô tả ở thao tác gửi reference; offline external/local agent path không giả cloud generation.
- Jobs bounded/cancellable, stale candidate hoặc timeout không mutate. Long job progress rõ, cancellation giải phóng preview/temp buffers và không để partial project/library save.

### 35.4 Qualification

- Ít nhất một workflow agent thật qua CLI/MCP và staging UI: tạo title tiếng Việt gradient+viền+bóng+character in/out, refine, apply, save pack, reopen và export.
- Workflow thứ hai: multilayer sticker/quote card từ prompt/reference, với resource missing/unsupported handling.
- Same deterministic draft applied through UI/CLI/MCP cho canonical state/evaluation equivalent; generated visual quality có review riêng, không dùng nondeterministic model output làm unit golden.
- Test invalid/oversized/script payload, unsupported option, missing resource, stale revision, duplicate retry, timeout/cancel và failed repair budget.

## 3. Acceptance

- **P35-AC-01:** Agent nhận capability/schema có version, output typed draft; unknown/script payload bị reject trước mutation.
- **P35-AC-02:** Generate/refine/validate/dry-run/preview/apply/save hoàn chỉnh với ít nhất một agent client thật; mock response riêng lẻ không đủ gate.
- **P35-AC-03:** Generated template sửa text/style/motion được, có pack round-trip, render không cần model/agent sau apply.
- **P35-AC-04:** UI/CLI/MCP application dùng same commands, revision/idempotency/undo/redo/persistence equivalence PASS.
- **P35-AC-05:** Reference/provenance/unsupported approximation rõ, asset/resource path không tự thành shell command hoặc unauthorized media upload.
- **P35-AC-06:** Cancel/timeout/stale/repair-limit failures không mutate hoặc leak; secrets không vào project/pack/log.
- **P35-AC-07:** Native candidate/refine/apply/save flow và source/command/export evidence đầy đủ; provider adapter chỉ count khi tested thật.
- **P35-AC-08:** Automation/schema/parity/library/UI regressions, builds/checks/restart/current native runtime PASS.

## 4. Handoff

Lưu deterministic drafts, prompt/reference inventory, adapter/client/version, validation/repair diagnostics, receipts và rendered outputs dưới phase-35; không lưu secrets/private references trong tracked fixtures. User guide phải nói rõ agent tạo native template, imported conversion và baked media khác nhau.

Không đòi chọn vendor trước khi làm schema/CLI-MCP client. Nếu chưa có provider in-app, ghi rõ UI external-agent workflow đang giao; không báo “AI trong app hoàn chỉnh” chỉ vì nút tồn tại.
