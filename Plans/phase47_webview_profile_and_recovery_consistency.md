# Phase 47 — WebView profile recovery and monotonic Undo revisions

**Status:** COMPLETE WITH LIMITS

## Goal

Resolve two reproducible operational defects without deleting user profile or recovery data:

1. The default WebView2 profile produces a black TinixCut window, while the same binary renders correctly with a fresh isolated WebView2 user-data folder.
2. Undo/Redo restores an older project revision, so the autosave recovery compare-and-swap refuses to store the current edited state when a newer recovery snapshot exists.

## Scope

- Establish a stable, app-owned WebView2 profile location that is separate from the existing profile. Preserve the old profile and all its contents for rollback; do not copy/parse WebView2 internal databases.
- Make Undo and Redo produce new monotonically increasing project revisions while restoring the intended document content.
- Verify that unsaved recovery saves after Apply → Undo → Redo reflect each current document state and retain stale-writer protection.
- Keep the Phase 46 disposable media/project and existing recovery candidates intact.

## Acceptance

- The Tauri config/runtime creates and uses the new app-specific WebView2 profile; process command line confirms the new path and native UI renders Core Healthy.
- The existing profile remains present and unchanged.
- Rust regression tests prove Undo and Redo increase revision monotonically and retain content round-trip behavior.
- Recovery regression test saves an applied state, an undone state, and a redone state in order; the last snapshot matches the current project. A stale/lower revision writer remains rejected.
- `cargo fmt --check`, focused Rust tests, required frontend/build validation, and `git diff --check` pass.
- Restart only the workspace-owned TinixCut runtime after final changes; verify the new runtime and Vite health without stopping Paseo or other TinixCut processes.

## Limits

A new WebView2 profile does not migrate Chromium-managed localStorage from the preserved profile. App settings currently stored there may need to be re-entered. The previous profile remains available for rollback. This phase does not claim a root-cause diagnosis inside WebView2's old profile data.
### Implementation evidence — 2026-11-04

- `src-tauri/src/lib.rs` now selects `%LOCALAPPDATA%/com.tinixcut.editor/webview-profile-v2` as `WEBVIEW2_USER_DATA_FOLDER` on Windows only when no explicit override is already present. The previous default profile at `%LOCALAPPDATA%/com.tinixcut.editor` is preserved.
- Native project Undo/Redo now restore document snapshots while assigning the next monotonic revision. This makes revision-based recovery writes after history navigation valid and keeps stale lower-revision writers rejected.
- Regression test `undo_redo_recovery_snapshots_keep_monotonic_revisions` passed; existing `unsaved_recovery_round_trips_and_rejects_stale_overwrite_or_discard` passed. `cargo check --bin tinixcut` and `cargo fmt` passed. `git diff --check` passed with existing LF/CRLF warnings.
- Native restart/screenshot verification completed as recorded below.
### Native verification — 2026-11-04

- Current-source native runtime opened Core Healthy using `C:\Users\Admin\AppData\Local\com.tinixcut.editor\webview-profile-v2\EBWebView`. The previous `...\com.tinixcut.editor\EBWebView` profile remains present. The same workspace runtime used a temporary Cargo target so the healthy Phase 46 instance was not overwritten or stopped.
- On a new empty project, Add Track reached revision 1 and `Recovery saved (rev 1)`. Undo restored zero tracks at revision 2 and `Recovery saved (rev 2)`. Redo restored one track at revision 3 and `Recovery saved (rev 3)`. Screenshot and accessibility tree matched. The existing recovery prompt was dismissed with Later; no existing recovery copy was restored or discarded.
- The native window remained Core Healthy, the WebView2 child command line reported the configured profile path, and Vite returned HTTP 200. Paseo and the other TinixCut instances were left running.
- Rust regressions for monotonic Undo/Redo recovery revisions and stale recovery overwrite protection passed; `cargo check --bin tinixcut`, `cargo fmt`, and `git diff --check` passed.
- Machine-readable evidence: `artifacts/phase47/phase47_native_recovery_evidence.json`.

Limits: the cause of the old profile startup failure was not isolated to a specific WebView2 database/component. The old profile is preserved for rollback; localStorage preferences are not migrated to the new profile and may need to be entered again. The test created/updated a disposable unsaved-project recovery candidate; it was left intact.