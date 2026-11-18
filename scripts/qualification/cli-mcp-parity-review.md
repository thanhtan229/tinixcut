# CLI and former MCP parity review

Baseline: CLI at `272bf269` with qualification-only build changes; MCP built from `98dbbf95` (parent of removal commit `8267342c`). Comparisons ran on Windows in background. MCP has already been removed from master. The 36 names and input schemas matched exactly. MCP output schemas and tool annotations are absent from CLI discovery, and one-shot CLI exits nonzero for a reported operation error.

Status meanings: **paired** means successful calls through both adapters using the same input and compared results, subject to the stated scope. **partial** means evidence is limited to error handling, one adapter, static schema, or a narrower prior run. Empty-argument checks compared content, structured content and error flag for 35 commands; `export_start` needs a persistent session and was excluded from that check. They do not establish success-path parity.

| Operation | Evidence and remaining gap |
|---|---|
| project_create | partial — CLI success; paired missing-argument error. Compare actual creates on independent copies. |
| project_variant | partial — paired empty-argument error; compare successful clone and stale/conflicting destination. |
| project_compare | partial — paired empty-argument error; compare successful same/different projects. |
| audio_align | partial — paired empty-argument error; compare successful aligned and ambiguous audio fixtures. |
| project_chapters | partial — paired empty-argument error; compare chapter extraction with markers. |
| frame_scopes | paired — same video fixture, identical structured result; additional frame/color cases open. |
| lut_probe | partial — paired empty-argument error; compare valid and invalid LUT files. |
| media_probe | paired — same WAV and same video metadata; more codecs open. |
| edit_plan_validate | paired — valid track plan, identical structured result; stale/invalid cases open. |
| edit_plan_draft | paired — deterministic draft on same media, identical structured result; model/error variants open. |
| edit_plan_scene_draft | paired — same local visual report; generated IDs differ, remaining observed fields match; broader constraints open. |
| edit_plan_scene_selection_suggest | paired — real local Gemma selected same scene; generated rationale differed; other constraints open. |
| edit_plan_reorder | paired — caller-order two-clip success, identical structured result; local-model order open. |
| edit_plan_trim | paired — success, identical structured result; boundary variants open. |
| edit_plan_volume | paired — success, identical structured result; bounds and locks open. |
| edit_plan_canvas | paired — success, identical structured result; bounds open. |
| edit_plan_text_replace | paired — Unicode text fixture, identical structured result; broader selection cases open. |
| media_scene_candidates | paired — same generated video, identical structured result; threshold and cancellation variants open. |
| media_visual_scenes | paired — independent caches, real local Gemma on same video, identical structured result; provider failures and active cancellation open. |
| media_transcript | paired — independent caches, real Whisper/model and speech WAV, identical structured result; language/model failures and active cancellation open. |
| inspect | paired — same saved project, identical structured result; offline fixtures open. |
| capabilities | paired — identical content and structured result. |
| dry_run | partial — paired empty-argument error; compare valid and invalid envelopes. |
| apply | paired — same TrackCreate envelope on independent project copies, receipt matched after excluding generated paths/IDs; more operations and conflicts open. |
| save | partial — CLI success and paired empty-argument error; compare save-as/conflict. |
| workflow_manifest_create | paired — same plan on independent paths, matched after excluding generated workflow ID/path; other plans open. |
| workflow_manifest_validate | paired — same planned state, matched after excluding generated workflow ID; stale cases open. |
| workflow_manifest_checkpoint | paired — applied/saved/exported checkpoints matched except generated workflow ID/path; retry/conflict open. |
| export_preflight | partial — paired empty-argument error; compare valid readiness and output failure. |
| export_start | paired — same 2s project exported H.264/AAC; output SHA-256 identical and decoded successfully; more codecs and hardware open. |
| export_status | paired — completed and cancelled states observed; progress, failure and concurrency open. |
| export_cancel | paired — same 60s fixture: active temporary render observed before cancel, status cancelled, output and project retained, temp cleaned; repeated/race variants open. |
| template_validate | paired — Vietnamese draft valid and structured result identical; malformed/resource cases open. |
| template_preview | paired — same draft, identical PNG SHA-256; timing and dimensions variants open. |
| template_apply | paired — same text project copies, receipts match after generated values, resulting tracks identical; stale/conflict cases open. |
| template_save | paired — same draft to separate isolated catalogs; receipts match except path; overwrite/resource cases open. |

Evidence in this machine's temporary directories: `tinixcut-group-parity-uewjfttu/report.json`, `tinixcut-write-parity-vpcuvgck/report.json`, `tinixcut-edit-extra-u2zoamod/report.json`, `tinixcut-edit-extra-k0bpr2d8/report.json`, `tinixcut-visual-parity-kp9_tmzw/report.json`, `tinixcut-transcript-parity-k8bry2_o/report.json`, `tinixcut-adapter-success-l7t14z90/`, `tinixcut-adapter-cancel-17c8enc4/`, and the local `artifacts/background/cli-mcp-empty-arguments.txt`. Temp evidence is not portable or committed.

Qualification isolation smoke: the production identifier build used a separate WebView profile and feature-gated settings/recovery/plugin directories. Settings and recovery survived restart; a feature-gated scope probe granted a fixture and verified restoration after restart; the persisted asset scope was under the test root. This does not validate every plugin behavior or an installed release.

Qualification review checkpoint:

- The ordinary build uses the registry plugin; only `qualification-isolation` enables the separately copied plugin. Both `cargo check --locked` variants passed. The production-identity qualification configuration is rejected at startup when the feature is absent.
- The scope harness creates a unique direct child of the temporary directory, checks process binary, WebView profile and window title, and confirms scope grant and restore receipts. It retains the data root for review. The scope, settings and recovery restart smoke passed with production identifier; process and WebView cleanup passed. Qualification also routes editor and automation media caches and the default template catalog into that root. A background run saved a template and cleared its isolated cache; separate control sentinels and SHA-256 snapshots of existing default cache/catalog contents remained unchanged.
- The harness now also supplies disposable TEMP/TMP and catalog fallback controls before launching; it checks their complete file inventories in a finally block and restores the original environment. A controlled early-failure run and a successful restart run passed; focused control tests detect modified/deleted cache and unexpected catalog entries. One independent snapshot of the real cache changed concurrently (two files added); a later before/after SHA-256 snapshot found no added, removed or modified files. The source of the earlier concurrent additions was not established.
- A focused qualification guard test passed for an allowed root, an invalid root and a plugin directory junction. The checks reduce accidental misuse; filesystem path checks are not a security boundary against a malicious concurrent process replacing paths.
- Temporary comparison scripts still contain machine-specific paths and are excluded from the proposed commit, as are build targets and temporary evidence. The parity evidence referenced above is local and not portable.
- Normal installed production runtime, installed packaging, output-schema/annotation migration, and the partial cases above remain unqualified. Do not interpret this review as exhaustive equivalence.
