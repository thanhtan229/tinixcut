# plan_phase18.md — Phase 18: Audio Crossfade Semantics & Preview/Export Parity

**Project:** TinixCut
**Phase:** 18
**Title:** Audio Crossfade Semantics & Preview/Export Parity
**Plan status:** SELF-REVIEWED AND LOCKED FOR IMPLEMENTATION
**Implementation status:** COMPLETE WITH DEFERRED ITEMS
**Research gate:** COMPLETE — see Phase 18 in `RESEARCH.md`
**Architecture decision:** ADR-006
**Schema:** v4 (lossless migration from v1–v3)
**Primary platform:** Windows 10/11 x86_64

---

## 1. Objective

Add a persistent, undoable, automation-accessible equal-power crossfade between adjacent standalone audio clips on an Audio track. Evaluate one canonical gain envelope for sampled preview and full export, and make standalone Audio-track clips audible in the editor preview during transport.

The feature must preserve exact project duration and existing per-clip volume, fade, mute, and keyframe semantics. It must not add timeline overlap or make FFmpeg filter syntax part of the project model.

## 2. Scope

- Schema v4 `AudioCrossfade` stored on an Audio track, with exact `RationalTime` duration and stable transition identity.
- Validation for one adjacent pair of standalone `ItemKind::Audio` clips on a `TrackKind::Audio` track, provable source-media handles, sufficient editorial extents, unique IDs, and one crossfade per cut.
- Lossless v1→v4 migration; older projects receive empty audio-transition collections.
- Command Service create, update-duration, and remove operations with revision fencing, receipts, undo/redo, and deterministic dependent move/trim/split/delete/duplicate behavior.
- Automation capability and MCP mutation exposure through the existing command path.
- Canonical equal-power gains over the centered transition interval: outgoing `cos(πp/2)`, incoming `sin(πp/2)`, with `p` derived from canonical timeline time. Use the video transition model's deterministic split `L=floor(D/2)`, `R=D-L`: the left clip supplies `R` postroll and the right clip supplies `L` preroll. Extend only normalized/rendered audio branches into those source handles; persisted timeline ranges remain adjacent and project duration is unchanged.
- These gains multiply existing clip volume, keyframe automation, fades, and mute. In transition handles outside an item's editorial range, clamp clip-local offset to `[0, item_duration]`, then evaluate keyframes and fades using the existing evaluator at that endpoint. Incoming preroll therefore holds offset 0 (including zero gain when fade-in exists); outgoing postroll holds `item_duration` (including zero gain when fade-out exists). Preview and export use this identical rule.
- Normalized preview samples expose the same effective audio gains. Preview transport plays standalone Audio-track media and embedded audio from active Video items, synchronizes seeks and play/pause, and applies the sampled gains. For Video items, the video element owns its embedded audio clock so decoded picture and sound stay synchronized. Crossfades involving embedded video audio remain outside this phase.
- Export produces sample-rate-grid gain envelopes for crossfade participants and combines them with existing per-clip automation before the existing mixer. It does not shorten the project or concatenate track audio with `acrossfade`.
- Timeline/inspector affordances for creating, selecting, resizing, and removing an audio crossfade, following `UX_UI.md`.

## 3. Non-goals

- Crossfades for embedded audio on Video tracks or transitions coupled to video `CutTransition`.
- Audio overlap editing, arbitrary audio transition curves, crossfades across gaps, more than two participants, or generalized transition frameworks.
- A new native audio engine, waveform redesign, audio effects, loudness normalization, or audio-device configuration.
- Changing video transitions, their schema semantics, preview compositor, or export video graph.
- Hardware-accelerated audio/render work or release packaging qualification.

## 4. Canonical Semantics and Constraints

1. Audio items remain adjacent with zero persisted overlap; transition duration does not change project duration.
2. A crossfade occupies a centered interval at the cut using deterministic left/right division for odd duration ticks: `L=floor(D/2)`, `R=D-L`. The outgoing item needs `R` verified source postroll; the incoming item needs `L` verified source preroll. Both also need editorial extents of at least `L` and `R`, respectively. The transition may read those source handles outside persisted item ranges only in its derived composition branch.
3. The power-complementary curve is fixed in v1. For normalized progress `p ∈ [0,1]`, outgoing gain is `cos(πp/2)` and incoming gain is `sin(πp/2)`. The stored model does not persist curve names or FFmpeg options.
4. Transition gain is multiplied by the clip's existing effective gain. At each audio output sample, evaluate the same semantic gain on the supported 44.1 kHz or 48 kHz grid. Preview samples use the same evaluator at their requested canonical time.
5. Boundary behavior is deterministic: before the transition only the outgoing transition multiplier applies; inside it both participants receive their corresponding gains; at/after its end only the incoming multiplier applies.
6. Only Audio-track standalone `ItemKind::Audio` participants are valid. Missing/offline or unprobed media cannot prove source handles and transition creation fails with a stable actionable error; it never substitutes another asset.
7. All authoritative mutations route through `CommandService`; gesture state remains transient until a command is committed.
8. Schema migration adds an empty transition collection and preserves all existing v1–v3 project values. No migration may infer transitions.

## 5. Acceptance Criteria

- **P18-AC-01 — Domain and schema:** Schema v4 models `AudioCrossfade` with stable ID, owning track, participant IDs, and exact duration. Validation enforces track/media kind, adjacency, unique cut/IDs, editorial extents, and provable source handles; malformed projects fail clearly with stable error codes.
- **P18-AC-02 — Migration:** v1, v2, and v3 fixtures migrate losslessly to v4 with empty audio transitions; current v4 save/load round-trips transitions; unsupported future schema remains rejected.
- **P18-AC-03 — Commands and history:** Create/update/remove use the Command Service, produce structured results, enforce revisions, and round-trip through undo/redo without changing project duration.
- **P18-AC-04 — Dependent edits:** Move, trim, split, delete, and duplicate preserve or invalidate affected transitions deterministically, with regression evidence for each relevant operation.
- **P18-AC-05 — Canonical gain:** Tests verify endpoints, midpoint gains, power complement, mixed rational rates, mute, volume, fades, animated volume composition, and transition-handle boundary evaluation.
- **P18-AC-06 — Preview sampling:** `sample_preview_at` returns identical effective gains for representative points and boundary frames; stale requests remain supersedable.
- **P18-AC-07 — Audible preview:** Audio-track media and embedded audio from active Video items follow play, pause, seek, stop, and project-end transport. Gain changes smoothly across supported audio-track crossfades; muted/offline clips do not produce sound; preview cleanup releases media resources. Video preview synchronization must not continually seek an actively playing decoder during normal playhead updates.
- **P18-AC-08 — Export envelopes:** Export automation emits sample-accurate envelopes for crossfade participants even without animated volume. Envelope values match canonical gains at 44.1 kHz and 48 kHz, multiply existing fades/keyframes, and preserve exact timeline placement and duration.
- **P18-AC-09 — Real media export:** Deterministic tone fixtures with verified source handles confirm both participants contribute across the transition, endpoint dominance, no gap/extra overlap, and correct final audio duration using app-owned FFmpeg 9.0.2.
- **P18-AC-10 — Automation:** Capabilities and MCP tools expose create/update/remove through the same Command Service and preserve idempotency/revision behavior.
- **P18-AC-11 — UI:** Audio-track adjacent cuts offer create/select/remove and duration editing. Invalid cuts explain why unavailable; all committed edits are undoable and accessible without shortcuts.
- **P18-AC-12 — Parity and negative control:** Preview-sampled and export-envelope gains match on deterministic fixtures within a documented sample-grid tolerance; a deliberately altered curve or boundary is detected.
- **P18-AC-13 — Regression:** Existing video-only `CutTransition`, keyframe export, audio fades, volume automation, persistence/recovery, MCP, and project-duration invariants pass their relevant tests.
- **P18-AC-14 — Verification and runtime:** Required Rust/frontend checks and builds pass. Restart only the identified TinixCut development runtime after final edits and verify the new app starts, the window loads, and typed `app_health_check` succeeds.

## 6. Validation Strategy

- Add focused domain, migration, command/history, composition-sampling, export-envelope, MCP, and UI regression tests.
- Use deterministic generated tone media for actual FFmpeg verification at supported sample rates; avoid wall-clock performance thresholds in ordinary CI.
- Run focused suites during implementation, then `pnpm run test:all`, `pnpm run typecheck`, Rust formatting, clippy, and build according to `DEVELOPMENT_WORKFLOW.md`.
- Check project status and diff before the planning checkpoint and before the implementation completion checkpoint.
- Restart and health-check the exact development app only after all implementation edits and required checks are complete.

## 6.1. Completion Evidence and Reconciliation

**Phase classification:** `COMPLETE WITH DEFERRED ITEMS`

| Criterion | Result | Evidence / deferred qualification |
|---|---|---|
| P18-AC-01 | PASS | Domain validation and stable failure cases in Rust tests; `cargo test` |
| P18-AC-02 | PASS | v1/v2/v3 empty-transition migrations and v4 save/reopen round-trip |
| P18-AC-03 | PASS | Create/update/remove, revision fencing, duration invariant, undo/redo lifecycle tests |
| P18-AC-04 | PASS | Move, trim, split, delete, and duplicate reconciliation regression |
| P18-AC-05 | PASS | Equal-power endpoints/midpoint, mixed rational rates, volume, fades, keyframes, mute, and clamped handles |
| P18-AC-06 | PASS | Preview samples use the canonical evaluator; supersession behavior retained and tested |
| P18-AC-07 | PARTIAL — INTERACTIVE TRANSPORT PASS | User manually verified video/audio playback beyond 10 seconds, pause, seek, project-end behavior, and mute/unmute. Preview media-resource cleanup was not independently observed in this session. |
| P18-AC-08 | PASS | Sample-grid envelopes verified at 44.1 kHz and 48 kHz, including composition with clip gain |
| P18-AC-09 | PASS | App-owned FFmpeg 9.0.2 tone export verifies both tones and exact output duration |
| P18-AC-10 | PASS | MCP create/update/remove acceptance verifies the shared command path and idempotency/revision behavior |
| P18-AC-11 | PASS | User manually verified valid crossfade creation, audible crossfade, duration change, removal, undo/redo, and a clear missing-handle error. |
| P18-AC-12 | PASS | Preview/export parity fixture passes and deliberately altered gain is detected |
| P18-AC-13 | PASS | Full regression suite covers video transitions, keyframes, fades, persistence/recovery, MCP, and duration invariants |
| P18-AC-14 | DEFERRED | Typecheck, fmt, Clippy, tests, and builds pass; development runtime is restarted and its new process/window is verified. Typed `app_health_check` IPC could not be invoked/observed in this session. |

Re-reviewed against ADR-006: implementation follows its schema, command, composition, export, and preview boundaries; no durable architecture change or deviation was found, so `ARCHITECTURE.md` remains unchanged. `RESEARCH.md`, `PLAN.md`, and `PROJECT_STATUS.md` record the implementation evidence and remaining deferred runtime checks. The mixed-frame-rate regression uncovered during review was fixed by normalizing clip-local time before fade evaluation.

Automated verification at the latest Phase 18 checkpoint: `pnpm run test:all` (44 frontend tests; 210 Rust tests passed, with 5 intentionally ignored benchmark tests), `pnpm run typecheck`, `pnpm run build`, `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`, and `cargo build --manifest-path src-tauri/Cargo.toml` all passed. The user subsequently passed the documented interactive transport and crossfade scenarios. Computer-use did not expose a native application window, so it could not independently invoke typed `app_health_check`; preview media-resource cleanup also was not independently observed.

## 7. Stop Conditions

Stop implementation and reconcile the plan/architecture before proceeding if:

- a schema-v4 migration cannot preserve existing projects losslessly;
- audio source handles cannot be proven from existing asset metadata/validation without weakening the video-transition handle policy;
- transition-handle audio evaluation cannot be made consistent with clip-local fade/keyframe semantics in both preview and export;
- canonical sample-grid gains cannot be shared by preview sampling and export;
- current volume/fade/keyframe composition has contradictory or undocumented gain ordering;
- preview playback requires a new durable subsystem or violates the Preview Backend boundary;
- the selected UI/API behavior requires expanding beyond standalone audio-track cuts;
- real media tests expose duration drift or an unbounded export/preview resource path.

## 8. Completion Gate

After implementation, conduct an independent second-pass review against every P18-AC item, inspect the full diff and test evidence, classify the phase as `COMPLETE`, `COMPLETE WITH DEFERRED ITEMS`, or `INCOMPLETE`, reconcile `PLAN.md`, `ARCHITECTURE.md`, `RESEARCH.md`, and `PROJECT_STATUS.md`, then create the authorized phase completion checkpoint commit. Do not claim completion for any unverified runtime or external gate.
