# ADR-006: Audio Crossfade Semantics and Sample-Grid Gain

**Status:** ACCEPTED FOR PHASE 18 IMPLEMENTATION
**Date:** 2026-10-07
**Decision owners:** TinixCut project owner; planning and implementation delegated for this phase

## Context

TinixCut currently persists video cut transitions on tracks and compiles per-clip audio volume/fade/keyframe behavior into a normalized `CompositionPlan`. Audio cuts remain butt-cuts. Preview sampling already reports audio gain, but the current preview mutes video elements and does not play standalone audio tracks. Export builds independent delayed audio clip branches and mixes them; using FFmpeg `acrossfade` as project semantics would conflict with arbitrary track placement, clip gain automation, and zero-overlap canonical timelines.

Persisted transition intent requires an explicit schema change. Adding an unversioned field while claiming Schema v3 would violate the project format's versioning contract.

## Decision

1. Promote the project format to Schema v4 for `AudioCrossfade` semantics. Store transitions on `Track.audio_transitions`; v1–v3 migrations initialize this collection empty and never infer transitions.
2. Support only adjacent standalone Audio items on Audio tracks in this phase. Keep their timeline ranges adjacent and preserve project duration.
3. Define a centered transition interval using exact `RationalTime`, with `L=floor(D/2)` and `R=D-L`. The outgoing item must supply `R` verified source postroll and the incoming item `L` verified source preroll, following the established video cut-transition handle policy. Read those handles only in the derived transition composition branch; do not change the items' persisted timeline/source ranges.
4. Compute outgoing/incoming multipliers with fixed equal-power curves `cos(πp/2)` and `sin(πp/2)`, respectively.
5. Multiply transition gain by existing clip-level gain semantics (volume, keyframes, fades, mute) in the normalized composition evaluator. For times in a transition handle outside an item's editorial range, clamp its clip-local offset to `[0, item_duration]`, then evaluate volume keyframes and fades with the existing evaluator at that endpoint. Thus incoming preroll holds offset zero (including a zero fade-in gain when fade-in exists), and outgoing postroll holds the item duration (including a zero fade-out gain when fade-out exists).
6. Use that evaluator for preview samples and for sample-rate-grid export envelopes. Continue to mix timeline-positioned clip branches with the existing mixer; do not model transitions as `acrossfade` concatenation.
7. Add audible preview for standalone Audio-track clips using the existing WebView media path and transport synchronization. Keep native audio decoding/engine selection deferred. Embedded Video-item audio playback remains outside this phase.
8. Route create/update/remove through the existing Command Service and expose them through the existing UI and automation surfaces.

## Rationale

- Equal-power gains provide a fixed, deterministic v1 curve already identified by the deferred-work record.
- Per-clip gain envelopes compose with existing gain automation and preserve arbitrary timeline positions and mix behavior.
- The normalized evaluator maintains preview/export semantic parity without persisting backend filter syntax.
- Reusing the proven cut-transition handle policy supplies source audio across the zero-overlap cut while retaining strict project-duration invariance.
- Endpoint-clamped clip-local offsets reuse the visual transition plan's established handle behavior and the current audio fade/keyframe evaluator. An existing fade reaches its endpoint value in a handle instead of being bypassed by an early out-of-range mute.
- A dedicated versioned collection prevents v3 meaning from changing and permits explicit lossless migration.
- Restricting participants to standalone Audio-track clips avoids entangling audio transitions with the separate video transition compositor and linked-video audio policy.

## Consequences

- Current schema becomes v4; migration fixtures and compatibility checks are mandatory.
- Crossfade creation requires probed, available source media with enough preroll/postroll; unavailable handles produce an actionable validation error.
- Existing project duration, item ranges, and video transition schema remain unchanged.
- Preview gains a WebView audio playback path with cleanup, seek synchronization, and mute/offline handling.
- Equal-power is fixed for v1; other curves, linked-video audio transitions, native audio engine work, and advanced audio mixing remain deferred.

## Rejected Alternatives

- **Unversioned field added to Schema v3:** rejected because persisted semantics would change without a schema migration boundary.
- **FFmpeg `acrossfade` as the canonical feature:** rejected because it concatenates two streams and reduces their combined duration by the overlap, while TinixCut's audio clips are independently positioned and mixed on a duration-invariant timeline.
- **Export-only crossfade:** rejected because the editor already samples audio gain for preview and a persisted transition that cannot be heard while editing would be an incomplete editing behavior.
- **Native audio engine:** deferred because it would add a new preview subsystem before evidence requires it; current WebView media playback is sufficient for this bounded phase.
