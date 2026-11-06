# Phase 48 — Settings, session và độ tin cậy dữ liệu

**Trạng thái:** IN PROGRESS — phần settings và kiểm soát history đã triển khai; gate native chưa đóng.  
**Ngày cập nhật:** 2026-11-04  
**Kế hoạch tổng:** [Product completion — Phases 48–57](plan_product_completion_phases48_57.md)

## Đã triển khai

- Thêm app settings có schema version 1, lưu ở app config directory của Tauri, độc lập với WebView2 profile.
- Settings có đường dẫn Whisper CLI/model, ngôn ngữ, tối đa 20 project gần đây và cờ migration. Token/secret không được lưu.
- Đọc/ghi JSON có validate; ghi qua file tạm và backup thay thế. Nếu file settings hỏng, giữ nguyên file và trả lỗi rõ ràng. Nếu lần ghi trước bị ngắt sau khi chuyển file cũ sang `.bak`, lần đọc kế tiếp dùng được backup.
- Auto Caption chỉ đọc ba key localStorage đã cho phép khi marker migration chưa bật; native settings hiện có được ưu tiên. Không truy cập LevelDB hoặc tự xóa profile cũ.
- Project open/save ghi danh sách gần đây, tối đa 20 và đưa project vừa dùng lên đầu. Lỗi cập nhật recent list được báo sau khi thao tác project thành công.
- Undo/Redo giữ nguyên command ledger và client registry mới nhất, đồng thời tiếp tục tăng revision đơn điệu; history chỉ phục hồi nội dung document.
- Recovery được phân loại bằng so sánh snapshot bỏ qua revision, không chỉ so sánh số revision. Snapshot khác nội dung nhưng revision thấp hơn vẫn được hiển thị như bản recovery cần người dùng xem xét. File recovery không đọc được trả lỗi thay vì bị bỏ qua.
- Khi nạp bản recovery khác nội dung, revision trong bộ nhớ được nâng cao hơn cả revision lưu và revision recovery để lần sửa/lưu tiếp không làm lùi version.
- Recovery hỏng được trả thành trạng thái `RecoveryUnreadable`: file được giữ nguyên, còn người dùng vẫn có thể chọn mở project đã lưu.
- Snapshot recovery dùng compare-and-swap dưới cùng khóa file: revision cũ hơn hoặc nội dung khác ở cùng revision bị từ chối; ghi lại snapshot giống hệt là idempotent.
- Hộp thoại recovery mang theo SHA-256 của snapshot. Restore xác minh lại fingerprint sau khi đọc file, phát hiện thay thế cùng revision trong lúc prompt đang mở.
- Khi restore project đã lưu khác với project unsaved đang mở, engine xóa liên kết cleanup tới recovery unsaved cũ nhưng giữ nguyên file đó. Save project mới không thể xóa recovery của project trước.

## Kiểm tra đã chạy

- Rust settings tests: 3 đạt, gồm round-trip, giữ nguyên file hỏng, validate và danh sách recent bounded/order.
- Rust recovery classification test: đạt, gồm snapshot mới hơn khác nội dung, snapshot cùng nội dung khác revision và snapshot cũ hơn nhưng khác nội dung.
- Rust Undo/Redo/recovery test: đạt; command ledger/client registry còn nguyên và stale writer bị từ chối.
- Regression tests: 6 đạt — recovery hỏng vẫn cho mở project đã lưu và không sửa file; CAS từ chối snapshot cũ/cùng revision khác nội dung; restore project B giữ nguyên recovery unsaved của A; fingerprint từ chối thay nội dung cùng revision và ổn định qua 8 lần đọc project có 4 client trong HashMap.
- Rust `cargo check --bin tinixcut`: đạt.
- `cargo fmt --check`: đạt.
- Frontend typecheck: đạt; Vitest: 97/97; production build: đạt.
- `git diff --check`: đạt; Git có cảnh báo chuẩn hóa LF/CRLF trên các thay đổi tồn đọng.

## Còn mở / cần xử lý

- Native editor basic history/recovery smoke passed on 2026-11-04; it supersedes earlier notes that no window/accessibility observation was available. Settings UI, corrupted recovery prompt behavior, saved-project round-trip, and cross-project recovery preservation remain unverified.
- Legacy localStorage migration được thực hiện khi Auto Caption dialog mở và chỉ trong WebView profile đang hoạt động. Profile cũ đã lỗi không được tự phục hồi; người dùng cần nhập lại/import thiết lập sau này.

## Trạng thái gate

**G48 OPEN — implementation, automated checks and a basic native history/recovery smoke passed.** Full native settings and saved-project recovery workflows remain unverified. No original sample project was opened or modified.

### Native editor smoke — 2026-11-04

- Restarted workspace candidate PID 30056 rendered TinixCut with `Core Healthy (v0.1.0)`. Closed the unsaved-recovery prompt using `Later`; no Restore or Discard action was invoked.
- On the empty `Untitled Project`, UI Automation invoked `+ Track` (rev 1), Undo (zero tracks, rev 2), Redo (Track 1, rev 3), then Undo (zero tracks, rev 4). UI reported `Recovery saved (rev 4)`; the resulting disposable recovery file is `proj_31072ce9df3243608e22d42480ebacd9.unsaved.tinix`. Existing recovery entries were left untouched.
- This confirms the basic native history/recovery surface on a fresh empty project. Settings UI, persisted save/reopen and opening a saved project while preserving another project's unsaved recovery are still unverified; G48 remains open.

### Recovery-candidate disambiguation — 2026-11-05

- Recovery list candidates now include media, track and clip counts plus up to three distinct item/media labels. Labels are reduced to leaf names, capped at 64 characters each, and no source paths are returned. The summary stays ephemeral in the Tauri command response; it does not change persisted project schema or recovery files.
- Rust regression coverage confirms counts, three-label cap and path removal; existing recovery CAS/round-trip test also passes. `cargo check --bin tinixcut`, `cargo fmt --check`, `pnpm exec tsc --noEmit`, frontend suite (**28 files / 164 tests**), `pnpm build` and `git diff --check` pass. Build retains the known >500 kB chunk advisory.
- Workspace Tauri watcher rebuilt/relaunched PID 36608 as PID **35656** after the Rust change; PID 35656 started at 01:01:54. Vite PID **4292** serves `http://localhost:1420/` (HTTP 200). Native screenshot after restart shows `Core Healthy (v0.1.0)` and the new candidate details: e.g. `2 tracks · 4 clips · 2 media files` with the Phase 56/57 clip labels, no paths. Evidence: `../artifacts/phase54/native_ui/phase54_workspace_native_window_recovery_summary_20261105.png` (SHA-256 `211349DC31B4B6D8CF6EE059580A4D80D72D938D5BAB9788693543891257B8F2`).
- Invoked the single enabled UI Automation `Later` button, whose handler only hides the recovery prompt. All 11 `*.unsaved.tinix` files retained identical SHA-256 hashes before and after; no Restore/Discard or project edit occurred. The native editor returned to empty Untitled Project (rev 0), still `Core Healthy`. Evidence: `../artifacts/phase54/native_ui/phase54_workspace_native_editor_after_later_20261105.png` (SHA-256 `891B6713CE99281A166DEB85F9502BBD9291A1F71DE0553968BDC102D8237F30`).
- The screenshot/UIA check proves the native recovery-list IPC populated and that dismissing the prompt preserved all recovery files. It does not close settings persistence, saved project round-trip/recovery, or cross-project recovery preservation. **G48 remains OPEN.**

### G48 WebView session rehydration — 2026-11-06

- Observed on the disposable Phase 51 fixture after frontend reload: the backend retained a canonical project path while React initialized its path and saved revision as null. Recovery attempted the unsaved-project endpoint and the UI reported "Recovery save failed: Project already has a canonical path; use its project recovery snapshot". Source review also shows New/Open could be blocked by the same route and Save could prompt for a new path. The exact prior reload timing is diagnostic context; no new native before/after reproduction was obtained in this check.
- Added a read-only native session command that reads project, canonical path and persisted revision under one engine lock. Frontend startup hydrates all three before enabling editing, shortcuts or autosave; errors have an explicit retry. Existing CAS and recovery preservation remain in force. Offline-media inspection failure is reported separately.
- Reviewed exact candidate independently: QA read-only review PASS and code reviewer PASS for the final file hashes. On integrated workspace, frontend build/typecheck and 177/177 frontend tests passed; Rust formatting, 12 recovery tests and the new session receipt test passed. The Vite build retained its existing chunk-size warning.
- Restarted workspace-owned TinixCut PID 9992 and Vite PID 31836 as PID 11720 and PID 40304. New TinixCut process began after the source change and displayed Core Healthy; Vite returned HTTP 200 and served App.tsx containing the new session API call. Restart ended the unsaved mute edit on the disposable G51 fixture; its on-disk SHA-256 stayed 5C319A548FBBBF3993E60328D86DEB6E4D99A7FAA6D5931B31B265A1F4E7ED49.
- Created a separate empty MCP fixture at artifacts/phase48/disposable/recovery_session_60bd05194785435685c866cb4fc9e007.tinix, project ID proj_b5138aa6c3284fab93e7a0372b6cfed6, revision 0, SHA-256 1993D89166383084D2DAA7474239480B1FA828C48274E89FAEB9608D9FCB959E. The restart opened the existing unsaved-recovery chooser. A screenshot-backed scroll toward Later left the dialog unchanged; input was stopped, with no Restore or Discard. Therefore the saved-project edit → WebView reload → recovery snapshot/direct Save native scenario remains OPEN, as do the other G48 native gates.

### Native session rehydration verification ? 2026-11-06

- Supersedes the earlier note that the recovery chooser remained open: its Later button was reached and selected; existing recovery copies were retained.
- Opened disposable `artifacts/phase48/disposable/recovery_session_60bd05194785435685c866cb4fc9e007.tinix`, added one track (revision 1), and observed `Recovery saved (rev 1)`. After Ctrl+R reloaded the WebView, the same project, revision and track remained visible, with `Recovery saved (rev 1)` and no canonical-path recovery error observed.
- Dismissed the old recovery chooser with Later, then Save reported an atomic save to the same disposable path without a Save As dialog. The file reads back revision 1 and one track; SHA-256 `D8791E0E5FCA352501BC25E59CB948D0D189F3884ADE78ECE5C25A045119D4AD`. This is one native saved-project scenario, not complete G48 qualification.
