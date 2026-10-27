# Phase 36 — Integration, Performance và Release Qualification

**Status:** IMPLEMENTED WITH DEFERRED QUALIFICATION - Gate T5 passed for automated/static/package scope; native GUI, clean-VM, and hardware encoder qualification remain deferred.
**Dependency:** P27–P35 gates đạt, coverage gaps có owner rõ.
**Gate T5:** Bộ công cụ template đủ điều kiện sử dụng ổn định, với support matrix và evidence đúng.

## 1. Mục tiêu

Kiểm tra toàn workflow native template → pack → CapCut conversion → agent generation trên Windows, cùng regression editor và packaging. Retain qualification P26/clean-VM/hardware chưa hoàn tất; không reset lịch sử thành PASS. Đọc roadmap, từng phase AC/evidence, P26 checklist, workflow/architecture/parity/job/resource boundaries.

## 2. Slice thực hiện

### 36.1 Integration/regression

- Domain/time/easing, command transaction/revision/idempotency/history, migration/atomic save/recovery, resource relink và remove-cache/reopen.
- Native text/captions legacy và rich templates; import/export SRT/VTT/auto-caption path không bị đổi model/behavior ngoài scope.
- Media playback/audio/mute/seek, visual effects, crossdissolve/dip-to-black, crossfade, fixed clip speed, ratios/export profiles và missing media.
- UI/CLI/MCP equivalence, pack/library ownership, imported instance render khi đóng CapCut, generated instance render khi agent offline.
- Bounded jobs/cancel/supersession, renderer/FFmpeg crash/restart, stale result và output final atomicity.

### 36.2 Performance và stress

- Đo lại W1–W4 của P27 trên host/version đã ghi; cold/warm cache, 30/60fps capability, 16-layer limit, long text tới giới hạn, many library cards và repeated edits/seeks.
- Record p50/p95/end-to-end, dropped frames, export wall time, peak/native/WebView/cache memory và cancellation latency. Không dùng native raster timing để thay total delivery timing.
- Export dài streaming không tăng bộ nhớ/disk theo toàn bộ frame count ngoài output final. Delete cache xong phải regenerate đúng, cancellation không giữ temp buffers vô hạn.
- Có unsupported workload/capability rõ khi vượt scope; không silent bỏ shadow/animation để đạt fps. Existing transport budgets giữ theo architecture/P17.

### 36.3 Windows package và offline resources

- `pnpm tauri build` theo workflow; installer chứa đúng dependencies/notice/fonts có quyền phân phối và resolve resource ở installed path.
- Fresh Windows/clean-VM smoke không có CapCut, font cache máy dev hoặc SDK/env paths; native pack và Tinix fixtures vẫn hoạt động offline.
- User-only imported font/image external assets: portable pack nếu được phép, hoặc clear missing/relink. Không bundle CapCut cache tự động.
- RGBA nội bộ là requirement đã đạt P28/P30. Transparent user export PNG/frame sequence hoặc codec có alpha chỉ công bố khi actual installed encoder/pixel-format round-trip giữ alpha; nếu chưa có thì ghi chưa giao, MP4 H.264/HEVC thông thường không được gọi transparent.
- Hardware encoder/legacy clean-VM gates từ P26 giữ evidence riêng; software encoder pass không thay hardware pass.

### 36.4 Native user qualification

- Create style Việt → character animation → multilayer → save/reuse/version/reset → pack export/import.
- Import sample CapCut → inspect report → preview → apply → đổi text → save/reopen → export trên moving video; partial/unsupported/missing cases có workflow khắc phục.
- Agent prompt/reference → candidate/refine → preview → apply/save → reopen/export; cancel trước commit không sửa project.
- Bốn ratios và relevant fps, undo/redo, playback/seek, missing-font/image relink, xóa pack/cache và export error recovery.
- Reuse P26 checklist cho old editor flows; bổ sung mới, không gắn PASS cho item không được quan sát. Phân biệt user-reported và independent automated/native evidence.

### 36.5 Delivery và tài liệu

- Feature/format/profile compatibility matrix có version/fingerprint/evidence; tách native/recreated/full/partial/baked provenance.
- Hướng dẫn tiếng Việt: tạo template, pack/resource/font, CapCut import supported formats, agent workflow và các bước kiểm lỗi.
- Reconcile architecture/plan/status/workflow/UX khi subject thay đổi; không copy research hoặc test logs đầy đủ vào source code.
- Summary release có exact checks, tests, known gaps và new-runtime health/freshness evidence.

## 3. Acceptance

- **P36-AC-01:** P27–P35 gate ledger đầy đủ, tất cả mandatory features/AC trong matrix đã đạt; không feature chỉ tồn tại ở UI hoặc mock.
- **P36-AC-02:** Full relevant regression/parity/automation/migration/history/recovery suite PASS, timeout/failure pre-existing report rõ.
- **P36-AC-03:** Reference W1–W4 meets locked budgets; stress/cancel/cache không leak hoặc silent degradation.
- **P36-AC-04:** Native template, pack, supported CapCut conversion và real-agent workflow end-to-end có native/user evidence.
- **P36-AC-05:** Installed Windows app offline, source/font/resource lookup và clean-VM qualification đã đo hoặc được ghi deferred đúng mức.
- **P36-AC-06:** Missing/unlicensed-to-bundle resources có explicit packaging/relink policy; alpha/export capabilities được kiểm tra đúng codec/pixel format.
- **P36-AC-07:** P26 regressions/deferred hardware/runtime qualifiers được reconcile mà không tự xóa hoặc giả PASS.
- **P36-AC-08:** Builds/typecheck/format/Clippy/checks, exact owned runtime restart và fresh native health verification PASS.
- **P36-AC-09:** User guide/support matrix/research/architecture/status phản ánh actual implementation và limitation.

## 4. Báo cáo hoàn thành

Lưu final gate ledger dưới `artifacts/animated-text-templates/phase-36/`, liên kết đến evidence phase trước thay vì nhân bản mọi file. Ghi feature, format/profile version, AC, result, automated/native/user source và remediation cho từng gap.

Chỉ dùng `COMPLETE` khi mandatory gates đã đạt. Nếu code đã xong nhưng native/clean-VM/genuine-agent qualification chưa có, dùng `IMPLEMENTED WITH DEFERRED QUALIFICATION`, không báo release-ready. Unsupported CapCut families ngoài support scope là limitation sản phẩm, không được đánh dấu lỗi đã sửa hoặc full compatibility.

Không commit/push/release installer ra bên ngoài tự động. Task coding cuối phải để đúng local runtime mới đang chạy và ghi verification result.
