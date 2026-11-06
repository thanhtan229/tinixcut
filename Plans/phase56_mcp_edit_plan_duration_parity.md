# Phase 56 — MCP edit-plan duration parity

**Status:** PASS (2026-11-05); automated saved-project ownership/stale-writer protection and native conflict → Save a Copy → reopen are verified. Native artifact viewing and export decode evidence are complete.

## Delivered

- `tinixcut_edit_plan_scene_draft` accepts optional `targetDurationFrames` and `exactDuration` fields.
- MCP calls the same Rust scene builder and exact frame scheduler as the native scene draft UI; no MCP-specific edit reducer or scheduler was added.
- The tool remains read-only and returns a review plan. Applying it remains a separate Command Service call.
- Omitting both fields preserves the existing full selected-range behavior. Infeasible exact targets return a structured tool error.
- Added `tinixcut_workflow_manifest_create`: validates a plan against the loaded project, stores plan hash/schema, project revision, unique operation IDs, input fingerprints and caller-specified artifact paths. It omits brief text, credentials, media bytes and source paths.
- Added read-only `tinixcut_workflow_manifest_validate` so clients can re-read a persisted manifest and check state, revision, exact plan hash/operation identities and media fingerprints before retrying. It returns an explicit `WORKFLOW_RESUME_REJECTED` error on stale or changed inputs and never applies the plan.
- Phase 56 added its checkpoint to the then-current 31-tool inventory. Phase 57 later added the read-only export preflight, bringing MCP discovery/schema coverage to 32 tools.
- Manifest creation is no-clobber. It writes and flushes a temporary file, then creates the destination by same-volume hard link; an existing destination returns an explicit error.
- `validate_resume` requires the planned state, same project/revision, exact plan hash and operation IDs, and unchanged referenced-input fingerprints. A changed revision, plan or media input is rejected instead of replaying edits.

## Verification

- Phase 15 MCP acceptance scenario passed with an exact 240-frame request, two source scenes, and two 120-frame timeline clips.
- The acceptance scenario verifies project revision, tracks and assets remain unchanged.
- Scheduler unit tests: 2/2; `cargo check --bin tinixcut` passes.
- Manifest unit tests: 2/2 for non-sensitive serialization, stale revision rejection and no-overwrite save behavior.
- MCP acceptance verifies manifest persistence, omitted brief/source path, artifact path, collision refusal, successful resume validation and rejection of a modified plan; the project remains unchanged.
- MCP discovery inventory passes for exactly 30 named tools; MCP JSON Schema 2020-12/camel-case acceptance also passes with the added validator schema.
- Restarted the exact workspace `pnpm tauri dev` runtime after the native change: Tauri CLI PID 37532, Vite PID 34040, new workspace `tinixcut.exe` PID 32292 (14:33:56); `localhost:1420` returns HTTP 200. Startup still logs `Cannot read properties of undefined (reading 'invoke')` for project/health/preview IPC. A separate pre-existing TinixCut PID 33680 was left untouched. Native UI and MCP-to-UI ownership/revision conflicts remain unverified.

## Checkpoint extension — 2026-11-04

- Added `tinixcut_workflow_manifest_checkpoint` to MCP. The immutable sidecar sequence is `applied → saved → exported`; each step must match the manifest, exact plan hash, operation IDs, source fingerprints, project identity and expected revision.
- `applied` requires every planned operation to appear in the durable command ledger at one common revision newer than the base revision. `saved` requires an applied sidecar and the matching revision loaded from the saved project file. `exported` requires the matching saved checkpoint and a declared artifact that ffprobe can read with a positive duration.
- Export evidence records artifact SHA-256, byte size, video codec, dimensions and duration. Checkpoints use atomic no-clobber sidecars; a retry succeeds only when evidence is identical. A different valid export at the same declared path is rejected without replacing the prior checkpoint.
- The workflow acceptance suite includes the checkpoint in exact-name/schema discovery. A focused Rust lifecycle test exercises ordering, durable apply/save state, real FFmpeg artifact probing, idempotent retry and collision rejection.
- Full real MCP stdio acceptance performs manifest → apply/save → all checkpoints, closes stdin, restarts MCP and retries the export checkpoint. It returns `alreadyRecorded=true` and leaves exactly one project mutation. The latest full `phase15_mcp_acceptance`: **45 passed, 0 failed, 3 ignored**; ignored cases require explicitly started local LM Studio.
- Real-stdio batch acceptance now covers existing and new projects. The existing-project batch proves a per-project invalid request and output collision do not stop other items; two projects render successfully and pass FFprobe/full decode. `test_p56_real_stdio_new_project_stale_revision_recovers_and_exports` creates a project through MCP, imports a real fixture through Command Service, rejects a stale revision without mutation, resumes at the current revision, checkpoints apply/save/export and full-decodes the result. Combined `phase15_mcp_acceptance`: **47 passed, 0 failed, 3 ignored**.
- `test_p56_real_stdio_batch_cancel_isolated_and_next_project_exports` now proves a 60-second batch item can be cancelled during active FFmpeg rendering, its temporary file is cleaned and no output is promoted, then a second independent project exports successfully in the same MCP stdio process and passes full FFmpeg decode. The two saved projects remain unchanged by export. Evidence: [batch cancellation](../artifacts/phase56/phase56_batch_cancel_isolation_evidence.json). Latest combined `phase15_mcp_acceptance`: **49 passed, 0 failed, 3 ignored**.

## Remaining G56

- Batch subprocess acceptance covers three existing projects in one MCP stdio process: an invalid project request is isolated; one pre-existing output is reported and preserved; the other two projects apply/save/export and reach durable checkpoints. Both resulting H.264/AAC 1920×1080 30 fps one-second MP4s pass FFprobe and full FFmpeg decode. `test_p56_real_stdio_new_project_stale_revision_recovers_and_exports` separately proves new-project creation/import and recovery from a stale revision. Evidence: [batch artifacts](../artifacts/phase56/phase56_batch_acceptance_evidence.json) and [new-project workflow](../artifacts/phase56/phase56_new_project_acceptance_evidence.json).
- **Ownership contract (automated PASS):** MCP addresses an explicit saved `projectPath` and reloads that durable snapshot for each call; it has no handle to the UI's in-memory dirty state. Desktop edits remain in memory until save. If MCP commits a newer disk revision first, the UI's stale save is rejected by `persisted_revision` compare-and-swap and the dirty UI snapshot remains available for Save As/reload. This is the intended conflict behavior; it does not merge edits or provide live collaboration, so no session broker is introduced.
- New and existing project creation, stale-revision recovery, per-project failure isolation, collision handling, cancellation during a batch, EOF shutdown and identical checkpoint retry after a fresh process restart pass. Native UI observation is still required to verify the user-facing conflict message and Save As/reload recovery path.
- Verify final artifacts in the native editor and with ffprobe/full decode.

### Latest automated continuation — 2026-11-04

- Added and passed real-stdio multi-project cancellation isolation. A long first export is cancelled only after temporary rendering begins; cleanup removes the temporary file without promoting an output. The same MCP process then completes and full-decodes the next project's export. Evidence: [batch cancellation](../artifacts/phase56/phase56_batch_cancel_isolation_evidence.json).
- Full MCP integration acceptance now reports **49 passed, 0 failed, 3 ignored**. The ignored tests require an explicitly started local LM Studio model.

## Next action

Next: qualify the artifacts in native editor and observe the already-defined UI conflict/recovery path when reliable UI automation is available. New/existing project, saved-project ownership contract, stale-revision recovery, process/retry, cancellation isolation and collision acceptance now pass automatically; native editor UI verification remains deferred to the final environment-blocker pass.

### Saved-project versus dirty UI ownership — 2026-11-04

- Source inspection confirms `project_execute_command` keeps edits in the desktop engine; `project_save` uses its last `persisted_revision` with `save_project_atomic_if_revision`. MCP `tinixcut_apply` reloads the explicit disk path and persists by the same cross-process revision-guarded atomic writer.
- Re-ran `test_p15_ac25_stale_revision_rejection` (1/1), `test_revision_guard_rejects_stale_writer_and_preserves_newer_project` (1/1), and `phase37_cross_process_persistence` (2/2). The stale UI snapshot cannot replace the newer MCP revision; the disk project retains the MCP update.
- This verifies the persistence conflict contract, not the native error dialog or user's recovery interaction. Those native UI checks remain open with the other G56 artifact-openability checks. Evidence: [ownership contract](../artifacts/phase56/phase56_saved_project_ownership_evidence.json).

### Phase 55 tool-contract extension — 2026-11-04

- Scene draft MCP description and JSON Schema now disclose optional canvas and title-template inputs. The schema acceptance test follows the nullable `$ref` definitions and checks the four canvas and six template enum values, plus `openingTitle` presence.
- Scene draft MCP acceptance verifies a reviewed 9:16/title plan, exact frame duration, and rejection of an incomplete title/template pair without project mutation.

### Phase 55 source policy and brief-intent MCP contract — 2026-11-04

- Both scene suggestion and draft tools expose optional `allowedAssetIds`; empty preserves legacy all-source behavior. The suggestion tool's schema also exposes optional `captionIntent` and `audioIntent` strings.
- `tinixcut_edit_plan_scene_selection_suggest` sends the model only allowed scenes and records the guidance in its read-only response. It explicitly does not create caption/audio operations.
- MCP acceptance verifies the draft tool discovers the source allowlist field, the suggestion tool discovers both intent fields, unknown asset allowlists fail closed, and the valid exact-duration plan still leaves the project unchanged.
- Schema 2020-12 and scene-draft acceptance tests pass after these optional additions.

### Final current-worktree regression run — 2026-11-04

- Ran `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance` with an isolated `%TEMP%\tinixcut-phase50-57-final-target` build directory: **49 passed, 0 failed, 3 ignored**. The ignored cases require a deliberately started local LM Studio server/model. Coverage includes G56 real stdio batch/checkpoint flows and G57 preflight/cancel flows.
- Ran `pnpm run typecheck`: pass; `pnpm test -- --reporter=dot`: **25 files / 150 tests passed**; `pnpm run build`: pass (existing >500 kB chunk warning); `cargo fmt -- --check`: pass; `git diff --check`: pass.
- These are automated/current-worktree checks. They do not close native UI, exact installed UI, cross-version upgrade, disk-full I/O, or independent editorial-review gates. No application source changed in this verification slice.
### Cross-process write protection recheck — 2026-11-04

- Ran `cargo test --manifest-path src-tauri/Cargo.toml --test phase37_cross_process_persistence` in the isolated final target: **2 passed, 0 failed**. The child-process stale snapshot was rejected; persisted project remained at the newer revision/name. This strengthens G56 saved-project concurrency evidence, but does not cover the unsaved project held only in a native UI process.

### Phase 56 persistent export verification — 2026-11-04

The latest three-project batch and new-project stdio outputs were rechecked from the preserved temporary run. SHA-256 matched the existing acceptance receipts for all three MP4s; `ffprobe` reports H.264/AAC, 1920×1080, 30 fps, 30 frames and 1 second, and full FFmpeg decode exited 0. Copies are retained under `artifacts/phase56/verified_exports/`; per-file evidence is in `artifacts/phase56/phase56_persistent_export_verification.json`.

The native-editor open check and native unsaved-project conflict/Save As/reload flow remain **OPEN**. This session has no Computer Use tool, so no native UI outcome is claimed. G56 is not marked PASS until those required native observations are available.

### Native artifact open probe — 2026-11-04

A read-only UIA inspection found two TinixCut windows: the active Phase 54 fixture and a separate `Untitled Project` (PID 33680). The artifact import dialog was opened only on the blank project. `SendKeys` mangled the absolute path (`D:\___TOLS__TINIX\TinixCut\atifactsphae56veifiedeportsbatch_output_0.mp4`) and Windows returned `Windows can't find ...`. The dialog was closed with UIA `WindowPattern.Close`; the blank project reports `No media imported.` No project was saved or changed. Evidence is appended to `artifacts/phase56/phase56_persistent_export_verification.json`. Native opening remains BLOCKED/unverified; the successful ffprobe/full-decode checks do not substitute for it.

### Native open follow-up — 2026-11-04

A separate UIA/Win32 attempt set the file picker filename ComboBoxEx32 field to the exact verified absolute path and read the same path back byte-for-byte. Sending the dialog `IDOK` command left the Import Media Files dialog open and the media library still reported `No media imported`. I closed the dialog with UIA `WindowPattern.Close`; the blank project remains unchanged. G56 native artifact opening remains unverified. The readback/action result is recorded in `artifacts/phase56/phase56_persistent_export_verification.json`.

### Native picker activation diagnosis — 2026-11-04

The exact path was read back from the filename ComboBoxEx32 and its child combo. `WM_COMMAND(IDOK)`, `BM_CLICK`, and a foreground click at the UIA-reported Open bounds all left the picker open and the library empty. The dialog was closed; no project was saved or modified. This establishes a session-level native-input activation blocker, not a TinixCut import rejection (the app never observed a selected file). Computer Use is unavailable here. Full attempt log: `artifacts/phase56/phase56_persistent_export_verification.json`.

### Native source-viewer open — 2026-11-04

- Copied the verified `batch_output_0.mp4` to the disposable Phase 54 fixture directory; source and copy SHA-256 both equal `22FB8453BE91BDF0CD88F709CC57D1F7D9560F7818CFD6B39BEDB0B5BAA3DD8B`.
- In the current candidate's native Import Media Files picker, the exact disposable copy was listed and invoked through its unique UIA ListItem. TinixCut reported `Imported 1 of 1 media file(s)`; Media Library identified it as Video, 1920×1080.
- Invoked Preview on that exact video row. The native Source Viewer opened for `phase56_verified_batch_output_0.mp4`; a captured UI frame showed a solid green image matching frame 0 extracted by FFmpeg from the canonical verified export. The viewer was closed with its Close button; the project remains disposable and unsaved. The canonical verified MP4 was not modified.
- This closes native import/render observation for one artifact; G56 still needs the remaining distinct artifact review and the dirty-project conflict/Save As/reload flow.
- Evidence: `phase56_persistent_export_verification.json` and [`native Source Viewer capture`](../artifacts/phase56/native_ui/phase56_verified_batch_output_0_source_viewer.png) (SHA-256 `49F8D1B8CC1B1159A037014F06EB414D74D008D44EBF75FE2351B23B3D514328`).
### Native open of verified MCP exports — 2026-11-04

- In the current disposable project, the native media picker imported all three preserved Phase 56 MP4 artifacts. SHA-256 of each test copy matched its canonical source. The two batch outputs are byte-identical; the new-project output is distinct.
- Native Source Viewer opened both distinct output files. The batch artifact rendered solid green and the new-project artifact rendered solid blue; each captured native frame visually matched FFmpeg-extracted frame 0. The byte-identical second batch artifact was imported, while its separate preview was unnecessary to distinguish content.
- Project recovery revision reached 5 and remains unsaved. No canonical export artifact or source fixture was modified.
- This closes the native artifact-import/render portion of G56. The stale UI-save conflict message, safe Save As/reload flow, and persisted dirty-project interaction remain OPEN.
- Evidence: `phase56_persistent_export_verification.json`, plus the two PNG captures under `artifacts/phase56/native_ui/`.
### G56 continuation checkpoint — 2026-11-05

- Re-ran `phase37_cross_process_persistence`: **2 passed, 0 failed**. A stale child-process snapshot cannot replace the newer persisted project revision.
- This reconfirms the automated ownership/CAS contract. It does not exercise the native stale-save conflict message, Save As/reload recovery, or reliable native opening of exported MP4 artifacts; those remain OPEN and are carried to the final native-input blocker pass.
- No project or recovery sidecar was changed by the test.
### Current-worktree MCP acceptance recheck — 2026-11-05

- Re-ran `cargo test --manifest-path src-tauri/Cargo.toml --test phase15_mcp_acceptance` using the isolated target directory: **49 passed, 0 failed, 3 ignored**. The ignored tests explicitly require starting local LM Studio and loading a model; no model service was started by this test run.
- Passing G56 coverage includes `test_p56_mcp_workflow_checkpoints_cover_apply_save_and_export`, real-stdio new-project stale-revision recovery, three-project batch collision/failure isolation, and active-cancel cleanup followed by a successful next export.
- This confirms the saved-project/MCP workflow and artifact automation in the current source tree. It does not close native stale-save message/Save As/reopen acceptance.

### Native unsaved-project revision conflict — 2026-11-05

- Opened only the separate-ID Phase 54 disposable fixture in the workspace TinixCut UI. A native `ProjectMarkerSet` committed in the UI produced an unsaved recovery snapshot at revision 1; the canonical project file remained at its original revision 0.
- A separate workspace `tinixcut mcp` stdio process inspected the same exact project path and applied two marker operations, advancing the disk project revision 0 → 1 → 2. Both calls returned success and the MCP process exited 0.
- Invoking the native toolbar Save produced: `Save error: Project revision conflict: expected disk revision Some(0), found Some(2); reload the project or save as a new file to preserve your edits`.
- The project file remained revision 2 and SHA-256 `2DE8FDDEB405E65ADF7686CED95C2601D50031284B276C5F46246EE4790763BC`; native unsaved edits remain in the recovery file, SHA-256 `ADE09B5B231D7A35AEB84FDC9877C567A7AFDEFAAFB4AFE30EE59CC381FFFFE0`. The zero-byte lock sidecar remains present. No Restore/Discard action was taken. This is a positive conflict-protection/data-preservation result.
- Added a contextual **Save a Copy…** footer action, shown only for this revision-conflict status, routing the current in-memory project through the existing native Save dialog and save command. Predicate regression test passes; full frontend tests/typecheck/build pass. At this checkpoint Computer Use was absent; native activation and reopen were subsequently verified in the follow-up below.
- G56 conflict detection and preservation: **PASS**. At this checkpoint the complete gate awaited native Save a Copy/reopen; that path is verified in the follow-up below. The original test copy and recovery evidence remain preserved.
- Follow-up native Save a Copy/reopen: on a new-ID disposable project, UI revision 3 held a native marker while a separate MCP stdio process advanced the saved file from revision 2 to 3 with a different marker. Native Save surfaced the expected-revision conflict and did not overwrite the MCP file. The conditional action opened the native dialog and wrote `artifacts/phase54/disposable/Phase 54 UIA Disposable 100 Clip - recovered.tinix` at revision 3 with the native marker. The exact new copy was selected through the native project picker and reopened; the editor showed revision 3, Core Healthy, and two preview layers. The conflict source file and earlier Phase 54 project/recovery hashes remained unchanged. Screenshot: `artifacts/phase56/native_ui/phase56_savecopy_reopened_20261105.png`; hashes/process evidence: `artifacts/phase56/phase56_native_conflict_and_restart_evidence_20261105.json`.
- **Gate disposition: G56 PASS.** The native dialog produced its default recovered filename; this does not claim the test harness's attempted programmatic filename override was accepted.

### Toolbar Save event guard regression — 2026-11-05

The shared TopBar callback previously passed the React click event into an App handler whose optional boolean parameter selected Save a Copy, causing ordinary toolbar Save to open the copy dialog. TopBar now receives an explicit zero-argument wrapper. Native verification after restart on a saved disposable scene-draft project confirmed toolbar Save atomically updated the same path without opening a dialog; the source conflict/recovery fixtures remained untouched. This follow-up does not alter the already-qualified G56 stale-writer result.