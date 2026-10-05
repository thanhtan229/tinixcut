# ADR-003: Adjacent-Cut CrossDissolve, Cut-Owned Topology, and Transition Compositing Semantics

## Status

**ACCEPTED & IMPLEMENTED** — Phase 13 Architecture Reconciliation (Implemented and Verified in Phase 13)  
**Date:** 2026-10-04  
**Deciders:** Core Architecture Team  
**Scope:** Video Track Adjacent-Cut CrossDissolve, Schema v2 Transition Topology, Dual-Backend Compositing  
**Consulted:** Phase 13 Technical Research Gate, Gate E Parity Specification, FFmpeg `xfade` filter internals, W3C Compositing and Blending Level 1

---

## 1. Context and Problem Statement

During Phase 9, single-clip opacity fade transitions (`fade_in`, `fade_out`) were implemented and verified. However, adjacent-clip cross-dissolve (`TransitionKind::CrossDissolve`) was explicitly deferred (`plan_phase9_crossdissolve_reconciliation.md`).

The root cause of the Phase 9 deferral was architectural:
1. **Flawed Item-Owned Topology**: In Schema v1, transitions were modeled as an optional property on individual clips (`TimelineItem.transition: Option<Transition>`). A cross-dissolve, however, is fundamentally a **binary relation between two adjacent clips meeting at a cut boundary**. Assigning ownership to either the outgoing (left) or incoming (right) clip created asymmetric editing semantics, ambiguous serialization, and impossible handle validations.
2. **Timeline Overlap Paradox**: In physical film editing and traditional analog workflows, a dissolve requires both media streams to play simultaneously over the transition duration $D$. Naive digital implementations attempt to model this by shifting the incoming clip leftward on the timeline, causing track overlap. In a single-track NLE model, overlapping clips on the same track violate track non-overlap invariants, mutate the total timeline duration, ripple all downstream clips, and break synchronization with audio and subtitle tracks.
3. **Compositing Luminance Dip (The $25\%$ Alpha Hole)**: Many naive video compositors implement cross-dissolve by placing the incoming clip on top of the outgoing clip and animating standard `source-over` opacity ($1-p$ on left, $p$ on right). As mathematically proven in Section 3, standard Porter-Duff `source-over` compositing between two attenuated layers results in a severe non-linear drop in luminance and alpha (a $25\%$ transparency hole at midpoint $p=0.5$), revealing whatever background or black layer lies underneath.
4. **Handle Ambiguity & Freeze-Frame Corruption**: A cross-dissolve requires frames beyond the cut point: postroll frames for the outgoing clip and preroll frames for the incoming clip. In the absence of sufficient media handles, consumer video editors frequently auto-shorten transitions or silently freeze the last/first frame. In an automation-first professional editor, silent frame freezing corrupts editorial intent and violates non-destructive guarantees.

Phase 13 establishes the authoritative architecture, data model, mathematical invariants, and dual-backend compositing rules for adjacent-cut `CrossDissolve`.

---

## 2. Decision Drivers

- **Zero Timeline Duration Mutation**: Adding, adjusting, or deleting a cut transition must preserve total track and project duration identically ($\Delta T_{\text{project}} = 0$). Editorial timeline sync across all other tracks must remain perfectly undisturbed.
- **Cut-Owned Topology**: A transition between adjacent clips is owned by the cut boundary on the track, not by an individual timeline clip.
- **Mathematical Parity Invariant**: Preview (isolated DOM `PreviewTransitionGroup`) and export (FFmpeg) must implement the identical canonical premultiplied linear blend $(1-p)A + pB$. Ordinary two-layer `source-over` opacity is strictly forbidden.
- **Mandatory Provable Handles**: Handles must be verified against source media duration prior to transition creation. If source media handles are insufficient or unverifiable, the command must reject with structured error codes (`INSUFFICIENT_TRANSITION_HANDLE`, `TRANSITION_HANDLE_UNVERIFIABLE`). Clamping, auto-shortening, or freeze-frame synthesis is prohibited.
- **Schema v2 Promotion**: Clear, versioned migration path from Schema v1 to Schema v2, adding track-level `CutTransition` collection for pairwise adjacent cuts, while preserving existing production-supported single-clip head/tail fades at the item level losslessly.
- **Strict Scope Discipline**: Video-only for Phase 13 (video/video participants; non-video rejected with `UNSUPPORTED_TRANSITION_MEDIA`). Audio crossfade (`acrossfade`) is explicitly deferred. Arbitrary shader engines, wipe transitions, general plugin frameworks, and Bezier/spectral waveform expansions remain deferred.

---

## 3. Mathematical Proof: Non-Equivalence of Ordinary Source-Over and Canonical CrossDissolve

### 3.1 Theorem (Luminance Dip in Modulated Source-Over)
Let clip $A$ have premultiplied color $C_A^* = \alpha_A C_A$ with alpha $\alpha_A \in [0, 1]$.  
Let clip $B$ have premultiplied color $C_B^* = \alpha_B C_B$ with alpha $\alpha_B \in [0, 1]$.  
Let $p \in [0, 1]$ be the normalized transition progress.

The canonical premultiplied cross-dissolve blend is defined as:
$$C_{\text{blend}}^* = (1 - p) C_A^* + p C_B^*$$
$$\alpha_{\text{blend}} = (1 - p) \alpha_A + p \alpha_B$$

For two fully opaque video frames ($\alpha_A = 1, \alpha_B = 1$):
$$\alpha_{\text{blend}} = (1 - p)(1) + p(1) = 1.0 \quad \forall p \in [0, 1]$$
$$C_{\text{blend}}^* = (1 - p) C_A + p C_B$$

Now consider an implementation that attempts to render a cross-dissolve by placing clip $B$ over clip $A$ using the standard Porter-Duff **`source-over`** compositing operator, with both layers attenuated by opacity factors:
$$\alpha_A' = (1 - p) \alpha_A, \quad \alpha_B' = p \alpha_B$$

In Porter-Duff `source-over` ($B'$ over $A'$), the resulting composite alpha $\alpha_{\text{over}}$ is:
$$\alpha_{\text{over}} = \alpha_B' + \alpha_A'(1 - \alpha_B')$$

Substituting $\alpha_A' = (1 - p)\alpha_A$ and $\alpha_B' = p\alpha_B$:
$$\alpha_{\text{over}} = p \alpha_B + (1 - p)\alpha_A (1 - p \alpha_B) = (1 - p)\alpha_A + p \alpha_B - p(1 - p)\alpha_A \alpha_B$$

Calculating the discrepancy $\Delta \alpha = \alpha_{\text{blend}} - \alpha_{\text{over}}$:
$$\Delta \alpha = p(1 - p)\alpha_A \alpha_B$$

For standard opaque video ($\alpha_A = 1, \alpha_B = 1$):
$$\Delta \alpha = p(1 - p)$$

Evaluating at the transition midpoint ($p = 0.5$):
$$\Delta \alpha = 0.5(1 - 0.5) = 0.25$$
$$\alpha_{\text{over}} = 1.0 - 0.25 = 0.75$$

### 3.2 Corollary (The Midpoint Transparency Hole)
In standard two-layer `source-over` opacity compositing, the composite alpha drops to $0.75$ at the transition center. Exactly $25\%$ of the underlying background canvas (typically black or empty track) bleeds through, producing an unphysical dark dip / luminance drop.

Furthermore, if the bottom layer $A$ is kept at opacity $1.0$ while only $B$ fades in with opacity $p$, the blend is asymmetric: $C_{\text{over}} = p C_B + (1 - p) C_A$ holds for opaque colors, but any semi-transparency or spatial transformation (scaling, rotation, cropping) immediately destroys the invariant and fails parity with export.

### 3.3 Normative Rule
Ordinary two-layer `source-over` opacity modulation is **mathematically prohibited** for CrossDissolve. CrossDissolve must be executed exclusively via additive premultiplied mixing:
$$C_{\text{blend}}^* = (1 - p) C_A^* + p C_B^*$$
In CSS preview, this corresponds to weighted A/B contributions with `mix-blend-mode: plus-lighter` on premultiplied RGB inputs strictly inside an isolated DOM stacking context (`PreviewTransitionGroup`), composited via `source-over` onto the normal stack. In FFmpeg export, this corresponds to bounded aligned `xfade=transition=fade`.

---

## 4. Architectural Decisions

### 4.1 Cut-Owned Transition Topology
1. A cross-dissolve transition does not belong to a clip. It belongs to the **cut boundary between two immediately adjacent clips** on the same track.
2. In the domain model, `CutTransition` is stored in a track-level collection:
   ```rust
   pub struct CutTransition {
       pub transition_id: String,
       pub track_id: String,
       pub left_item_id: String,
       pub right_item_id: String,
       pub kind: TransitionKind, // Restricted to CrossDissolve in Phase 13
       pub duration: RationalTime,
   }
   ```
   *Alignment note*: Phase 13 CrossDissolve provides centered cut semantics exclusively, governed by the deterministic L/R tick division rule ($L_{\text{ticks}} = \lfloor D_{\text{ticks}} / 2 \rfloor, R_{\text{ticks}} = D_{\text{ticks}} - L_{\text{ticks}}$). Future alignment extensibility (such as start-on-cut or end-on-cut) is deferred and is NOT a persisted field in Schema v2.
3. **Immediate Adjacency Invariant**:
   A cut transition is valid if and only if:
   $$\text{left.timeline\_range.end()} == \text{right.timeline\_range.start} == C$$
   Both clips must be on the same track, with no gap between them.
4. **Canonical Timeline Invariance**:
   The clips remain abutted edge-to-edge on the track timeline. Their stored `timeline_range` values are **never modified** when a transition is added, edited, or removed. The total timeline duration remains strictly constant ($\Delta T_{\text{project}} = 0$).

### 4.2 Schema v2 Specification and Migration Protocol
1. Project domain schema version is promoted to `schema_version = 2`.
2. `Track` struct is extended to hold `pub transitions: Vec<CutTransition>`.
3. **Preservation of Single-Clip Fade Semantics**:
   Production-supported clip-owned Fade semantics currently on `TimelineItem` (`fade_in`, `fade_out`, and supported item-level `FadeIn`/`FadeOut` variants) must **NOT** be cleared, dropped, or re-homed. They remain supported at the item level. `TimelineItem.transition` is not deprecated in any manner that implies field removal in Phase 13 while existing supported Fade variants still depend on it. Pairwise cut transitions live exclusively in `Track.transitions`.
4. **Lossless Migration Protocol**:
   - When loading a Schema v1 project (`schema_version = 1`), deserialization initializes `Track.transitions: Vec::new()` (empty cut-transition collection).
   - Existing v1 item-level Fade behavior survives v1→v2 migration losslessly.
   - Migration does **NOT** invent `CrossDissolve` from v1.
   - If a malformed Schema v1 project contains an unsupported `CrossDissolve` placeholder in item-level transition, it is handled explicitly according to migration/error policy (rejected or surfaced as an explicit migration/validation error; it must never be silently activated as a cut transition).
   - Saved projects are serialized with `schema_version: 2`.

### 4.3 Exact RationalTime Odd-Duration Rule and Mandatory Provable Media Handles
A cross-dissolve of duration $D$ centered on cut point $C$ (where $\text{left.timeline\_range.end()} == \text{right.timeline\_range.start} == C$) must not use floating-point math or naive fractional division. It is governed by the exact **RationalTime Odd-Duration Rule**:
1. Rescale transition duration $D$ to the project timeline frame rate, yielding exact integer ticks $D_{\text{ticks}}$.
2. Partition duration deterministically into left and right tick offsets:
   $$L_{\text{ticks}} = \lfloor D_{\text{ticks}} / 2 \rfloor$$
   $$R_{\text{ticks}} = D_{\text{ticks}} - L_{\text{ticks}}$$
3. Define the transition window boundaries:
   $$T_0 = C - L$$
   $$T_1 = C + R$$
   The transition interval is strictly half-open: $[T_0, T_1)$.
4. **Editorial Extent Invariants**:
   - Outgoing editorial extent $\ge L$ (i.e. $\text{left.timeline\_range.duration} = C - \text{left.timeline\_range.start} \ge L$).
   - Incoming editorial extent $\ge R$ (i.e. $\text{right.timeline\_range.duration} = \text{right.timeline\_range.end()} - C \ge R$).
5. **Handle Requirements & Available Handles**:
   - **Postroll Handle (Left / Outgoing Clip)** requires $\text{available\_postroll} \ge R$, where:
     $$\text{available\_postroll} = \text{asset\_duration} - (\text{left.source\_range.start} + \text{left.source\_range.duration})$$
   - **Preroll Handle (Right / Incoming Clip)** requires $\text{available\_preroll} \ge L$, where:
     $$\text{available\_preroll} = \text{right.source\_range.start}$$

### 4.4 Stable Machine-Readable Error Contract
Transition validation errors must produce stable, machine-readable category codes across command, validation, and automation layers:
- `INSUFFICIENT_TRANSITION_HANDLE`: Source media duration is known, but available postroll handle is less than $R$, available preroll handle is less than $L$, or editorial extent is less than required.
  ```json
  {
    "code": "INSUFFICIENT_TRANSITION_HANDLE",
    "message": "Cannot create CrossDissolve: required media handle exceeds available source frames.",
    "required_preroll": { "value": 15, "rate": 30 },
    "available_preroll": { "value": 10, "rate": 30 },
    "required_postroll": { "value": 15, "rate": 30 },
    "available_postroll": { "value": 30, "rate": 30 }
  }
  ```
- `TRANSITION_HANDLE_UNVERIFIABLE`: Media duration or stream handle availability cannot be proven (e.g. unprobed media, stream duration missing, or asset unavailable).
- `UNSUPPORTED_TRANSITION_MEDIA`: Participant is an image, synthetic graphic, text, or non-video item (Phase 13 supports video/video cuts exclusively).

**Strict Invariants**:
- **Prohibition of Auto-Shortening**: The engine must never automatically shorten the duration $D$ to match available handles.
- **Prohibition of Frame Freezing**: The engine must never duplicate or freeze endpoint frames to simulate missing media.

### 4.5 Dual-Backend Compositing Architecture

```text
                  +--------------------------------+
                  |  Track Cut Transition (Domain) |
                  +--------------------------------+
                                   |
                                   v
                  +--------------------------------+
                  |    Normalized TransitionPlan   |
                  |  - Left Clip + Postroll Handle |
                  |  - Right Clip + Preroll Handle |
                  |  - Interval [T0, T1) half-open |
                  +--------------------------------+
                                   |
            +----------------------+----------------------+
            |                                             |
            v                                             v
+-----------------------+                     +-----------------------+
|    Preview Backend    |                     |     Export Backend    |
| (DOM PreviewTransit-  |                     |        (FFmpeg)       |
|       ionGroup)       |                     |                       |
+-----------------------+                     +-----------------------+
| Explicit isolated     |                     | Normalized            |
| `PreviewTransition-   |                     | full-canvas RGBA A/B  |
|  Group` container     |                     |                       |
|                       |                     | Premultiply RGBA      |
| Left: opacity(1-p)    |                     |                       |
| Right: opacity(p)     |                     | Bounded aligned       |
|                       |                     | `xfade=transition=   |
| `mix-blend-mode:      |                     |  fade:duration=D`     |
|   plus-lighter`       |                     |                       |
| strictly inside group |                     | Unpremultiply to RGBA |
|                       |                     |                       |
| Blended group output  |                     | One transition-result |
| composited source-    |                     | contribution into     |
| over onto normal stack|                     | existing overlay chain|
+-----------------------+                     +-----------------------+
```

#### Preview Engine Implementation:
- Outside the transition interval ($t < T_0$ or $t \ge T_1$), clips render normally as standard video layers.
- Inside the transition interval $[T_0, T_1)$:
  - Both clips are placed inside an explicit `PreviewTransitionGroup` container (`<div class="preview-transition-group">`) forming an isolated DOM stacking/compositing group.
  - Weighted A/B contributions:
    Normalized progress $p(t) = (t - T_0) / D \in [0.0, 1.0)$.
    Left clip renders frame at source time $S_A(t) = \text{left.source\_range.start} + (t - \text{left.timeline\_range.start})$ with opacity $1 - p$.
    Right clip renders frame at source time $S_B(t) = \text{right.source\_range.start} - (C - t)$ with opacity $p$.
  - Layers are combined using `mix-blend-mode: plus-lighter` (additive premultiplied mixing) strictly inside the group.
  - The blended group output is composited via standard `source-over` onto the normal stack.
  - Canvas/offscreen compositor is **NOT** an authorized alternative Phase 13 implementation.
  - Silent fallback to ordinary `source-over` is strictly prohibited.

#### Export Engine Implementation:
The authoritative export sequence is:
`normalized full-canvas RGBA A/B`
$\to$ `premultiply`
$\to$ bounded aligned `xfade=transition=fade`
$\to$ `unpremultiply`
$\to$ one transition-result contribution
$\to$ existing overlay chain.

- Segment $[T_0, T_1)$ is rendered through FFmpeg filtergraph:
  1. Extract handle-extended streams for left and right clips.
  2. Normalize full-canvas RGBA pixel format (`format=pix_fmts=rgba`).
  3. Premultiply inputs prior to dissolve evaluation.
  4. Feed streams into bounded aligned `xfade=transition=fade:duration=D:offset=...`.
  5. Unpremultiply the blended stream back to RGBA.
  6. The transition result represents the single contribution for that track interval and joins the existing overlay chain.
- Inside $[T_0, T_1)$, ordinary unblended A and B contributions on the track must be strictly **suppressed**.

### 4.6 Scope Boundary: Video Only, Audio Deferred
- Phase 13 is strictly confined to **Video CrossDissolve** between two video clips (`ItemKind::Video`). Non-video items trigger `UNSUPPORTED_TRANSITION_MEDIA`.
- Audio across the cut boundary remains an exact butt-cut.
- True audio cross-fading (`acrossfade` / equal-power crossfade) requires dedicated audio graph scheduling and is explicitly **deferred** to a future audio phase.
- Arbitrary shader graphs, wipe masks, page turns, and third-party transition plugins are **not authorized**.
- Bezier curve interpolation and spectral waveform enhancements remain **deferred**.

---

## 5. Alternatives Considered and Rejected

| Alternative | Summary | Why Rejected |
|---|---|---|
| **1. Item-Owned Transition** | Keep `TimelineItem.transition` as owner of CrossDissolve on outgoing clip. | Asymmetric ownership; ambiguous serialization; fails when either clip is moved or split; cannot logically validate right clip's preroll handle from left clip's context. (Single-clip Fade remains item-owned). |
| **2. Timeline Overlap Mutation** | Slide incoming clip leftward on timeline so clips overlap on track. | Mutates total project duration; causes cascading ripple shifts across all downstream edits; breaks synchronization with external audio/subtitles; non-recoverable if transition is deleted. |
| **3. Ordinary Two-Layer Source-Over Opacity** | Modulate top clip opacity $p$ and bottom clip opacity $1-p$ using standard CSS/FFmpeg `overlay`. | Mathematically proven to cause a $25\%$ luminance/alpha dip at midpoint $p=0.5$, bleeding background canvas and failing Gate E parity. |
| **4. Auto-Clamping / Freeze-Framing** | Silently clamp duration or freeze the last frame when media handles are exhausted. | Violates non-destructive editing guarantees; corrupts creator editorial timing; hides media asset limits from automation agents. |
| **5. Generalized Transition Plugin Framework** | Introduce dynamic WebGL shaders and configurable wipe patterns. | Premature generalization; violates smallest safe change rule; introduces unvetted GPU dependencies and threatens dual-backend parity. |
| **6. Simultaneous Audio Crossfade** | Implement audio crossfade in tandem with video CrossDissolve. | Expands phase scope beyond single coherent slice; audio requires separate equal-power curve math and filtergraph restructuring. |
| **7. Canvas/Offscreen Compositor Alternative in Phase 13** | Use offscreen canvas rendering instead of isolated DOM `PreviewTransitionGroup`. | Violates smallest coherent change rule; adds unneeded buffer allocations and sync complexity when DOM stacking context with `plus-lighter` provides direct parity. |

---

## 6. Normative Contract Summary (CD-01 .. CD-43)

The complete canonical specification for Phase 13 is codified in the **CD-01 through CD-43** contract set recorded in `RESEARCH.md` and enforced by ADR-003:

- **CD-01 .. CD-07**: Domain Model & Track Topology (Cut-owned, immediate adjacency, single track, video items only, non-overlapping timeline storage, timeline duration invariance, transition singularity).
- **CD-08 .. CD-13**: Schema v2 & Migration (Schema v2 promotion, track-level cut-transitions collection, preservation of single-clip item-level fade semantics losslessly, lossless forward migration with empty transitions collection, explicit error policy for malformed v1 placeholders without silent activation).
- **CD-14 .. CD-21**: Time, Alignment & Handle Invariants (Exact `RationalTime` precision, deterministic odd-duration tick rule $L_{\text{ticks}} = \lfloor D_{\text{ticks}} / 2 \rfloor, R_{\text{ticks}} = D_{\text{ticks}} - L_{\text{ticks}}, T_0 = C - L, T_1 = C + R$, half-open interval $[T_0, T_1)$, centered semantics only without persisted `CutAlignment` in Schema v2, provable preroll $\ge L$ and postroll $\ge R$, editorial extent invariants, stable machine-readable error codes `INSUFFICIENT_TRANSITION_HANDLE` / `TRANSITION_HANDLE_UNVERIFIABLE` / `UNSUPPORTED_TRANSITION_MEDIA`, prohibition of auto-shortening, prohibition of freeze-framing, duration bounds).
- **CD-22 .. CD-27**: Mathematical Blend & Compositing Semantics (Canonical premultiplied RGBA blend $(1-p)A^* + pB^*$, linear progress $p(t) = (t - T_0)/D \in [0, 1)$, mathematical proof of source-over non-equivalence, prohibition of two-layer source-over, color space normalization, alpha preservation).
- **CD-28 .. CD-32**: Preview Architecture & PreviewTransitionGroup (Explicit isolated DOM `PreviewTransitionGroup` container, isolated stacking context, weighted A/B contributions, `plus-lighter` strictly inside group, group source-over onto normal stack, no Canvas/offscreen compositor alternative, no silent fallback to source-over, handle-aware frame dispatch, seamless $[T_0, T_1)$ window boundary transition, scrubbing/seek determinism).
- **CD-33 .. CD-37**: Export Filtergraph Architecture (Authoritative sequence: `normalized full-canvas RGBA A/B` $\to$ `premultiply` $\to$ bounded aligned `xfade=transition=fade` $\to$ `unpremultiply` $\to$ one transition-result contribution $\to$ existing overlay chain; suppression of ordinary A/B contributions during $[T_0, T_1)$; preview/export parity within Gate E tolerances).
- **CD-38 .. CD-43**: Scope Boundaries & Governance (Video-only confinement, audio crossfade deferral, prohibition of general transition framework, Bezier/spectral waveform deferral, `CrossDissolve` restriction, Command Service exclusivity, automation surface exposure).
