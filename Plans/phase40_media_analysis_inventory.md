# Phase 40 — Media analysis inventory

**Status:** COMPLETE (Gate 40 PASS WITH DOCUMENTED MODEL/FINGERPRINT LIMITS).

## Existing capabilities

| Capability | Current evidence | Gap for AI planning |
|---|---|---|
| Media metadata | `tinixcut_media_probe` uses `FFmpegService::probe_file` and is read-only | No scene summary/candidate timeline |
| Thumbnail | `FFmpegService::extract_thumbnail` | No first-class cancellable analysis result exposed to MCP |
| Waveform | Tauri `media_get_waveform` uses `JobManager`, deduplication and a background worker | Cache was keyed by asset ID, allowing stale reuse after source change; addressed below |
| Transcript | Local Whisper task is cancellable and maps offsets through `TranscriptionBoundary` to Command Service caption commands; `tinixcut_media_transcript` now returns source-time segments read-only | Requires explicit CLI and model paths; model absence is a stable error; no confidence calibration |
| Scene candidates | `tinixcut_media_scene_candidates` uses FFmpeg scene scores, bounded duration/candidate count and cancellable child process | Detector outputs candidates, not semantic scene labels; no accuracy claim beyond synthetic transition timestamps |

## Increment completed — waveform cache invalidation

- `WaveformData` schema is now version 2 and records `source_fingerprint`.
- The fingerprint is SHA-256 of canonical path, file size and modified timestamp. This is a quick invalidation signature; it is not a checksum of file contents.
- Cache file names include asset ID and fingerprint. Cache load verifies both schema and fingerprint. Old v1 cache records are treated as stale and regenerated.
- The current asset path is resolved before checking cache, so relinking to a different file naturally selects a different cache entry. Job deduplication also includes the fingerprint.

## Verification

- `modules::media::ffmpeg::tests::source_fingerprint_changes_when_source_metadata_changes`: PASS.
- `modules::media::cache::tests::waveform_cache_is_isolated_by_source_fingerprint_and_schema`: PASS.
- `modules::media::ffmpeg::tests::test_ffmpeg_probe_and_derivative_generation`: PASS as part of the full lib run.
- `cargo build --manifest-path src-tauri/Cargo.toml --bin tinixcut` on isolated target: PASS.
- Full `cargo test --lib`: 196 passed, 1 failed at `modules::media::qualification::tests::test_gate_f_poc5_matrix_execution`. Its report at `%TEMP%/tinix_test_poc5_matrix/poc5_qualification_report.json` shows P5-M09 NVENC driver/API mismatch; P5-M10 software export generated a 0-byte/unprobeable output; P5-M12 reported `passed=false` despite its details saying a valid MP4 was generated. Those export cases need separate diagnosis before relying on that matrix gate.
- `rustfmt --check` for the four changed Rust files and `git diff --check`: PASS.
- Runtime restart: `pnpm tauri dev`; TinixCut PID 25652 started after the changed source/binary timestamp, Vite PID 15652 serves `localhost:1420` with HTTP 200, Computer Use reads Core Healthy v0.1.0.

## Limitations and next work

- Metadata fingerprint can miss replacement where file length and modified timestamp are deliberately preserved. It avoids hashing full media on each waveform request; full content hashing would need its own bounded/cancellable job.
- Scene and transcript reports are versioned and cached by source/configuration fingerprints. Transcript cache configuration includes model metadata fingerprint and language.
- Scene timestamps passed a synthetic three-scene FFmpeg fixture at 1s and 2s; MCP repeat call returns the same cached report.
- Read-only transcript endpoint returns `WHISPER_CLI_MISSING` / `WHISPER_MODEL_MISSING` before launching a process. Cache configuration includes media, CLI and model metadata fingerprints plus language. Missing-model handling and successful local transcript E2E are both covered.
- Targeted scene parser tests (2/2), cache tests (3/3), scene/transcript MCP E2E, 16-tool discovery and JSON Schema/camelCase checks pass. E2E snapshots an unrelated project before analysis and confirms the full inspect report/revision remains unchanged afterward. Real local Whisper transcript matched the known JFK sample text and produced one 0–10.5s segment; report: `artifacts/phase40/transcript-readonly-e2e.json`.
- The analysis tools run through a shared four-worker capacity and terminate spawned FFmpeg/Whisper processes on MCP request cancellation. A live 220-second Whisper job was cancelled through MCP; the subsequent ping succeeded, no analysis response was sent for the cancelled request (rmcp protocol behavior), and no child process or temp work directory remained. Evidence: `artifacts/phase40/transcript-cancellation-e2e.json`.
- MCP E2E covers 1-candidate truncation, duration ceiling rejection (>1800s), scene and transcript offline-media rejection, four-worker saturation (`ANALYSIS_CAPACITY`), missing model and unchanged project revision.
- Confidence is not calibrated: FFmpeg scene scores are heuristic cut signals, not semantic labels, and Whisper segments carry no calibrated confidence values. Results need human review before they become confirmed transcript or edit intent.
- Fingerprints use canonical path, file length and modified time, so deliberate same-size replacement with restored timestamp may not invalidate cache.
- Gate 40 is complete for this scope. Continue to Phase 41 with model-generated outputs treated as suggestions and keep the remaining fingerprint/confidence limitations visible to the planner.
